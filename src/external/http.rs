//! HTTP client setup and remote media transfer into local files.

use super::super::local::queue::{DownloadJob, DownloadPhase};
use super::super::local::runtime::{EXTRACT_AUDIO_HOOK, PUBLISH_HOOK};
use super::super::{external, local};
use reqwest::blocking::Client;
use reqwest::header::RANGE;
use std::error::Error;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

pub(in super::super) const USER_AGENT: &str = "rustdl/0.1";

pub(in super::super) fn build_client() -> Result<Client, reqwest::Error> {
    Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
}

pub(in super::super) fn download(
    client: &Client,
    url: &str,
    output: &Path,
    force: bool,
) -> Result<(), Box<dyn Error>> {
    let mut response = client.get(url).send()?.error_for_status()?;
    let total = response.content_length();
    let temporary = local::files::part_path(output);
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }

    let result = (|| -> Result<(), Box<dyn Error>> {
        let mut file = File::create(&temporary)?;
        let mut buffer = [0_u8; 64 * 1024];
        let mut downloaded = 0_u64;
        let mut last_progress = None;

        loop {
            let count = response.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            file.write_all(&buffer[..count])?;
            downloaded += count as u64;
            local::format::show_progress(downloaded, total, &mut last_progress)?;
        }
        if let Some(total) = total
            && downloaded != total
        {
            return Err(
                format!("incomplete download: received {downloaded} of {total} bytes").into(),
            );
        }
        file.sync_all()?;
        eprintln!();

        if force && output.exists() {
            fs::remove_file(output)?;
        }
        fs::rename(&temporary, output)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub(in super::super) fn download_adaptive_track(
    client: &Client,
    url: &str,
    path: &Path,
    filename: &str,
    progress_offset: u64,
) -> Result<Option<u64>, String> {
    if local::queue::defer_for_network(filename) {
        return Ok(None);
    }
    let existing = fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let (mut response, mut file, downloaded, total) =
        external::http::open_resumable_download(client, url, path, existing)?;
    let mut received = downloaded;
    let mut buffer = vec![0_u8; local::runtime::adaptive_download_buffer_bytes()];
    loop {
        if local::queue::defer_for_network(filename) {
            file.sync_all().map_err(|error| error.to_string())?;
            return Ok(None);
        }
        match local::queue::download_job(filename).map(|job| job.phase) {
            Some(DownloadPhase::Paused) => {
                file.sync_all().map_err(|error| error.to_string())?;
                local::queue::persist_download_jobs();
                return Ok(None);
            }
            Some(DownloadPhase::Cancelled) | None => {
                file.sync_all().map_err(|error| error.to_string())?;
                local::queue::persist_download_jobs();
                return Ok(None);
            }
            _ => {}
        }
        let count = response
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])
            .map_err(|error| error.to_string())?;
        received += count as u64;
        local::queue::update_download_progress(
            filename,
            progress_offset.saturating_add(received),
            total.map(|total| progress_offset.saturating_add(total)),
        );
    }
    if let Some(total) = total
        && received != total
    {
        return Err(format!(
            "incomplete adaptive track: received {received} of {total} bytes"
        ));
    }
    file.sync_all().map_err(|error| error.to_string())?;
    Ok(Some(received))
}

pub(in super::super) fn open_resumable_download(
    client: &Client,
    media_url: &str,
    temporary: &Path,
    existing: u64,
) -> Result<(reqwest::blocking::Response, File, u64, Option<u64>), String> {
    let mut request = client.get(media_url);
    if existing > 0 {
        request = request.header(RANGE, format!("bytes={existing}-"));
    }
    let mut response = request.send().map_err(|error| error.to_string())?;
    if existing > 0 && response.status().as_u16() == 416 {
        response = client
            .get(media_url)
            .send()
            .map_err(|error| error.to_string())?;
    }
    response = response
        .error_for_status()
        .map_err(|error| error.to_string())?;

    let resumed = existing > 0
        && response.status().as_u16() == 206
        && local::media::content_range(&response).is_some_and(|(start, _)| start == existing);
    let downloaded = if resumed { existing } else { 0 };
    let total = if resumed {
        local::media::content_range(&response)
            .and_then(|(_, total)| total)
            .or_else(|| response.content_length().map(|length| existing + length))
    } else {
        response.content_length()
    };
    let file = if resumed {
        OpenOptions::new().create(true).append(true).open(temporary)
    } else {
        File::create(temporary)
    }
    .map_err(|error| error.to_string())?;
    Ok((response, file, downloaded, total))
}

pub(in super::super) fn finish_web_download(
    mut response: reqwest::blocking::Response,
    mut file: File,
    temporary: &Path,
    output: &Path,
    filename: &str,
    initial_downloaded: u64,
    total: Option<u64>,
) -> Result<(), String> {
    let mut fingerprint = local::files::blake3_hasher_for_existing(temporary, initial_downloaded)?;
    let mut buffer = vec![0_u8; local::runtime::adaptive_download_buffer_bytes()];
    let mut downloaded = initial_downloaded;
    loop {
        if local::queue::defer_for_network(filename) {
            file.sync_all().map_err(|error| error.to_string())?;
            return Ok(());
        }
        match local::queue::download_job(filename).map(|job| job.phase) {
            Some(DownloadPhase::Paused) => {
                file.sync_all().map_err(|error| error.to_string())?;
                local::queue::persist_download_jobs();
                return Ok(());
            }
            Some(DownloadPhase::Cancelled) | None => {
                drop(file);
                let _ = fs::remove_file(temporary);
                local::queue::persist_download_jobs();
                return Ok(());
            }
            _ => {}
        }
        let count = response
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])
            .map_err(|error| error.to_string())?;
        fingerprint.update(&buffer[..count]);
        downloaded += count as u64;
        local::queue::update_download_progress(filename, downloaded, total);
    }
    if let Some(total) = total
        && downloaded != total
    {
        return Err(format!(
            "incomplete download: received {downloaded} of {total} bytes"
        ));
    }
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    let extract_audio = local::queue::download_job(filename).is_some_and(|job| job.extract_audio);
    if extract_audio {
        let extract = EXTRACT_AUDIO_HOOK
            .get()
            .ok_or_else(|| "audio-only extraction is available in the Android app".to_owned())?;
        extract(temporary, output)?;
        fs::remove_file(temporary).map_err(|error| error.to_string())?;
    } else {
        fs::rename(temporary, output).map_err(|error| error.to_string())?;
    }

    let fingerprint = if extract_audio {
        local::files::blake3_file(output).map_err(|error| error.to_string())?
    } else {
        fingerprint.finalize().to_hex().to_string()
    };
    let output_dir = output
        .parent()
        .ok_or_else(|| "download output has no parent directory".to_owned())?;
    if let Err(error) = local::storage::record_file_fingerprint(output_dir, filename, &fingerprint)
    {
        eprintln!("could not cache downloaded media fingerprint: {error}");
    }

    let publish_error = PUBLISH_HOOK
        .get()
        .and_then(|publish| publish(output, filename).err());
    let previous = local::queue::download_job(filename);
    local::queue::set_download_job(
        filename,
        DownloadJob {
            phase: DownloadPhase::Ready,
            downloaded,
            total: Some(total.unwrap_or(downloaded)),
            error: publish_error.clone(),
            source_url: previous.as_ref().and_then(|job| job.source_url.clone()),
            media_url: previous.as_ref().and_then(|job| job.media_url.clone()),
            audio_url: previous.as_ref().and_then(|job| job.audio_url.clone()),
            extract_audio: previous.as_ref().is_some_and(|job| job.extract_audio),
            quality_label: previous.as_ref().and_then(|job| job.quality_label.clone()),
            quality_height: previous.and_then(|job| job.quality_height),
        },
    );
    if let Some(error) = publish_error {
        eprintln!("Android publish warning: {error}");
    }
    eprintln!("Streaming download finished at {}", output.display());
    Ok(())
}
