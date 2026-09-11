//! Local poster generation, bounded worker queues, and thumbnail serving.

use super::super::local;
use super::super::local::runtime::THUMBNAIL_HOOK;
use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::{fs, thread};
use tiny_http::{Request, Response, StatusCode};

pub(in super::super) static THUMBNAIL_QUEUE: OnceLock<SyncSender<ThumbnailTask>> = OnceLock::new();

pub(in super::super) static THUMBNAIL_PENDING: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

pub(in super::super) static THUMBNAIL_CACHE_HITS: AtomicU64 = AtomicU64::new(0);

pub(in super::super) static THUMBNAIL_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);

pub(in super::super) static THUMBNAIL_QUEUE_DROPS: AtomicU64 = AtomicU64::new(0);

pub(in super::super) const THUMBNAIL_QUEUE_CAPACITY: usize = 16;

pub(in super::super) const THUMBNAIL_WORKERS: usize = 2;

#[derive(Debug)]
pub(in super::super) struct ThumbnailTask {
    pub(in super::super) source: PathBuf,
    pub(in super::super) filename: String,
}

pub(in super::super) fn thumbnail_queue() -> &'static SyncSender<ThumbnailTask> {
    THUMBNAIL_QUEUE.get_or_init(|| {
        let (sender, receiver) = sync_channel::<ThumbnailTask>(THUMBNAIL_QUEUE_CAPACITY);
        let receiver = Arc::new(Mutex::new(receiver));
        for worker in 0..THUMBNAIL_WORKERS {
            let receiver = Arc::clone(&receiver);
            thread::Builder::new()
                .name(format!("rustdl-thumb-{worker}"))
                .spawn(move || {
                    loop {
                        let task = {
                            receiver
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner())
                                .recv()
                        };
                        let Ok(task) = task else { break };
                        if let Some(generate) = THUMBNAIL_HOOK.get() {
                            let _ = generate(&task.source, &task.filename);
                        }
                        THUMBNAIL_PENDING
                            .get_or_init(|| Mutex::new(HashSet::new()))
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .remove(&task.filename);
                    }
                })
                .expect("thumbnail worker should start");
        }
        sender
    })
}

pub(in super::super) fn enqueue_thumbnail(source: PathBuf, filename: &str) {
    let pending = THUMBNAIL_PENDING.get_or_init(|| Mutex::new(HashSet::new()));
    {
        let mut pending = pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !pending.insert(filename.to_owned()) {
            return;
        }
    }
    let task = ThumbnailTask {
        source,
        filename: filename.to_owned(),
    };
    if let Err(error) = local::thumbnails::thumbnail_queue().try_send(task) {
        let filename = match error {
            TrySendError::Full(task) | TrySendError::Disconnected(task) => task.filename,
        };
        pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&filename);
        THUMBNAIL_QUEUE_DROPS.fetch_add(1, Ordering::Relaxed);
    }
}

pub(in super::super) fn respond_thumbnail(
    request: Request,
    output_dir: &Path,
    thumbnail_name: &str,
) -> Result<(), Box<dyn Error>> {
    let Some(filename) = thumbnail_name.strip_suffix(".jpg") else {
        return local::html::respond_text(request, 404, "Thumbnail not found");
    };
    if !local::media::valid_video_filename(filename) {
        return local::html::respond_text(request, 404, "Thumbnail not found");
    }
    let thumbnail = output_dir.join(".thumbnails").join(thumbnail_name);
    let source = output_dir.join(filename);
    let thumbnail_fresh = thumbnail.is_file()
        && fs::metadata(&thumbnail)
            .and_then(|thumbnail| {
                fs::metadata(&source).map(|source| {
                    thumbnail.modified().ok() >= source.modified().ok() && thumbnail.len() > 0
                })
            })
            .unwrap_or(false);
    if !thumbnail_fresh {
        THUMBNAIL_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
        local::thumbnails::enqueue_thumbnail(source, filename);
    } else {
        THUMBNAIL_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
    }
    if !thumbnail.is_file() {
        let response = Response::from_string("Thumbnail is being prepared")
            .with_status_code(StatusCode(404))
            .with_header(local::html::header(
                "Content-Type",
                "text/plain; charset=utf-8",
            ))
            .with_header(local::html::header("Cache-Control", "no-store"))
            .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
        request.respond(response)?;
        return Ok(());
    }
    let response = Response::from_data(fs::read(thumbnail)?)
        .with_status_code(StatusCode(200))
        .with_header(local::html::header("Content-Type", "image/jpeg"))
        .with_header(local::html::header(
            "Cache-Control",
            "private, max-age=86400",
        ))
        .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}
