//! Local supported-link extraction and deduplication.

use super::super::local;
use std::collections::HashSet;

/// Download/discovery link kinds. Provider parsers remain the validation authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) enum SourceUrl {
    YouTubeVideo { video_id: String },
    YouTubePlaylist { playlist_id: String },
    XPost { status_id: String },
    XProfile { handle: String },
    SnapchatSpotlight { spotlight_id: String },
    Unsupported,
}

pub(in super::super) fn classify_url(url: &str) -> SourceUrl {
    if let Some(video_id) = local::youtube::video_id(url) {
        SourceUrl::YouTubeVideo { video_id }
    } else if let Some(playlist_id) = local::youtube::playlist_id(url) {
        SourceUrl::YouTubePlaylist { playlist_id }
    } else if let Some(spotlight_id) = local::snapchat::spotlight_id(url) {
        SourceUrl::SnapchatSpotlight { spotlight_id }
    } else if let Some(status_id) = local::x::status_id_from_url(url) {
        SourceUrl::XPost {
            status_id: status_id.to_owned(),
        }
    } else if let Some(handle) = local::x::profile_handle_from_url(url) {
        SourceUrl::XProfile { handle }
    } else {
        SourceUrl::Unsupported
    }
}

pub(in super::super) fn extract_download_urls(text: &str) -> Vec<String> {
    local::sources::extract_supported_urls(text)
        .into_iter()
        .filter(|candidate| {
            matches!(
                local::sources::classify_url(candidate),
                SourceUrl::XPost { .. }
                    | SourceUrl::YouTubeVideo { .. }
                    | SourceUrl::SnapchatSpotlight { .. }
            )
        })
        .collect()
}

pub(in super::super) fn extract_supported_urls(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    text.split_whitespace()
        .map(local::sources::trim_url_punctuation)
        .filter(|candidate| local::sources::classify_url(candidate) != SourceUrl::Unsupported)
        .filter(|candidate| seen.insert((*candidate).to_owned()))
        .take(50)
        .map(str::to_owned)
        .collect()
}

pub(in super::super) fn trim_url_punctuation(candidate: &str) -> &str {
    candidate.trim_matches(|character: char| {
        matches!(
            character,
            '<' | '>'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '"'
                | '\''
                | ','
                | ';'
                | '!'
                | '.'
                | '?'
                | ':'
        )
    })
}

#[cfg(test)]
mod tests {
    use super::super::super::local;
    use super::SourceUrl;

    #[test]
    fn classifies_discovery_links_with_their_identifiers() {
        for (url, expected) in [
            (
                "https://youtu.be/xw13xAOyZTw",
                SourceUrl::YouTubeVideo {
                    video_id: "xw13xAOyZTw".into(),
                },
            ),
            (
                "https://www.youtube.com/playlist?list=PL1234567890",
                SourceUrl::YouTubePlaylist {
                    playlist_id: "PL1234567890".into(),
                },
            ),
            (
                "https://x.com/user/status/123",
                SourceUrl::XPost {
                    status_id: "123".into(),
                },
            ),
            (
                "https://x.com/example",
                SourceUrl::XProfile {
                    handle: "example".into(),
                },
            ),
            (
                "https://www.snapchat.com/spotlight/abcdefghijklmnopqrst",
                SourceUrl::SnapchatSpotlight {
                    spotlight_id: "abcdefghijklmnopqrst".into(),
                },
            ),
            ("https://example.com/video", SourceUrl::Unsupported),
            ("not a url", SourceUrl::Unsupported),
            ("https://youtube.com/watch?v=short", SourceUrl::Unsupported),
        ] {
            assert_eq!(local::sources::classify_url(url), expected, "{url}");
        }
    }

    #[test]
    fn video_in_playlist_stays_a_single_video() {
        let url = "https://www.youtube.com/watch?v=xw13xAOyZTw&list=PL1234567890";
        assert_eq!(
            local::sources::classify_url(url),
            SourceUrl::YouTubeVideo {
                video_id: "xw13xAOyZTw".into()
            }
        );
        assert_eq!(
            local::sources::extract_download_urls(url),
            vec![url.to_owned()]
        );
    }

    #[test]
    fn extraction_preserves_order_deduplication_and_collection_filtering() {
        let post = "https://x.com/user/status/123";
        let profile = "https://x.com/example";
        let playlist = "https://www.youtube.com/playlist?list=PL1234567890";
        let text = format!("({post}), {profile} {post} {playlist} ignored");
        assert_eq!(
            local::sources::extract_supported_urls(&text),
            vec![post, profile, playlist]
        );
        assert_eq!(local::sources::extract_download_urls(&text), vec![post]);
        let batch = (1..=51)
            .map(|id| format!("https://x.com/user/status/{id}"))
            .collect::<Vec<_>>()
            .join(" ");
        let urls = local::sources::extract_download_urls(&batch);
        assert_eq!(urls.len(), 50);
        assert_eq!(urls.last().unwrap(), "https://x.com/user/status/50");
    }
}
