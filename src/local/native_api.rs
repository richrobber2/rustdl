//! Typed, in-process UI data. No HTTP or HTML parsing for native queue controls.
use super::super::{external, workflows};
use super::queue::{DownloadJob, DownloadPhase, QUEUE_OUTPUT_DIR};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static QUEUE_HANDLES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn handles() -> &'static Mutex<HashMap<String, String>> {
    QUEUE_HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn queue_row(filename: &str, job: &DownloadJob, id: &str, privacy: bool) -> Value {
    let actions: &[&str] = match job.phase {
        DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading => {
            &["pause", "cancel"]
        }
        DownloadPhase::Paused | DownloadPhase::Failed => &["resume", "cancel"],
        DownloadPhase::Cancelled => &["resume"],
        DownloadPhase::Ready => &[],
    };
    json!({
        "id": id,
        "title": if privacy { "Downloaded media" } else { filename },
        "phase": super::queue::download_phase_name(job.phase),
        "phaseLabel": super::queue::phase_label(job.phase),
        "downloaded": job.downloaded,
        "total": job.total.unwrap_or(0),
        "actions": actions,
        "canPlay": matches!(job.phase, DownloadPhase::Ready | DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading | DownloadPhase::Paused),
        // Provider error messages can contain paths; keep the native surface generic.
        "issueDetail": if privacy { None } else { job.error.clone() },
        "quality": if privacy { None } else { job.quality_label.clone() },
        "issue": job.error.as_ref().is_some_and(|error| !error.is_empty()),
    })
}

pub(crate) fn queue_snapshot(offset: usize, privacy: bool) -> String {
    let jobs = super::queue::download_jobs()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let mut ordered = jobs
        .iter()
        .filter(|(name, _)| super::media::valid_video_filename(name))
        .collect::<Vec<_>>();
    ordered.sort_unstable_by(|left, right| left.0.cmp(right.0));
    let offset = if ordered.is_empty() {
        0
    } else {
        offset.min((ordered.len() - 1) / 25 * 25)
    };
    let mut ids = handles().lock().unwrap_or_else(|p| p.into_inner());
    ids.retain(|_, name| jobs.contains_key(name));
    let items = ordered
        .iter()
        .skip(offset)
        .take(25)
        .map(|(name, job)| {
            let id = if let Some((id, _)) = ids.iter().find(|(_, candidate)| *candidate == *name) {
                id.clone()
            } else {
                let id = super::security::random_token();
                ids.insert(id.clone(), (*name).clone());
                id
            };
            queue_row(name, job, &id, privacy)
        })
        .collect::<Vec<_>>();
    json!({"ok":true, "detail":"", "offset":offset, "total":ordered.len(),
        "nextOffset": if offset + items.len() < ordered.len() { Some(offset + 25) } else { None },
        "previousOffset": if offset > 0 { Some(offset.saturating_sub(25)) } else { None },
        "items":items})
    .to_string()
}

pub(crate) fn queue_media(id: &str) -> Option<String> {
    handles()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(id)
        .cloned()
}

pub(crate) fn queue_action(id: &str, action: &str) -> bool {
    if !matches!(action, "pause" | "resume" | "cancel") {
        return false;
    }
    let Some(filename) = queue_media(id) else {
        return false;
    };
    let Some(output_dir) = QUEUE_OUTPUT_DIR.get() else {
        return false;
    };
    let Ok(client) = external::http::build_client() else {
        return false;
    };
    workflows::downloads::apply_queue_action(&client, output_dir, &filename, action).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn job(phase: DownloadPhase) -> DownloadJob {
        DownloadJob {
            phase,
            downloaded: 42,
            total: Some(100),
            error: Some("private synthetic-name.mp4".into()),
            source_url: Some("https://example.com/private".into()),
            media_url: None,
            audio_url: None,
            extract_audio: false,
            quality_label: None,
            quality_height: None,
        }
    }
    #[test]
    fn native_playback_details_exclude_provider_identity_and_errors() {
        let mut fixture = job(DownloadPhase::Downloading);
        fixture.quality_label = Some("Synthetic quality".into());
        let private = playback_details_row(&fixture, true);
        assert_eq!(private["downloaded"], 42);
        assert_eq!(private["total"], 100);
        assert!(private["quality"].is_null());
        assert!(!private.to_string().contains("private"));
        let public = playback_details_row(&fixture, false);
        assert_eq!(public["quality"], "Synthetic quality");
        assert!(!public.to_string().contains("https://"));
        assert!(public.get("error").is_none());
    }
    #[test]
    fn native_queue_privacy_omits_identifying_fields() {
        let row = queue_row(
            "synthetic-name.mp4",
            &job(DownloadPhase::Downloading),
            "opaque-handle",
            true,
        );
        assert!(!row.to_string().contains("synthetic-name"));
        assert!(!row.to_string().contains("example.com"));
        assert_eq!(row["title"], "Downloaded media");
        assert_eq!(row["downloaded"], 42);
        assert_eq!(row["actions"], json!(["pause", "cancel"]));
        assert_eq!(row["issue"], true);
    }
    #[test]
    fn native_queue_controls_follow_phase_without_mutating_jobs() {
        assert_eq!(
            queue_row(
                "synthetic-name.mp4",
                &job(DownloadPhase::Ready),
                "opaque",
                false
            )["actions"],
            json!([])
        );
        assert_eq!(
            queue_row(
                "synthetic-name.mp4",
                &job(DownloadPhase::Paused),
                "opaque",
                false
            )["actions"],
            json!(["resume", "cancel"])
        );
    }
}

#[derive(Clone)]
struct LibraryTarget {
    href: String,
    thumbnail: Option<String>,
}
static LIBRARY_REVISION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static LIBRARY_HANDLES: OnceLock<Mutex<HashMap<String, LibraryTarget>>> = OnceLock::new();
fn library_handles() -> &'static Mutex<HashMap<String, LibraryTarget>> {
    LIBRARY_HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn library_row(entry: &super::pages::gallery::GalleryEntry, id: &str, privacy: bool) -> Value {
    json!({
        "id": id,
        "title": if privacy {
            if entry.kind == "playlist" { "Playlist" } else { "Downloaded media" }
        } else { &entry.title },
        "subtitle": if privacy { &entry.state } else { &entry.subtitle },
        "kind": entry.kind,
        "state": entry.state,
        "hasThumbnail": !privacy && entry.thumbnail.is_some(),
        "canShare": entry.filename.is_some() && !entry.kind.starts_with("downloading"),
        "canDelete": entry.filename.is_some() && !entry.kind.starts_with("downloading"),
    })
}

fn select_library_entries<'a>(
    source: &'a [super::pages::gallery::GalleryEntry],
    search: &str,
    resume_selection: &str,
) -> Result<(Vec<&'a super::pages::gallery::GalleryEntry>, bool), &'static str> {
    let search = search.trim().to_lowercase();
    let resume_order = if resume_selection.is_empty() {
        None
    } else {
        Some(
            serde_json::from_str::<Vec<String>>(
                resume_selection
                    .strip_prefix("up-next:")
                    .unwrap_or(resume_selection),
            )
            .map_err(|_| "Could not load playback progress")?
            .into_iter()
            .filter(|name| super::media::valid_video_filename(name))
            .enumerate()
            .map(|(index, name)| (name, index))
            .collect::<HashMap<_, _>>(),
        )
    };
    let mut entries = source
        .iter()
        .filter(|entry| search.is_empty() || entry.search_text.contains(&search))
        .filter(|entry| {
            resume_order.as_ref().is_none_or(|order| {
                entry
                    .filename
                    .as_ref()
                    .is_some_and(|name| order.contains_key(name))
            })
        })
        .collect::<Vec<_>>();
    if let Some(order) = &resume_order {
        entries.sort_by_key(|entry| {
            entry
                .filename
                .as_ref()
                .and_then(|name| order.get(name))
                .copied()
                .unwrap_or(usize::MAX)
        });
    }
    Ok((entries, resume_order.is_some()))
}

pub(crate) fn library_snapshot(
    offset: usize,
    location: &str,
    search: &str,
    privacy: bool,
    resume_selection: &str,
) -> String {
    let result = (|| -> Result<Value, String> {
        let output = QUEUE_OUTPUT_DIR
            .get()
            .ok_or("Download engine is starting. Try again.")?;
        let selected = if location.is_empty() {
            None
        } else {
            let href = library_handles()
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .get(location)
                .map(|target| target.href.clone())
                .ok_or("Playlist is no longer available")?;
            Some(
                href.strip_prefix("/gallery/playlist/")
                    .ok_or("Invalid playlist")?
                    .to_owned(),
            )
        };
        let model = super::pages::gallery::model(output, selected.as_deref())
            .map_err(|_| "Could not load library")?;
        let (entries, resume_filter) =
            select_library_entries(&model.entries, search, resume_selection)?;
        let offset = if entries.is_empty() {
            0
        } else {
            offset.min((entries.len() - 1) / 25 * 25)
        };
        let mut handles = library_handles().lock().unwrap_or_else(|p| p.into_inner());
        // Keep the bridge bounded even after many different library searches.
        if handles.len() > 4096 {
            let current = handles.get(location).cloned();
            handles.clear();
            if let Some(current) = current {
                handles.insert(location.to_owned(), current);
            }
        }
        let items = entries
            .iter()
            .skip(offset)
            .take(25)
            .map(|entry| {
                let id = handles
                    .iter()
                    .find(|(_, target)| target.href == entry.href)
                    .map(|(id, _)| id.clone())
                    .unwrap_or_else(super::security::random_token);
                handles.insert(
                    id.clone(),
                    LibraryTarget {
                        href: entry.href.clone(),
                        thumbnail: entry.thumbnail.clone(),
                    },
                );
                let mut row = library_row(entry, &id, privacy);
                row["canSource"] = json!(
                    entry
                        .filename
                        .as_ref()
                        .and_then(|name| source_for_filename(name))
                        .is_some()
                );
                row
            })
            .collect::<Vec<_>>();
        Ok(
            json!({"ok":true,"detail":"","title": if resume_selection.starts_with("up-next:") { "Up Next" } else if resume_filter { "Continue watching" } else if privacy && selected.is_some() { "Playlist" } else { &model.library_title },
            "revision": LIBRARY_REVISION.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            "location":location,"offset":offset,"total":entries.len(),"items":items,
            "nextOffset":if offset+25<entries.len(){Some(offset+25)}else{None},
            "previousOffset":if offset>0{Some(offset.saturating_sub(25))}else{None}}),
        )
    })();
    result.unwrap_or_else(|detail| json!({"ok":false,"detail":detail,"title":"Library","location":"","offset":0,"total":0,"items":[],"nextOffset":null,"previousOffset":null})).to_string()
}

pub(crate) fn library_handle(filename: &str) -> Option<String> {
    if !super::media::valid_video_filename(filename) {
        return None;
    }
    let href = format!("/watch/{filename}");
    let mut handles = library_handles().lock().unwrap_or_else(|p| p.into_inner());
    if let Some((id, _)) = handles.iter().find(|(_, target)| target.href == href) {
        return Some(id.clone());
    }
    if handles.len() >= 4096 {
        handles.clear();
    }
    let id = super::security::random_token();
    handles.insert(
        id.clone(),
        LibraryTarget {
            href,
            thumbnail: None,
        },
    );
    Some(id)
}
pub(crate) fn library_media(id: &str) -> Option<String> {
    let handles = library_handles().lock().unwrap_or_else(|p| p.into_inner());
    handles
        .get(id)
        .and_then(|target| target.href.strip_prefix("/watch/"))
        .filter(|name| super::media::valid_video_filename(name))
        .map(str::to_owned)
}

fn validated_source(source: String) -> Option<String> {
    if !(source.starts_with("https://") || source.starts_with("http://")) {
        return None;
    }
    (!matches!(
        super::sources::classify_url(&source),
        super::sources::SourceUrl::Unsupported
    ))
    .then_some(source)
}
fn source_for_filename(filename: &str) -> Option<String> {
    let source = super::queue::download_jobs()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(filename)?
        .source_url
        .clone()?;
    validated_source(source)
}
pub(crate) fn library_source(id: &str) -> Option<String> {
    source_for_filename(&library_media(id)?)
}

pub(crate) fn library_thumbnail(id: &str, privacy: bool) -> Option<String> {
    if privacy {
        return None;
    }
    let name = library_handles()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(id)?
        .thumbnail
        .clone()?;
    if !super::media::valid_video_filename(&name) {
        return None;
    }
    let output = QUEUE_OUTPUT_DIR.get()?;
    let path = output.join(".thumbnails").join(format!("{name}.jpg"));
    if !path.is_file() {
        super::thumbnails::enqueue_thumbnail(output.join(&name), &name);
        return None;
    }
    Some(path.to_string_lossy().into_owned())
}

pub(crate) fn library_delete(id: &str) -> bool {
    let Some(filename) = library_media(id) else {
        return false;
    };
    let Some(output) = QUEUE_OUTPUT_DIR.get() else {
        return false;
    };
    super::storage::delete_stored_video(output, &filename).is_ok()
}

#[cfg(test)]
mod library_tests {
    use super::*;
    #[test]
    fn native_library_rows_redact_titles_filenames_and_thumbnails() {
        let entry = super::super::pages::gallery::GalleryEntry {
            href: "/gallery/playlist/PLsynthetic1234".into(),
            filename: None,
            state: "Playlist folder".into(),
            title: "Private synthetic collection".into(),
            subtitle: "private-name.mp4".into(),
            search_text: "private".into(),
            kind: "playlist".into(),
            thumbnail: Some("123-1.mp4".into()),
            transition_name: None,
        };
        let mut first = entry.clone();
        first.filename = Some("123-1.mp4".into());
        let mut second = first.clone();
        second.filename = Some("123-2.mp4".into());
        let mut absent = first.clone();
        absent.filename = Some("123-3.mp4".into());
        let source = vec![first, absent, second, entry.clone()];
        let (selected, active) =
            select_library_entries(&source, "", "[\"123-2.mp4\",\"123-1.mp4\"]").unwrap();
        assert!(active);
        assert_eq!(
            selected
                .iter()
                .map(|row| row.filename.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["123-2.mp4", "123-1.mp4"]
        );
        assert!(
            select_library_entries(&source, "", "[]")
                .unwrap()
                .0
                .is_empty()
        );
        assert_eq!(select_library_entries(&source, "", "").unwrap().0.len(), 4);
        assert!(select_library_entries(&source, "", "invalid").is_err());
        assert!(validated_source("javascript:alert(1)".into()).is_none());
        assert!(validated_source("ftp://www.youtube.com/watch?v=abcdefghijk".into()).is_none());
        assert!(validated_source("https://example.invalid/private".into()).is_none());
        assert!(validated_source("https://www.youtube.com/watch?v=abcdefghijk".into()).is_some());
        let row = library_row(&entry, "opaque", true);
        assert_eq!(row["title"], "Playlist");
        assert_eq!(row["hasThumbnail"], false);
        let text = row.to_string();
        for value in ["Private synthetic", "private-name", "123-1", "PLsynthetic"] {
            assert!(!text.contains(value));
        }
        assert_eq!(
            library_row(&entry, "opaque", false)["title"],
            "Private synthetic collection"
        );
    }
}

/// Cache-only artwork path for the Android transport session, never a GPUI field.
fn playback_artwork_path(
    output: &std::path::Path,
    filename: &str,
    privacy: bool,
) -> Option<std::path::PathBuf> {
    if privacy || !super::media::valid_video_filename(filename) || !filename.ends_with(".mp4") {
        return None;
    }
    Some(output.join(".thumbnails").join(format!("{filename}.jpg")))
}

pub fn playback_artwork(filename: &str, privacy: bool) -> Option<String> {
    let path = playback_artwork_path(QUEUE_OUTPUT_DIR.get()?, filename, privacy)?;
    // Never request generation or follow symlinks for transport artwork.
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > 8 * 1024 * 1024 {
        return None;
    }
    Some(path.to_string_lossy().into_owned())
}

#[test]
fn native_playback_artwork_is_private_cache_only_and_video_only() {
    let output = std::path::Path::new("synthetic-output");
    assert_eq!(
        playback_artwork_path(output, "123-1.mp4", false),
        Some(output.join(".thumbnails/123-1.mp4.jpg"))
    );
    for name in [
        "123-1.m4a",
        "../123-1.mp4",
        "/123-1.mp4",
        "https://example.test/file.mp4",
        "invalid.mp4",
    ] {
        assert_eq!(playback_artwork_path(output, name, false), None);
    }
    assert_eq!(playback_artwork_path(output, "123-1.mp4", true), None);
}

/// Resolve a validated app playback route without exposing filesystem paths.
pub fn playback_route(filename: &str) -> Option<&'static str> {
    if !super::media::valid_video_filename(&filename) {
        return None;
    }
    let output = super::queue::QUEUE_OUTPUT_DIR.get()?;
    if super::files::is_complete_download(&output.join(&filename)).ok()? {
        return Some("media");
    }
    let active = super::queue::download_job(&filename).is_some_and(|job| {
        matches!(
            job.phase,
            super::queue::DownloadPhase::Queued
                | super::queue::DownloadPhase::Starting
                | super::queue::DownloadPhase::Downloading
                | super::queue::DownloadPhase::Paused
        )
    });
    active.then_some("stream")
}

fn next_in_playback_order<'a>(items: &'a [String], current: &str) -> Option<&'a str> {
    match items.iter().position(|item| item == current) {
        Some(index) => items.get(index + 1).map(String::as_str),
        None => items
            .iter()
            .find(|item| item.as_str() != current)
            .map(String::as_str),
    }
}
/// App-only playback selection, never a native presentation field.
pub fn playback_next(current: &str) -> Option<String> {
    if !super::media::valid_video_filename(current) {
        return None;
    }
    let output = QUEUE_OUTPUT_DIR.get()?;
    let items = super::gallery::playback_order(output).ok()?;
    next_in_playback_order(&items, current).map(str::to_owned)
}
#[cfg(test)]
#[test]
fn native_playback_order_next_preserves_end_and_missing_selection() {
    let items = vec!["123-1.mp4".to_owned(), "123-2.mp4".to_owned()];
    assert_eq!(
        next_in_playback_order(&items, "123-1.mp4"),
        Some("123-2.mp4")
    );
    assert_eq!(next_in_playback_order(&items, "123-2.mp4"), None);
    assert_eq!(
        next_in_playback_order(&items, "456-1.mp4"),
        Some("123-1.mp4")
    );
    assert_eq!(next_in_playback_order(&[], "123-1.mp4"), None);
}

/// Opaque playback actions reuse the validated native library registry.
pub fn playback_handle(filename: &str) -> Option<String> {
    library_handle(filename)
}

/// Numeric progress only; used internally to bound growing-decoder recovery.
pub fn playback_downloaded(filename: &str) -> u64 {
    if !super::media::valid_video_filename(filename) {
        return 0;
    }
    super::queue::download_job(filename)
        .map(|job| job.downloaded)
        .unwrap_or(0)
}

fn playback_details_row(job: &DownloadJob, privacy: bool) -> Value {
    json!({"downloaded": job.downloaded.min(i64::MAX as u64), "total": job.total.unwrap_or(0).min(i64::MAX as u64),
        "quality": if privacy { None } else { job.quality_label.clone() }})
}

/// Playback progress and optional quality only, never source addresses or errors.
pub fn playback_details(filename: &str, privacy: bool) -> String {
    if !super::media::valid_video_filename(filename) {
        return "{}".into();
    }
    super::queue::download_job(filename)
        .map(|job| playback_details_row(&job, privacy).to_string())
        .unwrap_or_else(|| "{}".into())
}
