//! X post, thread, and profile requests through FxTwitter.

use super::super::local;
use super::super::local::models::ResolvedVideo;
use super::super::local::x::{ApiResponse, V2Status, V2ThreadResponse, V2TimelineResponse};
use reqwest::blocking::Client;
use std::error::Error;

pub(in super::super) fn resolve_video(
    client: &Client,
    source_url: &str,
) -> Result<ResolvedVideo, Box<dyn Error>> {
    let status_id = local::x::status_id_from_url(source_url)
        .ok_or("expected a supported X, YouTube, or Snapchat Spotlight URL")?;
    let video_number = local::x::video_number_from_url(source_url).unwrap_or(1);

    eprintln!("Resolving X post {status_id}...");
    let metadata_url = format!("https://api.fxtwitter.com/status/{status_id}");
    let response = client.get(metadata_url).send()?.error_for_status()?;
    let metadata: ApiResponse = response.json()?;
    if metadata.code != 200 {
        return Err(format!(
            "metadata service returned {}: {}",
            metadata.code, metadata.message
        )
        .into());
    }

    let videos = metadata
        .tweet
        .and_then(|tweet| tweet.media)
        .map(|media| media.videos)
        .ok_or("the post has no downloadable video")?;
    let video = videos.get(video_number - 1).ok_or_else(|| {
        format!(
            "the post contains {} video(s), so video {video_number} does not exist",
            videos.len()
        )
    })?;
    let media_url = local::x::best_mp4_url(video).to_owned();

    Ok(ResolvedVideo {
        filename: format!("{status_id}-{video_number}.mp4"),
        media_url,
        audio_url: None,
        extract_audio: false,
        quality_label: Some("Best available".to_owned()),
        quality_height: None,
    })
}

pub(in super::super) fn fetch_statuses(
    client: &Client,
    source: &str,
) -> Result<Vec<V2Status>, Box<dyn Error>> {
    Ok(
        if let Some(status_id) = local::x::status_id_from_url(&source) {
            let endpoint = format!("https://api.fxtwitter.com/2/thread/{status_id}");
            let response = client.get(endpoint).send()?.error_for_status()?;
            let payload: V2ThreadResponse = response.json()?;
            if payload.code != 200 {
                return Err(format!("thread service returned status {}", payload.code).into());
            }
            let mut statuses = payload.thread.unwrap_or_default();
            if let Some(status) = payload.status {
                statuses.insert(0, status);
            }
            statuses
        } else {
            let handle = local::x::profile_handle_from_url(&source)
                .ok_or_else(|| format!("unsupported X link: {source}"))?;
            let endpoint = format!("https://api.fxtwitter.com/2/profile/{handle}/media?count=40");
            let response = client.get(endpoint).send()?.error_for_status()?;
            let payload: V2TimelineResponse = response.json()?;
            if payload.code != 200 {
                return Err(format!("profile service returned status {}", payload.code).into());
            }
            payload.results
        },
    )
}
