//! Snapchat URL validation and metadata parsing without network requests.

use super::super::local;
use super::super::local::models::{DiscoveryCandidate, ResolvedVideo};
use reqwest::Url;
use std::error::Error;

pub(in super::super) fn candidate_from_html(
    spotlight_id: &str,
    canonical: &str,
    final_url: &Url,
    html: &str,
) -> Result<DiscoveryCandidate, Box<dyn Error>> {
    let media_url = local::html::html_meta_content(html, "og:video")
        .or_else(|| local::html::html_meta_content(html, "og:video:secure_url"))
        .ok_or("Snapchat Spotlight page did not expose a video")?;
    let parsed_media = Url::parse(&media_url)?;
    if parsed_media.scheme() != "https" || !local::snapchat::is_media_host(&parsed_media) {
        return Err("Snapchat returned an untrusted media URL".into());
    }
    let width = local::html::html_meta_content(html, "og:video:width")
        .and_then(|value| value.parse::<u32>().ok());
    let height = local::html::html_meta_content(html, "og:video:height")
        .and_then(|value| value.parse::<u32>().ok());
    let quality_height = width.zip(height).map(|(width, height)| width.min(height));
    let quality_label = quality_height
        .map(|height| format!("Original · {height}p"))
        .unwrap_or_else(|| "Original".to_owned());
    let meta_title = local::html::html_meta_content(html, "og:title")
        .unwrap_or_else(|| "Snapchat Spotlight".to_owned());
    let title_parts = meta_title
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let text = title_parts
        .get(1)
        .or_else(|| title_parts.first())
        .copied()
        .unwrap_or("Snapchat Spotlight")
        .to_owned();
    let author = final_url
        .path_segments()
        .and_then(|segments| {
            segments
                .filter_map(|segment| segment.strip_prefix('@'))
                .find(|handle| !handle.is_empty() && handle.len() <= 64)
        })
        .map(|handle| format!("@{handle} · Snapchat"))
        .unwrap_or_else(|| "Snapchat Spotlight".to_owned());
    let resolved = ResolvedVideo {
        filename: format!("snapchat-{spotlight_id}.mp4"),
        media_url,
        audio_url: None,
        extract_audio: false,
        quality_label: Some(quality_label),
        quality_height,
    };
    let audio = local::formats::audio_only_variant(&resolved, true);
    Ok(DiscoveryCandidate {
        qualities: vec![resolved.clone(), audio],
        resolved,
        source_url: canonical.to_owned(),
        author,
        text,
        playlist: None,
    })
}

pub(in super::super) fn is_media_host(url: &Url) -> bool {
    url.host_str()
        .map(str::to_ascii_lowercase)
        .is_some_and(|host| host == "sc-cdn.net" || host.ends_with(".sc-cdn.net"))
}

pub(in super::super) fn spotlight_id(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    if !matches!(host.as_str(), "snapchat.com" | "www.snapchat.com") {
        return None;
    }
    let segments = parsed
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let spotlight_index = segments
        .iter()
        .position(|segment| *segment == "spotlight")?;
    if spotlight_index > 1 || (spotlight_index == 1 && !segments[0].starts_with('@')) {
        return None;
    }
    let id = *segments.get(spotlight_index + 1)?;
    if segments.len() != spotlight_index + 2 {
        return None;
    }
    ((20..=160).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')))
    .then(|| id.to_owned())
}
