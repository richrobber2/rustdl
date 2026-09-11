//! YouTube URL and supplied playlist-response parsing without requests.

use super::super::local;
use reqwest::Url;
use std::collections::HashSet;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

pub(in super::super) fn url_is_fresh(url: &str) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Url::parse(url)
        .ok()
        .and_then(|url| {
            url.query_pairs()
                .find(|(key, _)| key == "expire")
                .and_then(|(_, value)| value.parse::<u64>().ok())
        })
        .is_some_and(|expires| expires > now.saturating_add(5 * 60))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct YouTubePlaylistEntry {
    pub(in super::super) video_id: String,
    pub(in super::super) title: String,
    pub(in super::super) author: String,
}

#[derive(Clone, Debug)]
pub(in super::super) struct YouTubePlaylist {
    pub(in super::super) playlist_id: String,
    pub(in super::super) title: String,
    pub(in super::super) entries: Vec<YouTubePlaylistEntry>,
}

pub(in super::super) fn playlist_title(data: &serde_json::Value) -> Option<String> {
    [
        "/metadata/playlistMetadataRenderer/title",
        "/header/playlistHeaderRenderer/title/simpleText",
        "/header/playlistHeaderRenderer/title/runs/0/text",
        "/header/pageHeaderRenderer/pageTitle",
        "/header/pageHeaderRenderer/content/pageHeaderViewModel/title/dynamicTextViewModel/text/content",
    ]
    .iter()
    .find_map(|pointer| data.pointer(pointer).and_then(serde_json::Value::as_str))
    .map(str::trim)
    .filter(|title| !title.is_empty())
    .map(|title| local::format::truncate_text(title, 120))
}

pub(in super::super) fn initial_data(html: &str) -> Result<&str, Box<dyn Error>> {
    const MARKERS: [&str; 2] = ["var ytInitialData = ", "window[\"ytInitialData\"] = "];
    MARKERS
        .into_iter()
        .find_map(|marker| local::youtube::json_object_after(html, marker))
        .ok_or_else(|| "YouTube playlist data was not found".into())
}

pub(in super::super) fn config_string(html: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\":");
    let start = html.find(&marker)? + marker.len();
    serde_json::Deserializer::from_str(&html[start..])
        .into_iter::<String>()
        .next()?
        .ok()
}

pub(in super::super) fn initial_playlist_entries(
    data: &serde_json::Value,
) -> Result<(Vec<YouTubePlaylistEntry>, Option<String>), Box<dyn Error>> {
    let tabs = data
        .pointer("/contents/twoColumnBrowseResultsRenderer/tabs")
        .and_then(serde_json::Value::as_array)
        .ok_or("YouTube playlist tabs were not found")?;
    let mut entries = Vec::new();
    let mut continuation = None;
    for tab in tabs {
        let Some(sections) = tab
            .pointer("/tabRenderer/content/sectionListRenderer/contents")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        for section in sections {
            let Some(items) = section
                .pointer("/itemSectionRenderer/contents")
                .and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            let (page_entries, next) = local::youtube::playlist_entries_from_items(items);
            entries.extend(page_entries);
            continuation = next.or(continuation);
        }
    }
    Ok((entries, continuation))
}

pub(in super::super) fn continuation_playlist_entries(
    data: &serde_json::Value,
) -> Result<(Vec<YouTubePlaylistEntry>, Option<String>), Box<dyn Error>> {
    let actions = data
        .get("onResponseReceivedActions")
        .or_else(|| data.get("onResponseReceivedEndpoints"))
        .and_then(serde_json::Value::as_array)
        .ok_or("YouTube playlist continuation actions were not found")?;
    let mut entries = Vec::new();
    let mut continuation = None;
    for action in actions {
        let items = action
            .pointer("/appendContinuationItemsAction/continuationItems")
            .or_else(|| action.pointer("/reloadContinuationItemsCommand/continuationItems"))
            .and_then(serde_json::Value::as_array);
        if let Some(items) = items {
            let (page_entries, next) = local::youtube::playlist_entries_from_items(items);
            entries.extend(page_entries);
            continuation = next.or(continuation);
        }
    }
    Ok((entries, continuation))
}

pub(in super::super) fn playlist_entries_from_items(
    items: &[serde_json::Value],
) -> (Vec<YouTubePlaylistEntry>, Option<String>) {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    let mut continuation = None;
    for item in items {
        if let Some(entry) = local::youtube::playlist_entry(item)
            && seen.insert(entry.video_id.clone())
        {
            entries.push(entry);
        }
        continuation = local::youtube::playlist_continuation_token(item).or(continuation);
    }
    (entries, continuation)
}

pub(in super::super) fn playlist_entry(item: &serde_json::Value) -> Option<YouTubePlaylistEntry> {
    let video_id = item
        .pointer("/playlistVideoRenderer/videoId")
        .or_else(|| item.pointer("/lockupViewModel/contentId"))
        .and_then(serde_json::Value::as_str)?;
    if !local::youtube::valid_video_id(video_id) {
        return None;
    }
    let title = item
        .pointer("/lockupViewModel/metadata/lockupMetadataViewModel/title/content")
        .or_else(|| item.pointer("/playlistVideoRenderer/title/runs/0/text"))
        .or_else(|| item.pointer("/playlistVideoRenderer/title/simpleText"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("YouTube video")
        .to_owned();
    let author = item
        .pointer("/lockupViewModel/metadata/lockupMetadataViewModel/metadata/contentMetadataViewModel/metadataRows/0/metadataParts/0/text/content")
        .or_else(|| item.pointer("/playlistVideoRenderer/shortBylineText/runs/0/text"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("YouTube")
        .to_owned();
    Some(YouTubePlaylistEntry {
        video_id: video_id.to_owned(),
        title,
        author,
    })
}

pub(in super::super) fn playlist_continuation_token(item: &serde_json::Value) -> Option<String> {
    item.pointer(
        "/continuationItemViewModel/continuationCommand/innertubeCommand/continuationCommand/token",
    )
    .or_else(|| {
        item.pointer("/continuationItemRenderer/continuationEndpoint/continuationCommand/token")
    })
    .and_then(serde_json::Value::as_str)
    .map(str::to_owned)
}

pub(in super::super) fn json_object_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let marker_end = text.find(marker)? + marker.len();
    let bytes = text.as_bytes();
    let start = bytes[marker_end..].iter().position(|byte| *byte == b'{')? + marker_end;
    let mut depth = 0_u32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in bytes[start..].iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(&text[start..=start + offset]);
                }
            }
            _ => {}
        }
    }
    None
}

pub(in super::super) fn playlist_id(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    if !matches!(
        host.as_str(),
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com"
    ) || parsed.path() != "/playlist"
    {
        return None;
    }
    let playlist_id = parsed
        .query_pairs()
        .find(|(key, _)| key == "list")
        .map(|(_, value)| value.into_owned())?;
    local::youtube::valid_playlist_id(&playlist_id).then_some(playlist_id)
}

pub(in super::super) fn valid_playlist_id(playlist_id: &str) -> bool {
    (10..=128).contains(&playlist_id.len())
        && playlist_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub(in super::super) fn valid_video_id(video_id: &str) -> bool {
    video_id.len() == 11
        && video_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub(in super::super) fn video_id(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    let candidate = if host == "youtu.be" || host == "www.youtu.be" {
        parsed
            .path_segments()?
            .find(|segment| !segment.is_empty())?
            .to_owned()
    } else if matches!(
        host.as_str(),
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com"
    ) {
        let mut segments = parsed
            .path_segments()?
            .filter(|segment| !segment.is_empty());
        match segments.next() {
            Some("shorts" | "embed" | "live") => segments.next()?.to_owned(),
            Some("watch") | None => parsed
                .query_pairs()
                .find(|(key, _)| key == "v")
                .map(|(_, value)| value.into_owned())?,
            _ => return None,
        }
    } else {
        return None;
    };
    local::youtube::valid_video_id(&candidate).then_some(candidate)
}
