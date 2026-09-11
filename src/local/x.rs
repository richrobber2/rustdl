//! X URL parsing and conversion of supplied metadata into media choices.

use super::super::local;
use super::super::local::models::{DiscoveryCandidate, ResolvedVideo};
use reqwest::Url;
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
pub(in super::super) struct ApiResponse {
    pub(in super::super) code: u16,
    pub(in super::super) message: String,
    pub(in super::super) tweet: Option<Tweet>,
}

#[derive(Debug, Deserialize)]
pub(in super::super) struct Tweet {
    pub(in super::super) media: Option<Media>,
}

#[derive(Debug, Deserialize)]
pub(in super::super) struct Media {
    #[serde(default)]
    pub(in super::super) videos: Vec<Video>,
}

#[derive(Debug, Deserialize)]
pub(in super::super) struct Video {
    pub(in super::super) url: String,
    #[serde(default)]
    pub(in super::super) variants: Vec<Variant>,
}

#[derive(Debug, Deserialize)]
pub(in super::super) struct Variant {
    pub(in super::super) url: String,
    #[serde(default)]
    pub(in super::super) bitrate: u64,
    pub(in super::super) content_type: String,
}

#[derive(Debug, Deserialize)]
pub(in super::super) struct V2ThreadResponse {
    pub(in super::super) code: u16,
    pub(in super::super) status: Option<V2Status>,
    pub(in super::super) thread: Option<Vec<V2Status>>,
}

#[derive(Debug, Deserialize)]
pub(in super::super) struct V2TimelineResponse {
    pub(in super::super) code: u16,
    #[serde(default)]
    pub(in super::super) results: Vec<V2Status>,
}

#[derive(Clone, Debug, Deserialize)]
pub(in super::super) struct V2Status {
    #[serde(default)]
    pub(in super::super) id: String,
    #[serde(default)]
    pub(in super::super) text: String,
    pub(in super::super) author: Option<V2Author>,
    pub(in super::super) media: Option<V2Media>,
}

#[derive(Clone, Debug, Deserialize)]
pub(in super::super) struct V2Author {
    #[serde(default)]
    pub(in super::super) name: String,
    #[serde(default)]
    pub(in super::super) screen_name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(in super::super) struct V2Media {
    #[serde(default)]
    pub(in super::super) videos: Vec<V2Video>,
}

#[derive(Clone, Debug, Deserialize)]
pub(in super::super) struct V2Video {
    pub(in super::super) url: String,
    #[serde(default)]
    pub(in super::super) formats: Vec<V2Format>,
}

#[derive(Clone, Debug, Deserialize)]
pub(in super::super) struct V2Format {
    pub(in super::super) url: String,
    pub(in super::super) container: Option<String>,
    #[serde(default)]
    pub(in super::super) bitrate: u64,
}

pub(in super::super) fn append_status_candidates(
    status: &V2Status,
    candidates: &mut Vec<DiscoveryCandidate>,
    seen: &mut HashSet<String>,
) {
    if status.id.is_empty() {
        return;
    }
    let Some(media) = &status.media else {
        return;
    };
    let author = status.author.as_ref();
    let handle = author
        .map(|value| value.screen_name.as_str())
        .filter(|value| local::x::valid_handle(value))
        .unwrap_or("i");
    let author_label = author
        .map(|value| {
            if value.name.is_empty() {
                format!("@{}", value.screen_name)
            } else {
                format!("{} · @{}", value.name, value.screen_name)
            }
        })
        .unwrap_or_else(|| "X post".to_owned());
    for (index, video) in media.videos.iter().enumerate() {
        let video_number = index + 1;
        let filename = format!("{}-{video_number}.mp4", status.id);
        if !seen.insert(filename) {
            continue;
        }
        let source_url = format!(
            "https://x.com/{handle}/status/{}/video/{video_number}",
            status.id
        );
        let qualities =
            local::x::quality_variants(video, &format!("{}-{video_number}.mp4", status.id));
        candidates.push(DiscoveryCandidate {
            resolved: qualities[0].clone(),
            qualities,
            source_url,
            author: author_label.clone(),
            text: status.text.clone(),
            playlist: None,
        });
    }
}

pub(in super::super) fn quality_variants(video: &V2Video, filename: &str) -> Vec<ResolvedVideo> {
    let mut formats = video
        .formats
        .iter()
        .filter(|format| {
            !format.url.is_empty()
                && format
                    .container
                    .as_deref()
                    .is_none_or(|value| value == "mp4")
        })
        .collect::<Vec<_>>();
    formats.sort_unstable_by_key(|format| std::cmp::Reverse(format.bitrate));
    formats.dedup_by(|left, right| left.url == right.url);
    let count = formats.len();
    let mut variants = formats
        .into_iter()
        .enumerate()
        .map(|(index, format)| ResolvedVideo {
            filename: filename.to_owned(),
            media_url: format.url.clone(),
            audio_url: None,
            extract_audio: false,
            quality_label: Some(local::formats::quality_label(
                index,
                count,
                None,
                format.bitrate,
            )),
            quality_height: None,
        })
        .collect::<Vec<_>>();
    if variants.is_empty() {
        variants.push(ResolvedVideo {
            filename: filename.to_owned(),
            media_url: video.url.clone(),
            audio_url: None,
            extract_audio: false,
            quality_label: Some("Original".to_owned()),
            quality_height: None,
        });
    }
    if let Some(smallest_source) = variants.last().cloned() {
        variants.push(local::formats::audio_only_variant(&smallest_source, true));
    }
    variants
}

pub(in super::super) fn status_id_from_url(url: &str) -> Option<&str> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if !matches!(
        host,
        "x.com"
            | "www.x.com"
            | "mobile.x.com"
            | "twitter.com"
            | "www.twitter.com"
            | "mobile.twitter.com"
    ) {
        return None;
    }
    let (_, tail) = url.split_once("/status/")?;
    let id = tail.split(['/', '?', '#']).next()?;
    (!id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit())).then_some(id)
}

pub(in super::super) fn profile_handle_from_url(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if !matches!(
        host,
        "x.com"
            | "www.x.com"
            | "mobile.x.com"
            | "twitter.com"
            | "www.twitter.com"
            | "mobile.twitter.com"
    ) {
        return None;
    }
    let mut segments = parsed
        .path_segments()?
        .filter(|segment| !segment.is_empty());
    let handle = segments.next()?;
    if segments.next().is_some() || !local::x::valid_handle(handle) {
        return None;
    }
    (!matches!(
        handle.to_ascii_lowercase().as_str(),
        "home" | "explore" | "search" | "notifications" | "messages" | "settings" | "compose"
    ))
    .then(|| handle.to_owned())
}

pub(in super::super) fn valid_handle(handle: &str) -> bool {
    !handle.is_empty()
        && handle.len() <= 15
        && handle
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

pub(in super::super) fn video_number_from_url(url: &str) -> Option<usize> {
    let (_, tail) = url.split_once("/video/")?;
    let value = tail.split(['/', '?', '#']).next()?;
    value.parse::<usize>().ok().filter(|number| *number > 0)
}

pub(in super::super) fn best_mp4_url(video: &Video) -> &str {
    video
        .variants
        .iter()
        .filter(|variant| variant.content_type == "video/mp4")
        .max_by_key(|variant| variant.bitrate)
        .map(|variant| variant.url.as_str())
        .unwrap_or(&video.url)
}
