//! Local queue persistence, concurrency gates, progress, and notifications.

use super::super::local;
use super::super::local::runtime::{EVENT_HOOK, EVENT_REVISION, TRANSFER_HOOK, TransferSummary};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tiny_http::{Request, Response, StatusCode};

pub(in super::super) static DOWNLOAD_JOBS: OnceLock<Mutex<HashMap<String, DownloadJob>>> =
    OnceLock::new();

pub(in super::super) static QUEUE_OUTPUT_DIR: OnceLock<PathBuf> = OnceLock::new();

pub(in super::super) static DOWNLOAD_GATE: OnceLock<(Mutex<usize>, Condvar)> = OnceLock::new();

pub(in super::super) static SCHEDULED_WORKERS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

pub(in super::super) static LAST_TRANSFER_NOTICE: OnceLock<Mutex<Option<Instant>>> =
    OnceLock::new();

pub(in super::super) static DOWNLOAD_PROGRESS_SIGNAL: OnceLock<(Mutex<u64>, Condvar)> =
    OnceLock::new();

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(in super::super) enum DownloadPhase {
    Queued,
    Starting,
    Downloading,
    Paused,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(in super::super) struct DownloadJob {
    pub(in super::super) phase: DownloadPhase,
    pub(in super::super) downloaded: u64,
    pub(in super::super) total: Option<u64>,
    pub(in super::super) error: Option<String>,
    #[serde(default)]
    pub(in super::super) source_url: Option<String>,
    #[serde(default)]
    pub(in super::super) media_url: Option<String>,
    #[serde(default)]
    pub(in super::super) audio_url: Option<String>,
    #[serde(default)]
    pub(in super::super) extract_audio: bool,
    #[serde(default)]
    pub(in super::super) quality_label: Option<String>,
    #[serde(default)]
    pub(in super::super) quality_height: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum DownloadOutcome {
    Started,
    InProgress,
    Duplicate,
}

pub(in super::super) fn download_jobs() -> &'static Mutex<HashMap<String, DownloadJob>> {
    DOWNLOAD_JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(in super::super) fn scheduled_workers() -> &'static Mutex<HashSet<String>> {
    SCHEDULED_WORKERS.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(in super::super) fn download_gate() -> &'static (Mutex<usize>, Condvar) {
    DOWNLOAD_GATE.get_or_init(|| (Mutex::new(0), Condvar::new()))
}

pub(in super::super) fn respond_queue_page(
    request: Request,
    errors: &[String],
) -> Result<(), Box<dyn Error>> {
    let mut jobs = local::queue::download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .map(|(filename, job)| (filename.clone(), job.clone()))
        .collect::<Vec<_>>();
    jobs.sort_unstable_by(|left, right| right.0.cmp(&left.0));
    let rows = if jobs.is_empty() {
        r#"<div class="empty">The queue is empty. Paste links on the home screen to begin.</div>"#
            .to_owned()
    } else {
        jobs.into_iter()
            .map(|(filename, job)| {
                let phase_name = local::queue::download_phase_name(job.phase);
                let phase = local::queue::phase_label(job.phase);
                let percent = job
                    .total
                    .filter(|total| *total > 0)
                    .map(|total| (job.downloaded.saturating_mul(100) / total).min(100));
                let progress = percent
                    .map(|value| format!(r#"<i style="width:{value}%"></i>"#))
                    .unwrap_or_default();
                let size = match job.total {
                    Some(total) => format!(
                        "{} / {}",
                        local::format::format_bytes(job.downloaded),
                        local::format::format_bytes(total)
                    ),
                    None => local::format::format_bytes(job.downloaded),
                };
                let quality = job
                    .quality_label
                    .as_deref()
                    .map(|quality| {
                        format!(r#"<span class="quality">{}</span>"#, local::html::escape_html(quality))
                    })
                    .unwrap_or_default();
                let actions = match job.phase {
                    DownloadPhase::Queued
                    | DownloadPhase::Starting
                    | DownloadPhase::Downloading => format!(
                        r#"<a href="/watch/{filename}">Play</a><a href="/queue/action?file={filename}&amp;action=pause">Pause</a><a class="danger" href="/queue/action?file={filename}&amp;action=cancel">Cancel</a>"#
                    ),
                    DownloadPhase::Paused | DownloadPhase::Failed => format!(
                        r#"<a href="/queue/action?file={filename}&amp;action=resume">Resume</a><a class="danger" href="/queue/action?file={filename}&amp;action=cancel">Cancel</a>"#
                    ),
                    DownloadPhase::Ready => {
                        format!(r#"<a href="/watch/{filename}">Open player</a>"#)
                    }
                    DownloadPhase::Cancelled => format!(
                        r#"<a href="/queue/action?file={filename}&amp;action=resume">Start again</a>"#
                    ),
                };
                let error = job
                    .error
                    .filter(|error| !error.is_empty())
                    .map(|error| format!(r#"<p class="error">{}</p>"#, local::html::escape_html(&error)))
                    .unwrap_or_default();
                format!(
                    r#"<article data-filename="{}" data-phase="{phase_name}"><div class="row"><div class="info"><span class="phase">{phase}</span><code>{}</code><span class="size">{size}</span>{quality}</div><nav>{actions}</nav></div><div class="progress">{progress}</div>{error}</article>"#,
                    local::html::escape_html(&filename),
                    local::html::escape_html(&filename)
                )
            })
            .collect::<String>()
    };
    let errors = errors
        .iter()
        .map(|error| format!(r#"<p class="batch-error">{error}</p>"#))
        .collect::<String>();
    let body = {
        let queue_script = &(local::live_events::QUEUE_SCRIPT);
        let dev_reload = &(local::dev::dev_reload_script());
        format!(
            include_str!("../../assets/html/queue.html"),
            queue_script = queue_script,
            dev_reload = dev_reload,
            page_css = include_str!("../../assets/css/queue.css"),
            errors = errors,
            rows = rows
        )
    };
    let response = Response::from_string(local::web_assets::decorate_app_html(body))
        .with_status_code(StatusCode(200))
        .with_header(local::html::header(
            "Content-Type",
            "text/html; charset=utf-8",
        ))
        .with_header(local::html::html_csp())
        .with_header(local::html::header("Cache-Control", "no-store"))
        .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

pub(in super::super) fn acquire_download_slot(filename: &str) -> bool {
    let (active, available) = local::queue::download_gate();
    let mut active = active
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loop {
        let mut jobs = local::queue::download_jobs()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let phase = jobs.get(filename).map(|job| job.phase);
        if !matches!(phase, Some(DownloadPhase::Queued | DownloadPhase::Starting)) {
            return false;
        }
        if local::runtime::download_network_reason().is_none()
            && *active < local::runtime::adaptive_download_limit()
        {
            *active += 1;
            jobs.get_mut(filename).expect("checked queue entry").phase = DownloadPhase::Starting;
            return true;
        }
        drop(jobs);
        active = available
            .wait_timeout(active, Duration::from_millis(250))
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .0;
    }
}

pub(in super::super) fn release_download_slot() {
    let (active, available) = local::queue::download_gate();
    let mut active = active
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *active = active.saturating_sub(1);
    available.notify_all();
}

// Return active transfers to Queued, preserving partial files and user pauses.
pub(in super::super) fn defer_for_network(filename: &str) -> bool {
    if local::runtime::download_network_reason().is_none() {
        return false;
    }
    let mut jobs = download_jobs().lock().unwrap_or_else(|p| p.into_inner());
    let mut deferred = false;
    if let Some(job) = jobs.get_mut(filename)
        && matches!(
            job.phase,
            DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading
        )
    {
        job.phase = DownloadPhase::Queued;
        job.error = None;
        deferred = true;
    }
    drop(jobs);
    if deferred {
        persist_download_jobs();
        notify_transfer_state(true);
    }
    deferred
}

pub(in super::super) fn phase_label(phase: DownloadPhase) -> &'static str {
    if matches!(
        phase,
        DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading
    ) && let Some(reason) = local::runtime::download_network_reason()
    {
        return reason;
    }
    match phase {
        DownloadPhase::Queued => "Queued",
        DownloadPhase::Starting => "Starting",
        DownloadPhase::Downloading => "Downloading",
        DownloadPhase::Paused => "Paused",
        DownloadPhase::Ready => "Ready",
        DownloadPhase::Failed => "Needs retry",
        DownloadPhase::Cancelled => "Cancelled",
    }
}

pub(in super::super) fn audio_part_path(output: &Path) -> PathBuf {
    let mut name = output.as_os_str().to_owned();
    name.push(".audio.part");
    PathBuf::from(name)
}

pub(in super::super) fn set_download_job(filename: &str, job: DownloadJob) {
    local::queue::download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(filename.to_owned(), job);
    local::queue::persist_download_jobs();
    local::queue::notify_transfer_state(true);
}

pub(in super::super) fn update_download_progress(
    filename: &str,
    downloaded: u64,
    total: Option<u64>,
) {
    if let Some(job) = local::queue::download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get_mut(filename)
    {
        if matches!(job.phase, DownloadPhase::Paused | DownloadPhase::Cancelled) {
            return;
        }
        job.phase = DownloadPhase::Downloading;
        job.downloaded = downloaded;
        job.total = total;
    }
    local::queue::notify_transfer_state(false);
}

pub(in super::super) fn signal_growing_media() {
    let (generation, changed) =
        DOWNLOAD_PROGRESS_SIGNAL.get_or_init(|| (Mutex::new(0), Condvar::new()));
    let mut generation = generation
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *generation = generation.wrapping_add(1);
    changed.notify_all();
}

pub(in super::super) fn notify_transfer_state(force: bool) {
    local::queue::signal_growing_media();
    if TRANSFER_HOOK.get().is_none() && EVENT_HOOK.get().is_none() {
        return;
    }
    let now = Instant::now();
    if !force {
        let mut last = LAST_TRANSFER_NOTICE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if last.is_some_and(|instant| now.duration_since(instant) < Duration::from_millis(750)) {
            return;
        }
        *last = Some(now);
    }
    let summary = local::queue::download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .values()
        .filter(|job| {
            matches!(
                job.phase,
                DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading
            )
        })
        .fold(
            TransferSummary {
                count: 0,
                downloaded: 0,
                total: 0,
            },
            |mut summary, job| {
                summary.count = summary.count.saturating_add(1);
                summary.downloaded = summary.downloaded.saturating_add(job.downloaded);
                summary.total = summary.total.saturating_add(job.total.unwrap_or(0));
                summary
            },
        );
    if let Some(hook) = TRANSFER_HOOK.get()
        && let Err(error) = hook(summary)
    {
        eprintln!("Android transfer notification warning: {error}");
    }
    if let Some(hook) = EVENT_HOOK.get() {
        let event = serde_json::json!({
            "type": "queue",
            "version": 1,
            "revision": EVENT_REVISION.fetch_add(1, Ordering::Relaxed) + 1,
            "active": summary.count,
            "downloaded": summary.downloaded,
            "total": summary.total,
        })
        .to_string();
        if let Err(error) = hook(&event) {
            eprintln!("Android WebView event warning: {error}");
        }
    }
}

pub(in super::super) fn mark_download_failed(filename: &str, error: String) {
    let previous = local::queue::download_job(filename);
    let downloaded = QUEUE_OUTPUT_DIR
        .get()
        .and_then(|directory| fs::metadata(local::files::part_path(&directory.join(filename))).ok())
        .map(|metadata| metadata.len())
        .or_else(|| previous.as_ref().map(|job| job.downloaded))
        .unwrap_or(0);
    local::queue::set_download_job(
        filename,
        DownloadJob {
            phase: DownloadPhase::Failed,
            downloaded,
            total: previous.as_ref().and_then(|job| job.total),
            error: Some(error),
            source_url: previous.as_ref().and_then(|job| job.source_url.clone()),
            media_url: previous.as_ref().and_then(|job| job.media_url.clone()),
            audio_url: previous.as_ref().and_then(|job| job.audio_url.clone()),
            extract_audio: previous.as_ref().is_some_and(|job| job.extract_audio),
            quality_label: previous.as_ref().and_then(|job| job.quality_label.clone()),
            quality_height: previous.and_then(|job| job.quality_height),
        },
    );
}

pub(in super::super) fn download_job(filename: &str) -> Option<DownloadJob> {
    local::queue::download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(filename)
        .cloned()
}

pub(in super::super) fn queue_state_path(output_dir: &Path) -> PathBuf {
    output_dir.join(".queue.json")
}

pub(in super::super) fn persist_download_jobs() {
    if local::runtime::inspection_mode() {
        return;
    }
    let Some(output_dir) = QUEUE_OUTPUT_DIR.get() else {
        return;
    };
    let jobs = local::queue::download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let Ok(data) = serde_json::to_vec_pretty(&jobs) else {
        return;
    };
    let destination = local::queue::queue_state_path(output_dir);
    let temporary = output_dir.join(".queue.json.part");
    if fs::write(&temporary, data).is_ok() {
        let _ = fs::rename(temporary, destination);
    }
}

pub(in super::super) fn download_phase_name(phase: DownloadPhase) -> &'static str {
    match phase {
        DownloadPhase::Queued => "queued",
        DownloadPhase::Starting => "starting",
        DownloadPhase::Downloading => "downloading",
        DownloadPhase::Paused => "paused",
        DownloadPhase::Ready => "ready",
        DownloadPhase::Failed => "failed",
        DownloadPhase::Cancelled => "cancelled",
    }
}

pub(in super::super) fn app_state_job(filename: &str, job: &DownloadJob) -> serde_json::Value {
    serde_json::json!({
        "filename": filename,
        "phase": local::queue::download_phase_name(job.phase),
        "phaseLabel": local::queue::phase_label(job.phase),
        "downloaded": job.downloaded,
        "total": job.total,
        "sizeLabel": match job.total {
            Some(total) => format!("{} / {}", local::format::format_bytes(job.downloaded), local::format::format_bytes(total)),
            None => local::format::format_bytes(job.downloaded),
        },
        "quality": job.quality_label,
        "height": job.quality_height,
        "source": job.source_url,
        "error": job.error,
    })
}
