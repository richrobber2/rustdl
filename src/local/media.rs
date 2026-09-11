//! Local media serving, growing-file reads, and byte-range parsing.

use super::super::local;
use super::super::local::queue::{DOWNLOAD_PROGRESS_SIGNAL, DownloadPhase};
use reqwest::header::CONTENT_RANGE;
use std::error::Error;
use std::fs::File;
use std::io;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Condvar, Mutex};
use std::time::Duration;
use tiny_http::{Request, Response, StatusCode};

pub(in super::super) fn content_range(
    response: &reqwest::blocking::Response,
) -> Option<(u64, Option<u64>)> {
    let value = response.headers().get(CONTENT_RANGE)?.to_str().ok()?;
    let range = value.strip_prefix("bytes ")?;
    let (bounds, total) = range.split_once('/')?;
    let (start, _) = bounds.split_once('-')?;
    Some((
        start.parse().ok()?,
        (total != "*").then(|| total.parse().ok()).flatten(),
    ))
}

pub(in super::super) fn respond_media(
    request: Request,
    output_dir: &Path,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    if !local::media::valid_video_filename(filename) {
        return local::html::respond_text(request, 404, "Video not found");
    }
    let path = output_dir.join(filename);
    if !local::files::is_complete_download(&path)? {
        return local::html::respond_text(request, 404, "Video not found");
    }

    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let range_header = request
        .headers()
        .iter()
        .find(|value| value.field.equiv("Range"))
        .map(|value| value.value.as_str().to_owned());
    let range = match range_header {
        Some(value) => match local::media::parse_byte_range(&value, length) {
            Some(range) => Some(range),
            None => {
                let response = Response::from_string("Requested range is not satisfiable")
                    .with_status_code(StatusCode(416))
                    .with_header(local::html::header(
                        "Content-Range",
                        &format!("bytes */{length}"),
                    ))
                    .with_header(local::html::header("Accept-Ranges", "bytes"));
                request.respond(response)?;
                return Ok(());
            }
        },
        None => None,
    };

    let common_headers = || {
        vec![
            local::html::header("Content-Type", local::media::media_content_type(filename)),
            local::html::header("Accept-Ranges", "bytes"),
            local::html::header("Cache-Control", "private, max-age=3600"),
            local::html::header("X-Content-Type-Options", "nosniff"),
        ]
    };
    if let Some((start, end)) = range {
        file.seek(SeekFrom::Start(start))?;
        let bytes = end - start + 1;
        let mut headers = common_headers();
        headers.push(local::html::header(
            "Content-Range",
            &format!("bytes {start}-{end}/{length}"),
        ));
        let response = Response::new(
            StatusCode(206),
            headers,
            file.take(bytes),
            Some(usize::try_from(bytes)?),
            None,
        )
        .with_chunked_threshold(usize::MAX);
        request.respond(response)?;
    } else {
        let response = Response::new(
            StatusCode(200),
            common_headers(),
            file,
            Some(usize::try_from(length)?),
            None,
        )
        .with_chunked_threshold(usize::MAX);
        request.respond(response)?;
    }
    Ok(())
}

pub(in super::super) struct GrowingFile {
    pub(in super::super) file: File,
    pub(in super::super) filename: String,
    pub(in super::super) position: u64,
    pub(in super::super) end: Option<u64>,
}

impl GrowingFile {
    pub(in super::super) fn open(
        output_dir: &Path,
        filename: &str,
        start: u64,
        end: Option<u64>,
    ) -> io::Result<Self> {
        let output = output_dir.join(filename);
        let temporary = local::files::part_path(&output);
        let mut file = match File::open(&temporary) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => File::open(output)?,
            Err(error) => return Err(error),
        };
        file.seek(SeekFrom::Start(start))?;
        Ok(Self {
            file,
            filename: filename.to_owned(),
            position: start,
            end,
        })
    }
}

impl Read for GrowingFile {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        loop {
            let limit = match self.end {
                Some(end) if self.position > end => return Ok(0),
                Some(end) => (end - self.position + 1).min(buffer.len() as u64) as usize,
                None => buffer.len(),
            };
            let count = self.file.read(&mut buffer[..limit])?;
            if count > 0 {
                self.position += count as u64;
                return Ok(count);
            }

            match local::queue::download_job(&self.filename) {
                Some(job) if job.phase == DownloadPhase::Failed => {
                    return Err(io::Error::other(
                        job.error
                            .unwrap_or_else(|| "the background download failed".to_owned()),
                    ));
                }
                Some(job)
                    if matches!(
                        job.phase,
                        DownloadPhase::Queued
                            | DownloadPhase::Starting
                            | DownloadPhase::Downloading
                    ) =>
                {
                    let (generation, changed) =
                        DOWNLOAD_PROGRESS_SIGNAL.get_or_init(|| (Mutex::new(0), Condvar::new()));
                    let generation = generation
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let observed = *generation;
                    if self.file.metadata()?.len() > self.position {
                        continue;
                    }
                    let _ = changed
                        .wait_timeout_while(generation, Duration::from_secs(1), |value| {
                            *value == observed
                        })
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
                _ => return Ok(0),
            }
        }
    }
}

pub(in super::super) fn respond_growing_media(
    request: Request,
    output_dir: &Path,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    if !local::media::valid_video_filename(filename) {
        return local::html::respond_text(request, 404, "Video not found");
    }
    if local::files::is_complete_download(&output_dir.join(filename))? {
        return local::media::respond_media(request, output_dir, filename);
    }
    let Some(job) = local::queue::download_job(filename) else {
        return local::html::respond_text(request, 404, "Active download not found");
    };
    if job.phase == DownloadPhase::Failed {
        return local::html::respond_text(request, 422, "The background download failed");
    }

    let range_header = request
        .headers()
        .iter()
        .find(|value| value.field.equiv("Range"))
        .map(|value| value.value.as_str().to_owned());
    let total = job.total.filter(|total| *total > 0);
    let requested_range = match (range_header, total) {
        (Some(value), Some(total)) => match local::media::parse_byte_range(&value, total) {
            Some(range) => Some(range),
            None => {
                let response = Response::from_string("Requested range is not satisfiable")
                    .with_status_code(StatusCode(416))
                    .with_header(local::html::header(
                        "Content-Range",
                        &format!("bytes */{total}"),
                    ))
                    .with_header(local::html::header("Accept-Ranges", "bytes"));
                request.respond(response)?;
                return Ok(());
            }
        },
        _ => None,
    };
    let (status, start, end, length) = match (requested_range, total) {
        (Some((start, end)), _) => (
            StatusCode(206),
            start,
            Some(end),
            Some(usize::try_from(end - start + 1)?),
        ),
        (None, Some(total)) => (
            StatusCode(200),
            0,
            Some(total - 1),
            Some(usize::try_from(total)?),
        ),
        (None, None) => (StatusCode(200), 0, None, None),
    };
    let reader = GrowingFile::open(output_dir, filename, start, end)?;
    let mut headers = vec![
        local::html::header("Content-Type", local::media::media_content_type(filename)),
        local::html::header("Accept-Ranges", "bytes"),
        local::html::header("Cache-Control", "no-store"),
        local::html::header("X-RustDL-Downloaded", &job.downloaded.to_string()),
        local::html::header("X-Content-Type-Options", "nosniff"),
    ];
    if let (Some((start, end)), Some(total)) = (requested_range, total) {
        headers.push(local::html::header(
            "Content-Range",
            &format!("bytes {start}-{end}/{total}"),
        ));
    }
    let response = Response::new(status, headers, reader, length, None)
        .with_chunked_threshold(if length.is_some() { usize::MAX } else { 0 });
    request.respond(response)?;
    Ok(())
}

pub(in super::super) fn valid_video_filename(filename: &str) -> bool {
    let Some(stem) = filename
        .strip_suffix(".mp4")
        .or_else(|| filename.strip_suffix(".m4a"))
    else {
        return false;
    };
    if let Some(episode_id) = stem.strip_prefix("anime-") {
        return episode_id.len() == 24
            && episode_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    }
    if let Some(video_id) = stem.strip_prefix("youtube-") {
        return video_id.len() == 11
            && video_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    }
    if let Some(spotlight_id) = stem.strip_prefix("snapchat-") {
        return (20..=160).contains(&spotlight_id.len())
            && spotlight_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    }
    let Some((status_id, video_number)) = stem.split_once('-') else {
        return false;
    };
    !status_id.is_empty()
        && status_id.bytes().all(|byte| byte.is_ascii_digit())
        && video_number.parse::<usize>().is_ok_and(|number| number > 0)
}

pub(in super::super) fn is_audio_filename(filename: &str) -> bool {
    filename.ends_with(".m4a") && local::media::valid_video_filename(filename)
}

pub(in super::super) fn media_content_type(filename: &str) -> &'static str {
    if local::media::is_audio_filename(filename) {
        "audio/mp4"
    } else {
        "video/mp4"
    }
}

pub(in super::super) fn parse_byte_range(value: &str, length: u64) -> Option<(u64, u64)> {
    if length == 0 || value.contains(',') {
        return None;
    }
    let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
    if start.is_empty() {
        let suffix = end.parse::<u64>().ok()?.min(length);
        return (suffix > 0).then_some((length - suffix, length - 1));
    }
    let start = start.parse::<u64>().ok()?;
    if start >= length {
        return None;
    }
    let end = if end.is_empty() {
        length - 1
    } else {
        end.parse::<u64>().ok()?.min(length - 1)
    };
    (start <= end).then_some((start, end))
}
