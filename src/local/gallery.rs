//! Local gallery indexes, playlist membership, and performance counters.

use super::super::local;
use super::super::local::queue::DownloadPhase;
use super::super::local::thumbnails::{
    THUMBNAIL_CACHE_HITS, THUMBNAIL_CACHE_MISSES, THUMBNAIL_PENDING, THUMBNAIL_QUEUE_DROPS,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, UNIX_EPOCH};
use std::{fs, io};
use tiny_http::{Request, Response, StatusCode};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(in super::super) struct PlaylistMembership {
    pub(in super::super) playlist_id: String,
    pub(in super::super) title: String,
    pub(in super::super) position: usize,
    pub(in super::super) total: usize,
}

pub(in super::super) static GALLERY_LISTING_CACHE: OnceLock<
    Mutex<HashMap<PathBuf, GalleryListingCache>>,
> = OnceLock::new();

pub(in super::super) static PLAYLIST_MEMBERSHIP_CACHE: OnceLock<
    Mutex<HashMap<PathBuf, PlaylistMembershipCache>>,
> = OnceLock::new();

pub(in super::super) static GALLERY_RENDER_COUNT: AtomicU64 = AtomicU64::new(0);

pub(in super::super) static GALLERY_RENDER_MICROS: AtomicU64 = AtomicU64::new(0);

pub(in super::super) static GALLERY_LAST_ITEMS: AtomicU64 = AtomicU64::new(0);

pub(in super::super) const GALLERY_INITIAL_ITEMS: usize = 32;

#[derive(Clone, Debug)]
pub(in super::super) struct GalleryListingCache {
    pub(in super::super) checked: Instant,
    pub(in super::super) directory_modified_nanos: Option<u128>,
    pub(in super::super) filenames: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct FileStamp {
    pub(in super::super) len: u64,
    pub(in super::super) modified_nanos: u128,
}

#[derive(Clone, Debug)]
pub(in super::super) struct PlaylistMembershipCache {
    pub(in super::super) stamp: Option<FileStamp>,
    pub(in super::super) memberships: HashMap<String, PlaylistMembership>,
}

pub(in super::super) fn respond_gallery_metrics(request: Request) -> Result<(), Box<dyn Error>> {
    let pending = THUMBNAIL_PENDING
        .get()
        .map(|pending| {
            pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .len()
        })
        .unwrap_or(0);
    let body = serde_json::json!({
        "renderCount": GALLERY_RENDER_COUNT.load(Ordering::Relaxed),
        "lastRenderMicros": GALLERY_RENDER_MICROS.load(Ordering::Relaxed),
        "lastItemCount": GALLERY_LAST_ITEMS.load(Ordering::Relaxed),
        "thumbnailHits": THUMBNAIL_CACHE_HITS.load(Ordering::Relaxed),
        "thumbnailMisses": THUMBNAIL_CACHE_MISSES.load(Ordering::Relaxed),
        "thumbnailPending": pending,
        "thumbnailQueueDrops": THUMBNAIL_QUEUE_DROPS.load(Ordering::Relaxed),
    });
    let response = Response::from_string(body.to_string())
        .with_status_code(StatusCode(200))
        .with_header(local::html::header(
            "Content-Type",
            "application/json; charset=utf-8",
        ))
        .with_header(local::html::header("Cache-Control", "no-store"))
        .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

pub(in super::super) fn respond_playback_order(
    request: Request,
    output_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    let ready = local::gallery::load_gallery_filenames(output_dir)?;
    let ready_set = ready.iter().cloned().collect::<HashSet<_>>();
    let mut active = local::queue::download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .filter(|(_, job)| {
            matches!(
                job.phase,
                DownloadPhase::Queued
                    | DownloadPhase::Starting
                    | DownloadPhase::Downloading
                    | DownloadPhase::Paused
            )
        })
        .map(|(filename, _)| filename.clone())
        .filter(|filename| {
            local::media::valid_video_filename(filename) && !ready_set.contains(filename)
        })
        .collect::<Vec<_>>();
    active.sort_unstable_by(|left, right| right.cmp(left));
    active.extend(ready);
    let response = Response::from_string(serde_json::json!({ "items": active }).to_string())
        .with_status_code(StatusCode(200))
        .with_header(local::html::header(
            "Content-Type",
            "application/json; charset=utf-8",
        ))
        .with_header(local::html::header("Cache-Control", "no-store"))
        .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

pub(in super::super) fn playlist_memberships_path(output_dir: &Path) -> PathBuf {
    output_dir.join(".playlists.json")
}

pub(in super::super) fn modified_nanos(path: &Path) -> Option<u128> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos())
}

pub(in super::super) fn file_stamp(path: &Path) -> Option<FileStamp> {
    let metadata = fs::metadata(path).ok()?;
    Some(FileStamp {
        len: metadata.len(),
        modified_nanos: metadata
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_nanos(),
    })
}

pub(in super::super) fn load_gallery_filenames(output_dir: &Path) -> io::Result<Vec<String>> {
    let directory_modified_nanos = local::gallery::modified_nanos(output_dir);
    {
        let mut cache = GALLERY_LISTING_CACHE
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(entry) = cache.get_mut(output_dir)
            && (entry.checked.elapsed() < Duration::from_millis(400)
                || entry.directory_modified_nanos == directory_modified_nanos)
        {
            entry.checked = Instant::now();
            return Ok(entry.filenames.clone());
        }
    }

    let mut filenames = Vec::new();
    if output_dir.is_dir() {
        for entry in fs::read_dir(output_dir)? {
            let entry = entry?;
            let filename = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type()?.is_file() && local::media::valid_video_filename(&filename) {
                filenames.push(filename);
            }
        }
    }
    filenames.sort_unstable_by(|left, right| right.cmp(left));

    let mut cache = GALLERY_LISTING_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if cache.len() >= 4 && !cache.contains_key(output_dir) {
        cache.clear();
    }
    cache.insert(
        output_dir.to_path_buf(),
        GalleryListingCache {
            checked: Instant::now(),
            directory_modified_nanos,
            filenames: filenames.clone(),
        },
    );
    Ok(filenames)
}

pub(in super::super) fn load_playlist_memberships(
    output_dir: &Path,
) -> io::Result<HashMap<String, PlaylistMembership>> {
    let path = local::gallery::playlist_memberships_path(output_dir);
    let stamp = local::gallery::file_stamp(&path);
    if let Some(cached) = PLAYLIST_MEMBERSHIP_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(output_dir)
        .filter(|cached| cached.stamp == stamp)
        .cloned()
    {
        return Ok(cached.memberships);
    }
    let memberships = match fs::read(&path) {
        Ok(data) => serde_json::from_slice::<HashMap<String, PlaylistMembership>>(&data)
            .unwrap_or_default()
            .into_iter()
            .filter(|(filename, membership)| {
                local::media::valid_video_filename(filename)
                    && local::youtube::valid_playlist_id(&membership.playlist_id)
                    && !membership.title.trim().is_empty()
            })
            .collect(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => HashMap::new(),
        Err(error) => return Err(error),
    };
    PLAYLIST_MEMBERSHIP_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(
            output_dir.to_path_buf(),
            PlaylistMembershipCache {
                stamp,
                memberships: memberships.clone(),
            },
        );
    Ok(memberships)
}

pub(in super::super) fn record_playlist_membership(
    output_dir: &Path,
    filename: &str,
    membership: PlaylistMembership,
) -> io::Result<()> {
    if !local::media::valid_video_filename(filename)
        || !local::youtube::valid_playlist_id(&membership.playlist_id)
        || membership.title.trim().is_empty()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid playlist membership",
        ));
    }
    fs::create_dir_all(output_dir)?;
    let mut memberships = local::gallery::load_playlist_memberships(output_dir)?;
    memberships.insert(filename.to_owned(), membership);
    let data = serde_json::to_vec_pretty(&memberships).map_err(io::Error::other)?;
    let destination = local::gallery::playlist_memberships_path(output_dir);
    let temporary = output_dir.join(".playlists.json.part");
    fs::write(&temporary, data)?;
    fs::rename(temporary, destination)?;
    PLAYLIST_MEMBERSHIP_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(output_dir);
    Ok(())
}
