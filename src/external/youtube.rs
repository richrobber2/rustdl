//! YouTube player resolution and public playlist requests.

use super::super::local::models::{DiscoveryCandidate, ResolvedVideo};
use super::super::local::youtube::YouTubePlaylist;
use super::super::{external, local};
use reqwest::Url;
use reqwest::blocking::Client;
use rustypipe::client::{ClientType as YouTubeClientType, RustyPipe};
use rustypipe::model::{
    AudioCodec as YouTubeAudioCodec, AudioFormat as YouTubeAudioFormat,
    VideoCodec as YouTubeVideoCodec, VideoFormat as YouTubeVideoFormat,
};
use std::collections::HashSet;
use std::error::Error;
use std::io::Read;

pub(in super::super) const MAX_PLAYLIST_ITEMS: usize = 5_000;

pub(in super::super) const MAX_PLAYLIST_PAGES: usize = 50;

/// One extractor and runtime per worker, preserving connections and extraction caches.
pub(in super::super) struct Resolver {
    runtime: tokio::runtime::Runtime,
    client: RustyPipe,
}

impl Resolver {
    pub(in super::super) fn new() -> Result<Self, Box<dyn Error>> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let client =
            runtime.block_on(async { RustyPipe::builder().no_storage().no_reporter().build() })?;
        Ok(Self { runtime, client })
    }

    pub(in super::super) fn resolve_candidate(
        &self,
        source_url: &str,
    ) -> Result<DiscoveryCandidate, Box<dyn Error>> {
        let video_id = local::youtube::video_id(source_url).ok_or("invalid YouTube video URL")?;
        let canonical = format!("https://www.youtube.com/watch?v={video_id}");
        eprintln!("Resolving YouTube video {video_id}...");
        let player = self
            .runtime
            .block_on(async {
                let mut last_error = None;
                for client_type in [
                    YouTubeClientType::Ios,
                    YouTubeClientType::Android,
                    YouTubeClientType::Tv,
                ] {
                    match self
                        .client
                        .query()
                        .player_from_client(&video_id, client_type)
                        .await
                    {
                        Ok(player)
                            if player.video_streams.iter().any(|stream| {
                                stream.format == YouTubeVideoFormat::Mp4 && !stream.url.is_empty()
                            }) || (player.video_only_streams.iter().any(|stream| {
                                stream.format == YouTubeVideoFormat::Mp4
                                    && stream.codec == YouTubeVideoCodec::Avc1
                                    && !stream.url.is_empty()
                            }) && player.audio_streams.iter().any(|stream| {
                                stream.format == YouTubeAudioFormat::M4a
                                    && stream.codec == YouTubeAudioCodec::Mp4a
                                    && !stream.url.is_empty()
                            })) =>
                        {
                            return Ok(player);
                        }
                        Ok(_) => {
                            last_error =
                                Some(format!("{client_type:?} returned no progressive MP4"));
                        }
                        Err(error) => last_error = Some(format!("{client_type:?}: {error}")),
                    }
                }
                Err(last_error
                    .unwrap_or_else(|| "no YouTube player clients were available".to_owned()))
            })
            .map_err(|error: String| -> Box<dyn Error> { error.into() })?;
        let details = player.details;
        let audio_url = player
            .audio_streams
            .iter()
            .filter(|stream| {
                stream.format == YouTubeAudioFormat::M4a
                    && stream.codec == YouTubeAudioCodec::Mp4a
                    && !stream.url.is_empty()
            })
            .max_by_key(|stream| stream.bitrate)
            .map(|stream| stream.url.clone());
        let has_progressive = !player.video_streams.is_empty();
        let mut streams = player
            .video_streams
            .into_iter()
            .filter(|stream| stream.format == YouTubeVideoFormat::Mp4 && !stream.url.is_empty())
            .collect::<Vec<_>>();
        if streams.is_empty() && audio_url.is_some() {
            streams = player
                .video_only_streams
                .into_iter()
                .filter(|stream| {
                    stream.format == YouTubeVideoFormat::Mp4
                        && stream.codec == YouTubeVideoCodec::Avc1
                        && !stream.url.is_empty()
                })
                .collect();
        }
        streams.sort_unstable_by_key(|stream| {
            std::cmp::Reverse((stream.width.min(stream.height), stream.bitrate))
        });
        let mut seen_resolutions = HashSet::new();
        streams.retain(|stream| seen_resolutions.insert(stream.width.min(stream.height)));
        let count = streams.len();
        let qualities = streams
            .into_iter()
            .enumerate()
            .map(|(index, stream)| {
                let resolution = stream.width.min(stream.height);
                ResolvedVideo {
                    filename: format!("youtube-{video_id}.mp4"),
                    media_url: stream.url,
                    audio_url: if has_progressive {
                        None
                    } else {
                        audio_url.clone()
                    },
                    extract_audio: false,
                    quality_label: Some(local::formats::quality_label(
                        index,
                        count,
                        Some(resolution),
                        u64::from(stream.bitrate),
                    )),
                    quality_height: Some(resolution),
                }
            })
            .collect::<Vec<_>>();
        let mut qualities = qualities;
        if let Some(audio_url) = audio_url {
            qualities.push(ResolvedVideo {
                filename: format!("youtube-{video_id}.m4a"),
                media_url: audio_url,
                audio_url: None,
                extract_audio: false,
                quality_label: Some("Audio only · M4A".to_owned()),
                quality_height: None,
            });
        }
        let resolved = qualities
            .first()
            .cloned()
            .ok_or("YouTube did not provide a progressive MP4 with audio")?;
        Ok(DiscoveryCandidate {
            resolved,
            qualities,
            source_url: canonical,
            author: details
                .channel_name
                .unwrap_or_else(|| "YouTube Short".to_owned()),
            text: details.name.unwrap_or_else(|| "YouTube video".to_owned()),
            playlist: None,
        })
    }
}

pub(in super::super) fn resolve_candidate(
    source_url: &str,
) -> Result<DiscoveryCandidate, Box<dyn Error>> {
    Resolver::new()?.resolve_candidate(source_url)
}

pub(in super::super) const MAX_YOUTUBE_PLAYLIST_PAGE_BYTES: u64 = 6 * 1024 * 1024;

pub(in super::super) fn fetch_playlist_entries(
    client: &Client,
    playlist_id: &str,
) -> Result<YouTubePlaylist, Box<dyn Error>> {
    eprintln!("Loading all entries in YouTube playlist {playlist_id}...");
    let canonical = format!("https://www.youtube.com/playlist?list={playlist_id}");
    let response = client.get(canonical).send()?.error_for_status()?;
    let html = external::youtube::read_playlist_response(response)?;
    let initial_data = local::youtube::initial_data(&html)?;
    let data: serde_json::Value = serde_json::from_str(initial_data)?;
    let title = local::youtube::playlist_title(&data).unwrap_or_else(|| {
        format!(
            "YouTube playlist · {}",
            &playlist_id[..playlist_id.len().min(12)]
        )
    });
    let (mut entries, mut continuation) = local::youtube::initial_playlist_entries(&data)?;
    let api_key = local::youtube::config_string(&html, "INNERTUBE_API_KEY")
        .ok_or("YouTube playlist API key was not found")?;
    let client_version = local::youtube::config_string(&html, "INNERTUBE_CONTEXT_CLIENT_VERSION")
        .ok_or("YouTube playlist client version was not found")?;
    let mut seen = entries
        .iter()
        .map(|entry| entry.video_id.clone())
        .collect::<HashSet<_>>();

    for page in 1..MAX_PLAYLIST_PAGES {
        let Some(token) = continuation.take() else {
            break;
        };
        if entries.len() >= MAX_PLAYLIST_ITEMS {
            return Err(format!(
                "playlist exceeds the safety limit of {MAX_PLAYLIST_ITEMS} entries"
            )
            .into());
        }
        eprintln!("Loading playlist page {}...", page + 1);
        let mut endpoint = Url::parse("https://www.youtube.com/youtubei/v1/browse")?;
        endpoint.query_pairs_mut().append_pair("key", &api_key);
        let response = client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .header("X-YouTube-Client-Name", "1")
            .header("X-YouTube-Client-Version", &client_version)
            .json(&serde_json::json!({
                "context": {
                    "client": {
                        "clientName": "WEB",
                        "clientVersion": client_version
                    }
                },
                "continuation": token
            }))
            .send()?
            .error_for_status()?;
        let body = external::youtube::read_playlist_response(response)?;
        let page_data: serde_json::Value = serde_json::from_str(&body)?;
        let (page_entries, next) = local::youtube::continuation_playlist_entries(&page_data)?;
        for entry in page_entries {
            if seen.insert(entry.video_id.clone()) {
                entries.push(entry);
            }
        }
        continuation = next;
    }
    if continuation.is_some() {
        return Err(
            format!("playlist exceeds the safety limit of {MAX_PLAYLIST_PAGES} pages").into(),
        );
    }
    if entries.is_empty() {
        return Err("the YouTube playlist has no selectable videos".into());
    }
    Ok(YouTubePlaylist {
        playlist_id: playlist_id.to_owned(),
        title,
        entries,
    })
}

pub(in super::super) fn read_playlist_response(
    response: reqwest::blocking::Response,
) -> Result<String, Box<dyn Error>> {
    let final_url = response.url();
    if final_url.scheme() != "https"
        || !matches!(
            final_url.host_str(),
            Some("youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com")
        )
    {
        return Err("YouTube redirected the playlist to an unsupported host".into());
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_YOUTUBE_PLAYLIST_PAGE_BYTES)
    {
        return Err("the YouTube playlist response is unexpectedly large".into());
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_YOUTUBE_PLAYLIST_PAGE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_YOUTUBE_PLAYLIST_PAGE_BYTES {
        return Err("the YouTube playlist response is unexpectedly large".into());
    }
    Ok(String::from_utf8(bytes)?)
}
