//! Local media-format choices and quality labels.

use super::super::local;
use super::super::local::models::ResolvedVideo;

pub(in super::super) fn audio_only_variant(
    source: &ResolvedVideo,
    extract_audio: bool,
) -> ResolvedVideo {
    ResolvedVideo {
        filename: local::formats::replace_media_extension(&source.filename, "m4a"),
        media_url: source.media_url.clone(),
        audio_url: None,
        extract_audio,
        quality_label: Some("Audio only · M4A".to_owned()),
        quality_height: None,
    }
}

pub(in super::super) fn replace_media_extension(filename: &str, extension: &str) -> String {
    let stem = filename
        .strip_suffix(".mp4")
        .or_else(|| filename.strip_suffix(".m4a"))
        .unwrap_or(filename);
    format!("{stem}.{extension}")
}

pub(in super::super) fn quality_label(
    index: usize,
    count: usize,
    height: Option<u32>,
    bitrate: u64,
) -> String {
    let tier = if count <= 1 {
        "Original"
    } else if index == 0 {
        "Best"
    } else if index + 1 == count {
        "Data saver"
    } else {
        "Balanced"
    };
    let details = match (height, bitrate) {
        (Some(height), bitrate) if bitrate > 0 => {
            format!("{height}p · {}", local::formats::format_bitrate(bitrate))
        }
        (Some(height), _) => format!("{height}p"),
        (None, bitrate) if bitrate > 0 => local::formats::format_bitrate(bitrate),
        _ => String::new(),
    };
    if details.is_empty() {
        tier.to_owned()
    } else {
        format!("{tier} · {details}")
    }
}

pub(in super::super) fn format_bitrate(bits_per_second: u64) -> String {
    if bits_per_second >= 1_000_000 {
        format!("{:.1} Mbps", bits_per_second as f64 / 1_000_000.0)
    } else {
        format!("{} Kbps", bits_per_second / 1_000)
    }
}
