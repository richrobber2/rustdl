//! Download lifecycle and scheduling across local state and remote I/O.

use super::super::local::models::ResolvedVideo;
use super::super::local::queue::{DownloadJob, DownloadOutcome, DownloadPhase, QUEUE_OUTPUT_DIR};
use super::super::local::runtime::{MUX_HOOK, PUBLISH_HOOK};
use super::super::{external, local, workflows};
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::error::Error;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::{fs, io, thread};

pub(in super::super) fn apply_queue_action(
    client: &Client,
    output_dir: &Path,
    filename: &str,
    action: &str,
) -> Result<(), String> {
    if !local::media::valid_video_filename(filename) {
        return Err("Invalid queue item".to_owned());
    }
    let mut should_schedule = false;
    {
        let mut jobs = local::queue::download_jobs()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let job = jobs
            .get_mut(filename)
            .ok_or_else(|| "Queue item not found".to_owned())?;
        match action {
            "pause"
                if matches!(
                    job.phase,
                    DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading
                ) =>
            {
                job.phase = DownloadPhase::Paused;
            }
            "resume"
                if matches!(
                    job.phase,
                    DownloadPhase::Paused | DownloadPhase::Failed | DownloadPhase::Cancelled
                ) =>
            {
                job.phase = DownloadPhase::Queued;
                job.error = None;
                should_schedule = true;
            }
            "cancel" if job.phase != DownloadPhase::Ready => {
                job.phase = DownloadPhase::Cancelled;
                job.error = None;
            }
            _ => return Err("That action is not available for this queue item".to_owned()),
        }
    }
    local::queue::persist_download_jobs();
    local::queue::download_gate().1.notify_all();
    local::queue::notify_transfer_state(true);
    if should_schedule {
        workflows::downloads::schedule_download_worker(
            client.clone(),
            output_dir.to_path_buf(),
            filename.to_owned(),
        );
    }
    Ok(())
}

pub(in super::super) fn start_web_download(
    client: &Client,
    source_url: &str,
    output_dir: &Path,
) -> Result<(PathBuf, DownloadOutcome), Box<dyn Error>> {
    let resolved = workflows::discovery::resolve_video(client, source_url)?;
    workflows::downloads::start_resolved_download(client, source_url, resolved, output_dir)
}

pub(in super::super) fn start_resolved_download(
    client: &Client,
    source_url: &str,
    resolved: ResolvedVideo,
    output_dir: &Path,
) -> Result<(PathBuf, DownloadOutcome), Box<dyn Error>> {
    fs::create_dir_all(output_dir)?;
    let filename = resolved.filename();
    let output = output_dir.join(&filename);
    let quality_label = resolved.quality_label.clone();
    let quality_height = resolved.quality_height;
    let audio_url = resolved.audio_url.clone();
    let extract_audio = resolved.extract_audio;

    if local::files::is_complete_download(&output)? {
        eprintln!("Duplicate detected; reusing {}", output.display());
        if let Some(publish) = PUBLISH_HOOK.get() {
            publish(&output, &filename)
                .map_err(|error| format!("could not publish to Android Downloads: {error}"))?;
        }
        local::queue::set_download_job(
            &filename,
            DownloadJob {
                phase: DownloadPhase::Ready,
                downloaded: fs::metadata(&output)?.len(),
                total: Some(fs::metadata(&output)?.len()),
                error: None,
                source_url: Some(source_url.to_owned()),
                media_url: Some(resolved.media_url),
                audio_url,
                extract_audio,
                quality_label,
                quality_height,
            },
        );
        return Ok((output, DownloadOutcome::Duplicate));
    }

    {
        let mut jobs = local::queue::download_jobs()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if jobs.get(&filename).is_some_and(|job| {
            matches!(
                job.phase,
                DownloadPhase::Queued
                    | DownloadPhase::Starting
                    | DownloadPhase::Downloading
                    | DownloadPhase::Paused
            )
        }) {
            return Ok((output, DownloadOutcome::InProgress));
        }
        if jobs
            .get(&filename)
            .is_some_and(|job| job.quality_label != quality_label)
        {
            let partial = local::files::part_path(&output);
            if partial.is_file() {
                fs::remove_file(partial)?;
            }
            let audio_partial = local::queue::audio_part_path(&output);
            if audio_partial.is_file() {
                fs::remove_file(audio_partial)?;
            }
        }
        let downloaded = fs::metadata(local::files::part_path(&output))
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        jobs.insert(
            filename.clone(),
            DownloadJob {
                phase: DownloadPhase::Queued,
                downloaded,
                total: None,
                error: None,
                source_url: Some(source_url.to_owned()),
                media_url: Some(resolved.media_url),
                audio_url,
                extract_audio,
                quality_label,
                quality_height,
            },
        );
    }
    local::queue::persist_download_jobs();
    local::queue::notify_transfer_state(true);
    workflows::downloads::schedule_download_worker(
        client.clone(),
        output_dir.to_path_buf(),
        filename.clone(),
    );

    eprintln!("Download queued for {}", output.display());
    Ok((output, DownloadOutcome::Started))
}

// This matches the highest adaptive transfer limit. Queued downloads retain
// only a task, never a waiting OS thread.
const DOWNLOAD_WORKERS: usize = 3;

struct DownloadTask {
    client: Client,
    output_dir: PathBuf,
    filename: String,
}

static DOWNLOAD_POOL: OnceLock<Result<mpsc::Sender<DownloadTask>, String>> = OnceLock::new();

fn download_pool() -> Result<&'static mpsc::Sender<DownloadTask>, String> {
    DOWNLOAD_POOL
        .get_or_init(|| {
            let (sender, receiver) = mpsc::channel::<DownloadTask>();
            let receiver = Arc::new(Mutex::new(receiver));
            for index in 0..DOWNLOAD_WORKERS {
                let receiver = Arc::clone(&receiver);
                thread::Builder::new()
                    .name(format!("download-{index}"))
                    .spawn(move || {
                        loop {
                            let task = receiver
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner())
                                .recv();
                            let Ok(task) = task else { break };
                            let result = catch_unwind(AssertUnwindSafe(|| {
                                run_download_worker(&task.client, &task.output_dir, &task.filename)
                            }))
                            .unwrap_or_else(|_| {
                                Err("download worker panicked; retry this item".to_owned())
                            });
                            finish_download_task(task, result);
                        }
                    })
                    .map_err(|error| format!("could not start download workers: {error}"))?;
            }
            Ok(sender)
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub(in super::super) fn schedule_download_worker(
    client: Client,
    output_dir: PathBuf,
    filename: String,
) {
    let sender = match download_pool() {
        Ok(sender) => sender,
        Err(error) => {
            local::queue::mark_download_failed(&filename, error);
            return;
        }
    };
    let mut scheduled = local::queue::scheduled_workers()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !scheduled.insert(filename.clone()) {
        return;
    }
    if sender
        .send(DownloadTask {
            client,
            output_dir,
            filename: filename.clone(),
        })
        .is_err()
    {
        scheduled.remove(&filename);
        drop(scheduled);
        local::queue::mark_download_failed(&filename, "download workers unavailable".to_owned());
    }
}

fn finish_download_task(task: DownloadTask, result: Result<(), String>) {
    // A read can fail during a network switch before the next chunk check.
    if result.is_err() {
        local::queue::defer_for_network(&task.filename);
    }
    // Serialize completion with scheduling. A resume that arrived while the old
    // worker was exiting must be requeued, without allowing two workers per file.
    let mut scheduled = local::queue::scheduled_workers()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut jobs = local::queue::download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut failed = false;
    if let Some(job) = jobs.get_mut(&task.filename)
        && let Err(error) = result
        && matches!(
            job.phase,
            DownloadPhase::Starting | DownloadPhase::Downloading
        )
    {
        eprintln!("background download failed for {}: {error}", task.filename);
        job.phase = DownloadPhase::Failed;
        job.error = Some(error);
        failed = true;
    }
    let retry = jobs
        .get(&task.filename)
        .is_some_and(|job| job.phase == DownloadPhase::Queued);
    drop(jobs);
    if retry {
        // The pool is initialized before a task can reach this point.
        if let Err(error) = download_pool()
            .expect("initialized download pool")
            .send(task)
        {
            scheduled.remove(&error.0.filename);
            drop(scheduled);
            local::queue::mark_download_failed(
                &error.0.filename,
                "download workers unavailable".to_owned(),
            );
            return;
        }
    } else {
        scheduled.remove(&task.filename);
    }
    drop(scheduled);
    if failed {
        local::queue::persist_download_jobs();
        local::queue::notify_transfer_state(true);
    }
}

struct DownloadSlot;

impl Drop for DownloadSlot {
    fn drop(&mut self) {
        local::queue::release_download_slot();
    }
}

pub(in super::super) fn run_download_worker(
    client: &Client,
    output_dir: &Path,
    filename: &str,
) -> Result<(), String> {
    if !local::queue::acquire_download_slot(filename) {
        return Ok(());
    }
    let _slot = DownloadSlot;
    workflows::downloads::run_download_worker_in_slot(client, output_dir, filename)
}

pub(in super::super) fn run_download_worker_in_slot(
    client: &Client,
    output_dir: &Path,
    filename: &str,
) -> Result<(), String> {
    if local::queue::defer_for_network(filename) {
        return Ok(());
    }
    let job =
        local::queue::download_job(filename).ok_or_else(|| "queue entry disappeared".to_owned())?;
    let media_url = if let Some(source_url) = job.source_url.as_deref().filter(|source_url| {
        matches!(
            local::sources::classify_url(source_url),
            local::sources::SourceUrl::YouTubeVideo { .. }
        )
    }) {
        if let Some(media_url) = job
            .media_url
            .clone()
            .filter(|url| local::youtube::url_is_fresh(url))
        {
            media_url
        } else {
            let candidate = external::youtube::resolve_candidate(source_url)
                .map_err(|error| error.to_string())?;
            let refreshed = candidate
                .qualities
                .iter()
                .filter(|quality| {
                    local::media::is_audio_filename(&quality.filename)
                        == local::media::is_audio_filename(filename)
                })
                .min_by_key(|quality| {
                    job.quality_height
                        .zip(quality.quality_height)
                        .map(|(wanted, actual)| wanted.abs_diff(actual))
                        .unwrap_or(0)
                })
                .cloned()
                .unwrap_or(candidate.resolved);
            if let Some(current) = local::queue::download_jobs()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .get_mut(filename)
            {
                current.media_url = Some(refreshed.media_url.clone());
                current.audio_url = refreshed.audio_url.clone();
                current.extract_audio = refreshed.extract_audio;
            }
            local::queue::persist_download_jobs();
            refreshed.media_url
        }
    } else {
        job.media_url
            .clone()
            .ok_or_else(|| "saved download has no media URL; submit the post again".to_owned())?
    };
    if local::queue::defer_for_network(filename) {
        return Ok(());
    }
    let audio_url = local::queue::download_job(filename).and_then(|job| job.audio_url);
    let output = output_dir.join(filename);
    if let Some(audio_url) = audio_url {
        return workflows::downloads::finish_adaptive_download(
            client, &media_url, &audio_url, &output, filename,
        );
    }
    let temporary = local::files::part_path(&output);
    let existing = fs::metadata(&temporary)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let (response, file, downloaded, total) =
        external::http::open_resumable_download(client, &media_url, &temporary, existing)?;

    local::queue::update_download_progress(filename, downloaded, total);
    external::http::finish_web_download(
        response, file, &temporary, &output, filename, downloaded, total,
    )
}

pub(in super::super) fn finish_adaptive_download(
    client: &Client,
    video_url: &str,
    audio_url: &str,
    output: &Path,
    filename: &str,
) -> Result<(), String> {
    let video_part = local::files::part_path(output);
    let audio_part = local::queue::audio_part_path(output);
    let Some(video_bytes) =
        external::http::download_adaptive_track(client, video_url, &video_part, filename, 0)?
    else {
        return Ok(());
    };
    let Some(audio_bytes) = external::http::download_adaptive_track(
        client,
        audio_url,
        &audio_part,
        filename,
        video_bytes,
    )?
    else {
        return Ok(());
    };
    let mux = MUX_HOOK
        .get()
        .ok_or_else(|| "adaptive YouTube muxing is available in the Android app".to_owned())?;
    mux(&video_part, &audio_part, output)?;
    fs::remove_file(&video_part).map_err(|error| error.to_string())?;
    fs::remove_file(&audio_part).map_err(|error| error.to_string())?;
    let downloaded = video_bytes.saturating_add(audio_bytes);
    local::storage::record_existing_file_fingerprint_async(
        output.to_path_buf(),
        filename.to_owned(),
    );
    let previous = local::queue::download_job(filename);
    let publish_error = PUBLISH_HOOK
        .get()
        .and_then(|publish| publish(output, filename).err());
    local::queue::set_download_job(
        filename,
        DownloadJob {
            phase: DownloadPhase::Ready,
            downloaded,
            total: Some(downloaded),
            error: publish_error,
            source_url: previous.as_ref().and_then(|job| job.source_url.clone()),
            media_url: previous.as_ref().and_then(|job| job.media_url.clone()),
            audio_url: previous.as_ref().and_then(|job| job.audio_url.clone()),
            extract_audio: previous.as_ref().is_some_and(|job| job.extract_audio),
            quality_label: previous.as_ref().and_then(|job| job.quality_label.clone()),
            quality_height: previous.and_then(|job| job.quality_height),
        },
    );
    Ok(())
}

pub(in super::super) fn initialize_download_queue(
    client: &Client,
    output_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    if local::runtime::inspection_mode() {
        return Ok(());
    }
    let _ = QUEUE_OUTPUT_DIR.set(output_dir.to_path_buf());
    let path = local::queue::queue_state_path(output_dir);
    let mut restored: HashMap<String, DownloadJob> = match fs::read(&path) {
        Ok(data) => serde_json::from_slice(&data).unwrap_or_default(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => HashMap::new(),
        Err(error) => return Err(error.into()),
    };
    restored.retain(|filename, _| local::media::valid_video_filename(filename));
    for (filename, job) in &mut restored {
        let output = output_dir.join(filename);
        if local::files::is_complete_download(&output)? {
            let length = fs::metadata(output)?.len();
            job.phase = DownloadPhase::Ready;
            job.downloaded = length;
            job.total = Some(length);
            continue;
        }
        job.downloaded = fs::metadata(local::files::part_path(&output))
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if matches!(
            job.phase,
            DownloadPhase::Starting | DownloadPhase::Downloading | DownloadPhase::Queued
        ) {
            job.phase = DownloadPhase::Queued;
        }
    }
    *local::queue::download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = restored;
    local::queue::persist_download_jobs();
    local::queue::notify_transfer_state(true);

    let resumable = local::queue::download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .filter(|(_, job)| job.phase == DownloadPhase::Queued && job.media_url.is_some())
        .map(|(filename, _)| filename.clone())
        .collect::<Vec<_>>();
    for filename in resumable {
        workflows::downloads::schedule_download_worker(
            client.clone(),
            output_dir.to_path_buf(),
            filename,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn task(filename: &str, phase: DownloadPhase) -> DownloadTask {
        local::queue::set_download_job(
            filename,
            DownloadJob {
                phase,
                downloaded: 0,
                total: None,
                error: None,
                source_url: None,
                media_url: Some("http://127.0.0.1:0/unavailable".to_owned()),
                audio_url: None,
                extract_audio: false,
                quality_label: None,
                quality_height: None,
            },
        );
        local::queue::scheduled_workers()
            .lock()
            .unwrap()
            .insert(filename.to_owned());
        DownloadTask {
            client: Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap(),
            output_dir: std::env::temp_dir(),
            filename: filename.to_owned(),
        }
    }

    #[test]
    fn resume_during_worker_exit_is_requeued_even_after_old_request_failed() {
        download_pool().unwrap();
        let filename = "youtube-poolrace001.mp4";
        // Resume sets Queued while the original worker is still scheduled.
        let task = task(filename, DownloadPhase::Queued);
        finish_download_task(task, Err("old interrupted request".to_owned()));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if !local::queue::scheduled_workers()
                .lock()
                .unwrap()
                .contains(filename)
            {
                break;
            }
            assert!(Instant::now() < deadline, "resumed item stranded in queue");
            thread::sleep(Duration::from_millis(10));
        }
        let job = local::queue::download_jobs()
            .lock()
            .unwrap()
            .remove(filename)
            .unwrap();
        assert_eq!(job.phase, DownloadPhase::Failed);
        assert_ne!(job.error.as_deref(), Some("old interrupted request"));
    }

    #[test]
    fn worker_exit_preserves_pause_and_cancel_even_after_network_error() {
        for (filename, phase) in [
            ("youtube-poolpause01.mp4", DownloadPhase::Paused),
            ("youtube-poolcancel1.mp4", DownloadPhase::Cancelled),
        ] {
            finish_download_task(task(filename, phase), Err("interrupted request".to_owned()));
            assert!(
                !local::queue::scheduled_workers()
                    .lock()
                    .unwrap()
                    .contains(filename)
            );
            let job = local::queue::download_jobs()
                .lock()
                .unwrap()
                .remove(filename)
                .unwrap();
            assert_eq!(job.phase, phase);
            assert!(job.error.is_none());
        }
    }
}
