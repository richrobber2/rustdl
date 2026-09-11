//! local / storage.

use super::super::local;
use super::super::local::queue::DownloadPhase;
use super::super::local::runtime::{DELETE_HOOK, WATCHED_HOOK};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fs, io, thread};
use tiny_http::{Request, Response, StatusCode};

pub(in super::super) static FINGERPRINT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug)]
pub(in super::super) struct StoredVideo {
    pub(in super::super) filename: String,
    pub(in super::super) bytes: u64,
    pub(in super::super) watched: bool,
    pub(in super::super) duplicate: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(in super::super) struct StoredFingerprint {
    pub(in super::super) bytes: u64,
    pub(in super::super) modified_nanos: u64,
    pub(in super::super) blake3: String,
}

#[derive(Clone, Debug)]
pub(in super::super) struct PartialFile {
    pub(in super::super) filename: String,
    pub(in super::super) path: PathBuf,
    pub(in super::super) bytes: u64,
    pub(in super::super) stale: bool,
}

#[derive(Debug)]
pub(in super::super) struct StorageSnapshot {
    pub(in super::super) videos: Vec<StoredVideo>,
    pub(in super::super) partials: Vec<PartialFile>,
    pub(in super::super) video_bytes: u64,
    pub(in super::super) partial_bytes: u64,
    pub(in super::super) thumbnail_bytes: u64,
    pub(in super::super) metadata_bytes: u64,
}

pub(in super::super) fn storage_snapshot(output_dir: &Path) -> io::Result<StorageSnapshot> {
    let watched = WATCHED_HOOK
        .get()
        .and_then(|hook| hook().ok())
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();
    let now = SystemTime::now();
    let mut videos = Vec::new();
    let mut partials = Vec::new();
    if output_dir.is_dir() {
        for entry in fs::read_dir(output_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let filename = entry.file_name().to_string_lossy().into_owned();
            let metadata = entry.metadata()?;
            if local::media::valid_video_filename(&filename) {
                videos.push(StoredVideo {
                    watched: watched.contains(&filename),
                    filename,
                    bytes: metadata.len(),
                    duplicate: false,
                });
                continue;
            }
            let Some(base) = filename
                .strip_suffix(".audio.part")
                .or_else(|| filename.strip_suffix(".part"))
            else {
                continue;
            };
            if !local::media::valid_video_filename(base) {
                continue;
            }
            let phase = local::queue::download_job(base).map(|job| job.phase);
            let active = matches!(
                phase,
                Some(DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading)
            );
            let age = metadata
                .modified()
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .unwrap_or_default();
            let stale = !active
                && (age >= Duration::from_secs(24 * 60 * 60)
                    || matches!(
                        phase,
                        Some(DownloadPhase::Failed | DownloadPhase::Cancelled)
                    ));
            partials.push(PartialFile {
                filename: base.to_owned(),
                path: entry.path(),
                bytes: metadata.len(),
                stale,
            });
        }
    }
    local::storage::mark_duplicate_videos(output_dir, &mut videos)?;
    videos.sort_unstable_by(|left, right| right.filename.cmp(&left.filename));
    partials.sort_unstable_by(|left, right| right.filename.cmp(&left.filename));
    let thumbnail_bytes = local::storage::directory_file_bytes(&output_dir.join(".thumbnails"))?;
    let metadata_bytes = [
        local::queue::queue_state_path(output_dir),
        local::gallery::playlist_memberships_path(output_dir),
        local::storage::fingerprints_path(output_dir),
    ]
    .iter()
    .filter_map(|path| fs::metadata(path).ok())
    .map(|metadata| metadata.len())
    .sum();
    Ok(StorageSnapshot {
        video_bytes: videos.iter().map(|video| video.bytes).sum(),
        partial_bytes: partials.iter().map(|partial| partial.bytes).sum(),
        videos,
        partials,
        thumbnail_bytes,
        metadata_bytes,
    })
}

pub(in super::super) fn directory_file_bytes(directory: &Path) -> io::Result<u64> {
    if !directory.is_dir() {
        return Ok(0);
    }
    let mut total = 0_u64;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            total = total.saturating_add(entry.metadata()?.len());
        }
    }
    Ok(total)
}

pub(in super::super) fn mark_duplicate_videos(
    output_dir: &Path,
    videos: &mut [StoredVideo],
) -> io::Result<()> {
    let _guard = FINGERPRINT_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut stored = local::storage::load_fingerprints(output_dir)?;
    let mut changed = false;
    let mut by_size: HashMap<u64, Vec<usize>> = HashMap::new();
    for (index, video) in videos.iter().enumerate() {
        by_size.entry(video.bytes).or_default().push(index);
    }
    for indexes in by_size.values().filter(|indexes| indexes.len() > 1) {
        let mut fingerprints: HashMap<String, Vec<usize>> = HashMap::new();
        for index in indexes {
            let filename = &videos[*index].filename;
            let path = output_dir.join(filename);
            let (bytes, modified_nanos) = local::storage::file_signature(&path)?;
            let fingerprint = match stored.get(filename) {
                Some(value)
                    if value.bytes == bytes
                        && value.modified_nanos == modified_nanos
                        && local::storage::valid_blake3_hash(&value.blake3) =>
                {
                    value.blake3.clone()
                }
                _ => {
                    let blake3 = local::files::blake3_file(&path)
                        .map_err(|error| io::Error::other(error.to_string()))?;
                    stored.insert(
                        filename.clone(),
                        StoredFingerprint {
                            bytes,
                            modified_nanos,
                            blake3: blake3.clone(),
                        },
                    );
                    changed = true;
                    blake3
                }
            };
            fingerprints.entry(fingerprint).or_default().push(*index);
        }
        for indexes in fingerprints.values().filter(|indexes| indexes.len() > 1) {
            for index in indexes {
                videos[*index].duplicate = true;
            }
        }
    }
    if changed {
        local::storage::persist_fingerprints(output_dir, &stored)?;
    }
    Ok(())
}

pub(in super::super) fn fingerprints_path(output_dir: &Path) -> PathBuf {
    output_dir.join(".fingerprints.json")
}

pub(in super::super) fn load_fingerprints(
    output_dir: &Path,
) -> io::Result<HashMap<String, StoredFingerprint>> {
    match fs::read(local::storage::fingerprints_path(output_dir)) {
        Ok(data) => Ok(serde_json::from_slice(&data).unwrap_or_default()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(error) => Err(error),
    }
}

pub(in super::super) fn persist_fingerprints(
    output_dir: &Path,
    fingerprints: &HashMap<String, StoredFingerprint>,
) -> io::Result<()> {
    let data = serde_json::to_vec(fingerprints).map_err(io::Error::other)?;
    let temporary = output_dir.join(".fingerprints.json.part");
    fs::write(&temporary, data)?;
    fs::rename(temporary, local::storage::fingerprints_path(output_dir))
}

pub(in super::super) fn file_signature(path: &Path) -> io::Result<(u64, u64)> {
    let metadata = fs::metadata(path)?;
    let modified_nanos = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .try_into()
        .unwrap_or(u64::MAX);
    Ok((metadata.len(), modified_nanos))
}

pub(in super::super) fn valid_blake3_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(in super::super) fn record_file_fingerprint(
    output_dir: &Path,
    filename: &str,
    blake3: &str,
) -> io::Result<()> {
    if !local::media::valid_video_filename(filename) || !local::storage::valid_blake3_hash(blake3) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid media fingerprint",
        ));
    }
    let (bytes, modified_nanos) = local::storage::file_signature(&output_dir.join(filename))?;
    let _guard = FINGERPRINT_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut fingerprints = local::storage::load_fingerprints(output_dir)?;
    fingerprints.insert(
        filename.to_owned(),
        StoredFingerprint {
            bytes,
            modified_nanos,
            blake3: blake3.to_ascii_lowercase(),
        },
    );
    local::storage::persist_fingerprints(output_dir, &fingerprints)
}

pub(in super::super) fn record_existing_file_fingerprint_async(output: PathBuf, filename: String) {
    thread::spawn(move || {
        let result = (|| -> Result<(), Box<dyn Error>> {
            let blake3 = local::files::blake3_file(&output)?;
            let output_dir = output
                .parent()
                .ok_or("media file has no parent directory")?;
            local::storage::record_file_fingerprint(output_dir, &filename, &blake3)?;
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("could not cache media fingerprint: {error}");
        }
    });
}

pub(in super::super) fn respond_storage_page(
    request: Request,
    output_dir: &Path,
    notice: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let snapshot = local::storage::storage_snapshot(output_dir)?;
    let total = snapshot
        .video_bytes
        .saturating_add(snapshot.partial_bytes)
        .saturating_add(snapshot.thumbnail_bytes)
        .saturating_add(snapshot.metadata_bytes);
    let watched_count = snapshot.videos.iter().filter(|video| video.watched).count();
    let duplicate_count = snapshot
        .videos
        .iter()
        .filter(|video| video.duplicate)
        .count();
    let stale_count = snapshot
        .partials
        .iter()
        .filter(|partial| partial.stale)
        .count();
    let stale_bytes = snapshot
        .partials
        .iter()
        .filter(|partial| partial.stale)
        .map(|partial| partial.bytes)
        .sum::<u64>();
    let rows = if snapshot.videos.is_empty() {
        r#"<div class="empty">No completed videos are stored in RustDL.</div>"#.to_owned()
    } else {
        snapshot
            .videos
            .iter()
            .map(|video| {
                let mut badges = String::new();
                if video.watched {
                    badges.push_str(r#"<span class="badge">Watched</span>"#);
                }
                if video.duplicate {
                    badges.push_str(r#"<span class="badge warning">Duplicate content</span>"#);
                }
                format!(
                    r#"<article><div class="file"><code>{}</code><span>{} {badges}</span></div><div class="actions"><a href="/watch/{}">Play</a><a class="danger" href="/storage/confirm?action=delete&amp;file={}">Delete</a></div></article>"#,
                    local::html::escape_html(&video.filename),
                    local::format::format_bytes(video.bytes),
                    video.filename,
                    video.filename
                )
            })
            .collect::<String>()
    };
    let partial_rows = snapshot
        .partials
        .iter()
        .map(|partial| {
            let state = if partial.stale {
                "Stale"
            } else {
                "Retained for resume"
            };
            format!(
                r#"<li><code>{}</code><span>{} · {state}</span></li>"#,
                local::html::escape_html(&partial.filename),
                local::format::format_bytes(partial.bytes)
            )
        })
        .collect::<String>();
    let watched_action = if watched_count > 0 {
        format!(
            r#"<a class="danger" href="/storage/confirm?action=watched">Remove {watched_count} watched</a>"#
        )
    } else {
        String::new()
    };
    let stale_action = if stale_count > 0 {
        format!(
            r#"<a href="/storage/confirm?action=stale">Clean {stale_count} stale partials · {}</a>"#,
            local::format::format_bytes(stale_bytes)
        )
    } else {
        String::new()
    };
    let thumbnail_action = if snapshot.thumbnail_bytes > 0 {
        format!(
            r#"<a href="/storage/confirm?action=thumbnails">Clear thumbnail cache · {}</a>"#,
            local::format::format_bytes(snapshot.thumbnail_bytes)
        )
    } else {
        String::new()
    };
    let notice = notice
        .map(|notice| {
            format!(
                r#"<div class="notice">{}</div>"#,
                local::html::escape_html(notice)
            )
        })
        .unwrap_or_default();
    let partials = if partial_rows.is_empty() {
        String::new()
    } else {
        format!(
            r#"<section class="panel"><h2>Partial downloads</h2><ul>{partial_rows}</ul></section>"#
        )
    };
    let body = {
        let total_size = &(local::format::format_bytes(total));
        let video_count = &(snapshot.videos.len());
        let video_size = &(local::format::format_bytes(snapshot.video_bytes));
        let partial_count = &(snapshot.partials.len());
        let partial_size = &(local::format::format_bytes(snapshot.partial_bytes));
        let dev_reload = &(local::dev::dev_reload_script());
        format!(
            include_str!("../../assets/html/storage.html"),
            STORAGE_CSS = STORAGE_CSS,
            total_size = total_size,
            video_count = video_count,
            video_size = video_size,
            partial_count = partial_count,
            partial_size = partial_size,
            dev_reload = dev_reload,
            duplicate_count = duplicate_count,
            notice = notice,
            partials = partials,
            rows = rows,
            stale_action = stale_action,
            thumbnail_action = thumbnail_action,
            watched_action = watched_action,
            watched_count = watched_count
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

pub(in super::super) const STORAGE_CSS: &str = include_str!("../../assets/css/storage.css");

pub(in super::super) fn respond_storage_confirmation(
    request: Request,
    output_dir: &Path,
    action: Option<&str>,
    filename: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let snapshot = local::storage::storage_snapshot(output_dir)?;
    let (heading, detail, hidden_file) = match action {
        Some("delete") => {
            let filename = filename.filter(|value| local::media::valid_video_filename(value));
            let Some(filename) = filename.filter(|value| output_dir.join(value).is_file()) else {
                return local::html::respond_text(request, 404, "Video not found");
            };
            (
                "Delete this video?",
                format!(
                    "This permanently removes {} from RustDL and Android Downloads.",
                    local::html::escape_html(filename)
                ),
                format!(
                    r#"<input type="hidden" name="file" value="{}">"#,
                    local::html::escape_html(filename)
                ),
            )
        }
        Some("watched") => {
            let count = snapshot.videos.iter().filter(|video| video.watched).count();
            if count == 0 {
                return local::html::respond_text(request, 404, "No watched videos to remove");
            }
            (
                "Remove watched videos?",
                format!(
                    "This permanently removes {count} completed video(s) from RustDL and Android Downloads."
                ),
                String::new(),
            )
        }
        Some("stale") => {
            let count = snapshot
                .partials
                .iter()
                .filter(|partial| partial.stale)
                .count();
            if count == 0 {
                return local::html::respond_text(request, 404, "No stale partials to remove");
            }
            (
                "Clean stale partials?",
                format!(
                    "This removes {count} incomplete transfer file(s). Completed videos are untouched."
                ),
                String::new(),
            )
        }
        Some("thumbnails") if snapshot.thumbnail_bytes > 0 => (
            "Clear thumbnail cache?",
            "Poster images will be removed and generated again when needed.".to_owned(),
            String::new(),
        ),
        _ => return local::html::respond_text(request, 400, "Invalid storage action"),
    };
    let action = action.unwrap_or_default();
    let token = local::security::action_token();
    let body = format!(
        include_str!("../../assets/html/storage-confirm.html"),
        STORAGE_CSS = STORAGE_CSS,
        action = action,
        detail = detail,
        heading = heading,
        hidden_file = hidden_file,
        token = token
    );
    let response = Response::from_string(local::web_assets::decorate_app_html(body))
        .with_status_code(StatusCode(200))
        .with_header(local::html::header(
            "Content-Type",
            "text/html; charset=utf-8",
        ))
        .with_header(local::html::html_csp())
        .with_header(local::html::header("Cache-Control", "no-store"));
    request.respond(response)?;
    Ok(())
}

pub(in super::super) fn respond_storage_action(
    mut request: Request,
    output_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut body = String::new();
    request
        .as_reader()
        .take(16 * 1024)
        .read_to_string(&mut body)?;
    let form = Url::parse(&format!("http://localhost/?{body}"))?;
    let value = |key: &str| {
        form.query_pairs()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.into_owned())
    };
    if value("token").as_deref() != Some(local::security::action_token()) {
        return local::html::respond_text(request, 403, "Storage action expired");
    }
    let action = value("action").unwrap_or_default();
    let result = (|| -> Result<String, Box<dyn Error>> {
        let notice = match action.as_str() {
            "delete" => {
                let filename = value("file").ok_or("missing video filename")?;
                local::storage::delete_stored_video(output_dir, &filename)?;
                format!("Removed {filename} from RustDL and Android Downloads.")
            }
            "watched" => {
                let watched_hook = WATCHED_HOOK
                    .get()
                    .ok_or("watched cleanup is only available in the Android app")?;
                let watched = watched_hook()?;
                let mut removed = 0;
                for filename in watched {
                    if local::media::valid_video_filename(&filename)
                        && output_dir.join(&filename).is_file()
                    {
                        local::storage::delete_stored_video(output_dir, &filename)?;
                        removed += 1;
                    }
                }
                format!("Removed {removed} watched video(s).")
            }
            "stale" => {
                let snapshot = local::storage::storage_snapshot(output_dir)?;
                let stale = snapshot
                    .partials
                    .into_iter()
                    .filter(|partial| partial.stale)
                    .collect::<Vec<_>>();
                for partial in &stale {
                    if partial.path.is_file() {
                        fs::remove_file(&partial.path)?;
                    }
                    let output = output_dir.join(&partial.filename);
                    for companion in [
                        local::files::part_path(&output),
                        local::queue::audio_part_path(&output),
                    ] {
                        if companion.is_file() {
                            fs::remove_file(companion)?;
                        }
                    }
                    local::queue::download_jobs()
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .remove(&partial.filename);
                }
                local::queue::persist_download_jobs();
                format!("Removed {} stale partial download(s).", stale.len())
            }
            "thumbnails" => {
                let directory = output_dir.join(".thumbnails");
                let mut removed = 0;
                if directory.is_dir() {
                    for entry in fs::read_dir(directory)? {
                        let entry = entry?;
                        if entry.file_type()?.is_file() {
                            fs::remove_file(entry.path())?;
                            removed += 1;
                        }
                    }
                }
                format!("Cleared {removed} cached thumbnail(s).")
            }
            _ => return Err("invalid storage action".into()),
        };
        Ok(notice)
    })();
    match result {
        Ok(notice) => local::storage::respond_storage_page(request, output_dir, Some(&notice)),
        Err(error) => local::html::respond_text(request, 422, &format!("Cleanup failed: {error}")),
    }
}

pub(in super::super) fn delete_stored_video(
    output_dir: &Path,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    if !local::media::valid_video_filename(filename) {
        return Err("invalid video filename".into());
    }
    if local::queue::download_job(filename).is_some_and(|job| {
        matches!(
            job.phase,
            DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading
        )
    }) {
        return Err("pause or cancel the active download before deleting it".into());
    }
    if let Some(delete) = DELETE_HOOK.get() {
        delete(filename).map_err(|error| format!("Android Downloads deletion failed: {error}"))?;
    }
    for path in [
        output_dir.join(filename),
        local::files::part_path(&output_dir.join(filename)),
        output_dir
            .join(".thumbnails")
            .join(format!("{filename}.jpg")),
        output_dir
            .join(".thumbnails")
            .join(format!("{filename}.jpg.part")),
    ] {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    local::queue::download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(filename);
    local::queue::persist_download_jobs();
    Ok(())
}
