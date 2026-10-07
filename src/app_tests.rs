use super::local;
use super::local::discovery::BULK_QUALITY_SCRIPT;
use super::local::gallery::{GALLERY_INITIAL_ITEMS, PlaylistMembership};
use super::local::media::GrowingFile;
use super::local::models::ResolvedVideo;
use super::local::pages::changelog::CHANGELOG;
use super::local::peers::{PEER_CSS, PEER_PAIRING_SCRIPT, PeerManifest};
use super::local::queue::{DownloadJob, DownloadPhase};
use super::local::web_assets::{
    APPEARANCE_BOOT_SCRIPT, APPEARANCE_CSS, APPEARANCE_SCRIPT, DARK_SPACE_BACKGROUND,
    DARK_SPACE_BACKGROUND_PLACEHOLDER, INDEX_CSS, INDEX_HTML, PLAYBACK_SCRIPT, PLAYER_CSS,
    VIEW_TRANSITION_SCRIPT,
};
use super::local::x::{V2Format, V2ThreadResponse, V2Video};
use reqwest::Url;
use std::fs::File;
use std::io::{Read, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, fs, thread};

#[test]
fn parses_status_id_and_video_number() {
    let url = "https://x.com/user/status/2091257067264733401/video/2?ref=test";
    assert_eq!(
        local::x::status_id_from_url(url),
        Some("2091257067264733401")
    );
    assert_eq!(local::x::video_number_from_url(url), Some(2));
}

#[test]
fn rejects_non_numeric_status_id() {
    assert_eq!(
        local::x::status_id_from_url("https://x.com/u/status/nope"),
        None
    );
}

#[test]
fn extracts_and_deduplicates_batch_links() {
    let links = local::sources::extract_download_urls(
        "first https://x.com/a/status/123/video/1\n\
         duplicate https://x.com/a/status/123/video/1 and \
         https://twitter.com/b/status/456/video/2.",
    );
    assert_eq!(links.len(), 2);
    assert_eq!(links[0], "https://x.com/a/status/123/video/1");
    assert_eq!(links[1], "https://twitter.com/b/status/456/video/2");
}

#[test]
fn extracts_playlist_links_for_discovery() {
    let playlist = "https://youtube.com/playlist?list=PLoSF8YdZLL8bmNYuiUCst9NMkq4dI5YOq&si=test";
    let links = local::sources::extract_supported_urls(&format!("music {playlist}"));
    assert_eq!(links, vec![playlist.to_owned()]);
    assert!(local::sources::extract_download_urls(playlist).is_empty());
}

#[test]
fn accepts_profile_links_for_discovery() {
    assert_eq!(
        local::x::profile_handle_from_url("https://x.com/AshtonLaxsma"),
        Some("AshtonLaxsma".to_owned())
    );
    assert_eq!(
        local::x::profile_handle_from_url("https://x.com/home"),
        None
    );
    assert_eq!(
        local::x::profile_handle_from_url("https://x.com/user/status/123"),
        None
    );
}

#[test]
fn parses_youtube_shorts_and_watch_links() {
    assert_eq!(
        local::youtube::video_id("https://youtube.com/shorts/xw13xAOyZTw?si=test"),
        Some("xw13xAOyZTw".to_owned())
    );
    assert_eq!(
        local::youtube::video_id("https://www.youtube.com/watch?v=xw13xAOyZTw"),
        Some("xw13xAOyZTw".to_owned())
    );
    assert_eq!(
        local::youtube::video_id("https://youtu.be/xw13xAOyZTw"),
        Some("xw13xAOyZTw".to_owned())
    );
}

#[test]
fn parses_youtube_playlist_links() {
    let id = "PLoSF8YdZLL8bmNYuiUCst9NMkq4dI5YOq";
    assert_eq!(
        local::youtube::playlist_id(&format!(
            "https://youtube.com/playlist?list={id}&si=XvucIps2QoTucO5q"
        )),
        Some(id.to_owned())
    );
    assert_eq!(
        local::youtube::playlist_id(&format!("https://example.com/playlist?list={id}")),
        None
    );
    assert_eq!(
        local::youtube::playlist_id(&format!("http://youtube.com/playlist?list={id}")),
        None
    );
}

#[test]
fn extracts_all_ordered_playlist_items_from_current_youtube_page() {
    let html = r#"<script>var ytInitialData = {
      "contents":{"twoColumnBrowseResultsRenderer":{"tabs":[{"tabRenderer":{"content":{
        "sectionListRenderer":{"contents":[{"itemSectionRenderer":{"contents":[
          {"lockupViewModel":{"contentId":"AAAAAAAAAAA","metadata":{"lockupMetadataViewModel":{"title":{"content":"brace } and escaped \" quote"},"metadata":{"contentMetadataViewModel":{"metadataRows":[{"metadataParts":[{"text":{"content":"Creator A"}}]}]}}}}}},
          {"lockupViewModel":{"contentId":"BBBBBBBBBBB"}},
          {"lockupViewModel":{"contentId":"AAAAAAAAAAA"}},
          {"playlistVideoRenderer":{"videoId":"CCCCCCCCCCC"}},
          {"lockupViewModel":{"contentId":"DDDDDDDDDDD"}},
          {"lockupViewModel":{"contentId":"EEEEEEEEEEE"}},
          {"lockupViewModel":{"contentId":"FFFFFFFFFFF"}},
          {"continuationItemViewModel":{"continuationCommand":{"innertubeCommand":{"continuationCommand":{"token":"next-page"}}}}}
        ]}}]}
      }}}]}}
    };</script>"#;
    let data: serde_json::Value =
        serde_json::from_str(local::youtube::initial_data(html).unwrap()).unwrap();
    let (entries, continuation) = local::youtube::initial_playlist_entries(&data).unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.video_id.as_str())
            .collect::<Vec<_>>(),
        [
            "AAAAAAAAAAA",
            "BBBBBBBBBBB",
            "CCCCCCCCCCC",
            "DDDDDDDDDDD",
            "EEEEEEEEEEE",
            "FFFFFFFFFFF"
        ]
    );
    assert_eq!(entries[0].title, "brace } and escaped \" quote");
    assert_eq!(entries[0].author, "Creator A");
    assert_eq!(continuation.as_deref(), Some("next-page"));
}

#[test]
fn rejects_playlist_data_outside_the_playlist_item_section() {
    let html = r#"<script>var ytInitialData = {
      "contents":{"twoColumnBrowseResultsRenderer":{"tabs":[{"tabRenderer":{"content":{
        "sectionListRenderer":{"contents":[{"otherRenderer":{"contents":[
          {"lockupViewModel":{"contentId":"AAAAAAAAAAA"}}
        ]}}]}
      }}}]}}
    };</script>"#;
    let data: serde_json::Value =
        serde_json::from_str(local::youtube::initial_data(html).unwrap()).unwrap();
    assert!(
        local::youtube::initial_playlist_entries(&data)
            .unwrap()
            .0
            .is_empty()
    );
}

#[test]
fn extracts_current_youtube_continuation_page() {
    let data = serde_json::json!({
        "onResponseReceivedActions": [{
            "appendContinuationItemsAction": {"continuationItems": [
                {"lockupViewModel": {"contentId": "GGGGGGGGGGG"}},
                {"continuationItemViewModel": {"continuationCommand": {
                    "innertubeCommand": {"continuationCommand": {"token": "more"}}
                }}}
            ]}
        }]
    });
    let (entries, continuation) = local::youtube::continuation_playlist_entries(&data).unwrap();
    assert_eq!(entries[0].video_id, "GGGGGGGGGGG");
    assert_eq!(continuation.as_deref(), Some("more"));
}

#[test]
fn parses_snapchat_spotlight_links() {
    let id = "W7_EDlXWTBiXAEEniNoMPwAAYbXV2bWRudHNxAaAtxClKAaAtwqIbAAAAAQ";
    assert_eq!(
        local::snapchat::spotlight_id(&format!(
            "https://www.snapchat.com/spotlight/{id}?share_id=test&locale=en-CA"
        )),
        Some(id.to_owned())
    );
    assert_eq!(
        local::snapchat::spotlight_id(&format!(
            "https://www.snapchat.com/@maja_karolczak/spotlight/{id}"
        )),
        Some(id.to_owned())
    );
    assert_eq!(
        local::snapchat::spotlight_id(&format!("https://example.com/spotlight/{id}")),
        None
    );
}

#[test]
fn parses_snapchat_open_graph_metadata() {
    let id = "W7_EDlXWTBiXAEEniNoMPwAAYbXV2bWRudHNxAaAtxClKAaAtwqIbAAAAAQ";
    let final_url =
        Url::parse(&format!("https://www.snapchat.com/@creator/spotlight/{id}")).unwrap();
    let html = r#"<meta content="Stats | Floor Routine | Creator | Spotlight" property="og:title">
        <meta property="og:video" content="https://bolt-gcdn.sc-cdn.net/v/video?x=1&amp;y=2">
        <meta property="og:video:width" content="540">
        <meta property="og:video:height" content="960">"#;
    let candidate = local::snapchat::candidate_from_html(
        id,
        &format!("https://www.snapchat.com/spotlight/{id}"),
        &final_url,
        html,
    )
    .unwrap();
    assert_eq!(candidate.text, "Floor Routine");
    assert_eq!(candidate.author, "@creator · Snapchat");
    assert_eq!(candidate.resolved.quality_height, Some(540));
    assert_eq!(
        candidate.resolved.media_url,
        "https://bolt-gcdn.sc-cdn.net/v/video?x=1&y=2"
    );
}

#[test]
fn x_quality_variants_are_sorted_and_labeled() {
    let video = V2Video {
        url: "https://video.example/fallback.mp4".to_owned(),
        formats: vec![
            V2Format {
                url: "https://video.example/low.mp4".to_owned(),
                container: Some("mp4".to_owned()),
                bitrate: 256_000,
            },
            V2Format {
                url: "https://video.example/high.mp4".to_owned(),
                container: Some("mp4".to_owned()),
                bitrate: 2_500_000,
            },
            V2Format {
                url: "https://video.example/mid.mp4".to_owned(),
                container: Some("mp4".to_owned()),
                bitrate: 900_000,
            },
        ],
    };
    let qualities = local::x::quality_variants(&video, "123-1.mp4");
    assert_eq!(qualities.len(), 4);
    assert_eq!(qualities[0].media_url, "https://video.example/high.mp4");
    assert!(
        qualities[0]
            .quality_label
            .as_deref()
            .is_some_and(|label| label.starts_with("Best"))
    );
    assert!(
        qualities[2]
            .quality_label
            .as_deref()
            .is_some_and(|label| label.starts_with("Data saver"))
    );
    assert_eq!(qualities[3].filename, "123-1.m4a");
    assert_eq!(
        qualities[3].quality_label.as_deref(),
        Some("Audio only · M4A")
    );
    assert!(qualities[3].extract_audio);
}

#[test]
fn old_queue_entries_default_missing_quality_fields() {
    let job: DownloadJob =
        serde_json::from_str(r#"{"phase":"Paused","downloaded":10,"total":null,"error":null}"#)
            .expect("deserialize old queue entry");
    assert_eq!(job.quality_label, None);
    assert_eq!(job.quality_height, None);
    assert!(!job.extract_audio);
}

#[test]
fn deserializes_nullable_thread_results() {
    let response: V2ThreadResponse =
        serde_json::from_str(r#"{"code":200,"status":null,"thread":null,"author":null}"#)
            .expect("parse thread response");
    assert_eq!(response.code, 200);
    assert!(response.thread.is_none());
}

#[test]
fn detects_same_content_under_different_video_names() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let directory = env::temp_dir().join(format!(
        "rustdl-duplicate-test-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&directory).expect("create duplicate test directory");
    fs::write(directory.join("111-1.mp4"), b"same video bytes").expect("write first video");
    fs::write(directory.join("222-1.mp4"), b"same video bytes").expect("write second video");
    let snapshot = local::storage::storage_snapshot(&directory).expect("scan storage");
    assert_eq!(snapshot.videos.len(), 2);
    assert!(snapshot.videos.iter().all(|video| video.duplicate));
    let fingerprints =
        local::storage::load_fingerprints(&directory).expect("load cached fingerprints");
    assert_eq!(fingerprints.len(), 2);
    assert_eq!(
        fingerprints["111-1.mp4"].blake3,
        fingerprints["222-1.mp4"].blake3
    );
    fs::remove_file(directory.join("111-1.mp4")).expect("remove first video");
    fs::remove_file(directory.join("222-1.mp4")).expect("remove second video");
    fs::remove_file(local::storage::fingerprints_path(&directory))
        .expect("remove fingerprint cache");
    fs::remove_dir(directory).expect("remove duplicate test directory");
}

#[test]
fn immutable_ui_assets_are_content_hashed() {
    assert!(local::web_assets::playback_script_path().starts_with("/__app/playback."));
    assert!(
        local::web_assets::view_transition_script_path().starts_with("/__app/view-transitions.")
    );
    assert!(local::web_assets::index_css_path().starts_with("/__app/index."));
    assert!(local::web_assets::player_css_path().starts_with("/__app/player."));
    assert!(local::web_assets::appearance_css_path().starts_with("/__app/appearance."));
    assert!(local::web_assets::appearance_script_path().starts_with("/__app/appearance."));
    assert!(local::web_assets::dark_space_background_path().starts_with("/__app/dark-space."));
    assert!(local::web_assets::dark_space_background_path().ends_with(".png"));
    assert!(local::web_assets::index_html_template().contains(local::web_assets::index_css_path()));
    assert!(!local::web_assets::index_html_template().contains("<style>"));
}

#[test]
fn appearance_is_shared_persistent_and_motion_aware() {
    let html = local::web_assets::decorate_app_html(
        "<!doctype html><html><head></head><body><main>Test</main></body></html>".to_owned(),
    );
    assert!(html.contains(local::web_assets::appearance_css_path()));
    assert!(html.contains(local::web_assets::appearance_script_path()));
    assert_eq!(html.matches("data-rustdl-appearance").count(), 2);
    assert!(APPEARANCE_BOOT_SCRIPT.contains("bridge.appearance()"));
    assert!(APPEARANCE_SCRIPT.contains("document.startViewTransition"));
    assert!(APPEARANCE_SCRIPT.contains("visibilitychange"));
    assert!(APPEARANCE_CSS.contains("rustdl-space-drift"));
    assert!(APPEARANCE_CSS.contains("prefers-reduced-motion"));
    assert!(APPEARANCE_CSS.contains("data-theme=\"light\""));
    assert!(APPEARANCE_CSS.contains(".gallery-filters button[aria-pressed=\"true\"]"));
    assert!(APPEARANCE_CSS.contains(".card-menu-button:hover"));
    assert!(APPEARANCE_CSS.contains(".card-popover :is(a, button)"));
    assert!(APPEARANCE_CSS.contains(".metric, .item, .system"));
    assert!(APPEARANCE_CSS.contains(".health div, .system-row, .media-row"));
    assert!(APPEARANCE_CSS.contains("form > button[type=\"submit\"]"));
    assert!(!APPEARANCE_CSS.contains("--rustdl-light-"));
    assert!(APPEARANCE_CSS.contains(":root[data-theme] :is("));
    assert!(
        local::web_assets::appearance_css()
            .contains(local::web_assets::dark_space_background_path())
    );
    assert!(!local::web_assets::appearance_css().contains(DARK_SPACE_BACKGROUND_PLACEHOLDER));
    assert!(
        local::web_assets::appearance_css()
            .contains(local::web_assets::rainy_city_background_path())
    );
    assert!(!local::web_assets::appearance_css().contains("__RUSTDL_RAINY_CITY_BACKGROUND__"));
    assert!(local::web_assets::RAINY_CITY_BACKGROUND.starts_with(b"RIFF"));
    assert!(DARK_SPACE_BACKGROUND.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(INDEX_HTML.contains("href=\"/streaming\""));
    for token in [
        "--rustdl-page:",
        "--rustdl-surface:",
        "--rustdl-text:",
        "--rustdl-control:",
        "--rustdl-input:",
        "--rustdl-accent:",
        "--rustdl-danger:",
    ] {
        assert_eq!(APPEARANCE_CSS.matches(token).count(), 2, "{token}");
    }
}

#[test]
fn isolated_streaming_uses_ranked_manifests_instead_of_full_watch_pages() {
    const MAIN_ACTIVITY: &str = include_str!("../android/MainActivity.java");
    const STREAMING_ACTIVITY: &str = include_str!("../android/StreamingActivity.java");
    assert!(MAIN_ACTIVITY.contains("EXTRA_MANIFEST_URL"));
    assert!(MAIN_ACTIVITY.contains("__app/stream-manifest.json?url="));
    assert!(MAIN_ACTIVITY.contains("request.isRedirect() || !request.hasGesture()"));
    assert!(STREAMING_ACTIVITY.contains("requestManifest"));
    assert!(STREAMING_ACTIVITY.contains("queueFailover"));
    assert!(STREAMING_ACTIVITY.contains("changeEpisode"));
    assert!(STREAMING_ACTIVITY.contains("Spinner.MODE_DROPDOWN"));
    assert!(STREAMING_ACTIVITY.contains("selectEpisode(selected.number)"));
    assert!(STREAMING_ACTIVITY.contains("episodeAdapter.notifyDataSetChanged()"));
    assert!(STREAMING_ACTIVITY.contains("chooseEpisodeDownload"));
    assert!(!STREAMING_ACTIVITY.contains("addJavascriptInterface"));
    assert!(STREAMING_ACTIVITY.contains("WindowManager.LayoutParams.FLAG_SECURE"));
    assert!(
        STREAMING_ACTIVITY.contains("allowedSourceHosts.contains(normalizeHost(uri.getHost()))")
    );
    assert!(
        STREAMING_ACTIVITY
            .contains("new String[] {\"all\", \"sub\", \"dub\", \"ready\", \"issues\"}")
    );
    assert!(STREAMING_ACTIVITY.contains("matchesFilter(sources.get(index), sourceFilter)"));
    assert!(STREAMING_ACTIVITY.contains("item.optJSONArray(\"allowedHosts\")"));
    assert!(!STREAMING_ACTIVITY.contains("HOME_URL"));
    assert!(!STREAMING_ACTIVITY.contains("webView.canGoBack()"));
    assert!(!STREAMING_ACTIVITY.contains("redirectsAllowedUntil"));
    assert!(!STREAMING_ACTIVITY.contains("initialRedirect"));
}

#[test]
fn runtime_tuning_balances_speed_and_phone_pressure() {
    let original = local::runtime::runtime_tuning();
    local::runtime::set_runtime_tuning(true, true, false, 0, 8 * 1024 * 1024 * 1024, 8);
    assert_eq!(local::runtime::adaptive_download_limit(), 3);
    assert_eq!(local::runtime::adaptive_download_buffer_bytes(), 512 * 1024);
    local::runtime::set_runtime_tuning(false, false, true, 4, 512 * 1024 * 1024, 8);
    assert_eq!(local::runtime::adaptive_download_limit(), 1);
    assert_eq!(local::runtime::adaptive_download_buffer_bytes(), 64 * 1024);
    local::runtime::set_runtime_tuning(
        original.unmetered,
        original.charging,
        original.power_save,
        original.thermal_status,
        original.free_bytes,
        original.processors,
    );
}

#[test]
fn rejects_non_x_hosts() {
    assert_eq!(
        local::x::status_id_from_url("https://example.com/u/status/2091257067264733401"),
        None
    );
}

#[test]
fn escapes_paths_for_result_page() {
    assert_eq!(
        local::html::escape_html("a&<b>'\""),
        "a&amp;&lt;b&gt;&#39;&quot;"
    );
}

#[test]
fn validates_generated_video_filenames() {
    assert!(local::media::valid_video_filename(
        "anime-0123456789abcdef01234567.mp4"
    ));
    assert!(!local::media::valid_video_filename(
        "anime-not-an-episode.mp4"
    ));
    assert!(!local::media::valid_video_filename(
        "anime-0123456789abcdef01234567/../file.mp4"
    ));
    assert!(local::media::valid_video_filename(
        "2091257067264733401-1.mp4"
    ));
    assert!(local::media::valid_video_filename(
        "youtube-xw13xAOyZTw.mp4"
    ));
    assert!(local::media::valid_video_filename(
        "youtube-xw13xAOyZTw.m4a"
    ));
    assert!(local::media::valid_video_filename(
        "snapchat-W7_EDlXWTBiXAEEniNoMPwAAYbXV2bWRudHNxAaAtxClKAaAtwqIbAAAAAQ.mp4"
    ));
    assert!(local::media::valid_video_filename(
        "snapchat-W7_EDlXWTBiXAEEniNoMPwAAYbXV2bWRudHNxAaAtxClKAaAtwqIbAAAAAQ.m4a"
    ));
    assert!(local::media::is_audio_filename("2091257067264733401-1.m4a"));
    assert_eq!(
        local::media::media_content_type("2091257067264733401-1.m4a"),
        "audio/mp4"
    );
    assert!(!local::media::valid_video_filename("../secret.mp4"));
    assert!(!local::media::valid_video_filename(
        "2091257067264733401-0.mp4"
    ));
}

#[test]
fn parses_browser_byte_ranges() {
    assert_eq!(
        local::media::parse_byte_range("bytes=10-19", 100),
        Some((10, 19))
    );
    assert_eq!(
        local::media::parse_byte_range("bytes=90-", 100),
        Some((90, 99))
    );
    assert_eq!(
        local::media::parse_byte_range("bytes=-10", 100),
        Some((90, 99))
    );
    assert_eq!(local::media::parse_byte_range("bytes=100-", 100), None);
    assert_eq!(local::media::parse_byte_range("bytes=0-1,4-5", 100), None);
}

#[test]
fn gallery_cards_open_one_player_without_embedding_videos() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let directory = env::temp_dir().join(format!(
        "rustdl-gallery-test-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&directory).expect("create gallery test directory");
    let filename = "2091257067264733401-1.mp4";
    let path = directory.join(filename);
    fs::write(&path, b"synthetic test byte").expect("create gallery test video");

    let html = local::pages::gallery::render(&directory).expect("render gallery");
    assert!(!html.contains("<video"));
    assert!(html.contains(&format!(r#"href="/watch/{filename}""#)));
    assert!(html.contains(&format!(r#"src="/thumbnail/{filename}.jpg""#)));
    assert!(html.contains(local::web_assets::index_css_path()));
    assert!(local::web_assets::index_css().contains("@view-transition { navigation: auto; }"));
    assert!(html.contains(r#"data-view-transition-name="video-2091257067264733401-1""#));

    fs::remove_file(path).expect("remove gallery test video");
    fs::remove_dir(directory).expect("remove gallery test directory");
}

#[test]
fn gallery_bounds_initial_dom_and_embeds_the_full_index() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let directory = env::temp_dir().join(format!(
        "rustdl-gallery-window-test-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&directory).expect("create gallery window directory");
    let filenames = (0..40)
        .map(|index| format!("{}-1.mp4", 9_000_000_000_000_000_000_u64 + index))
        .collect::<Vec<_>>();
    for filename in &filenames {
        fs::write(directory.join(filename), b"test").expect("create gallery item");
    }

    let html = local::pages::gallery::render(&directory).expect("render bounded gallery");
    assert_eq!(
        html.matches("data-gallery-index=").count(),
        GALLERY_INITIAL_ITEMS
    );
    assert_eq!(
        html.matches("class=\"card-menu-button\"").count(),
        GALLERY_INITIAL_ITEMS
    );
    assert!(html.contains("id=\"gallery-data\""));
    assert!(html.contains(&filenames[0]));
    assert!(PLAYBACK_SCRIPT.contains("IntersectionObserver"));
    assert!(PLAYBACK_SCRIPT.contains("batchSize=32"));

    for filename in filenames {
        fs::remove_file(directory.join(filename)).expect("remove gallery item");
    }
    fs::remove_dir(directory).expect("remove gallery window directory");
}

#[test]
fn playback_enhancements_cover_resume_seek_speed_and_pip() {
    assert!(PLAYBACK_SCRIPT.contains("savePosition"));
    assert!(PLAYBACK_SCRIPT.contains("dblclick"));
    assert!(PLAYBACK_SCRIPT.contains("playbackRate"));
    assert!(PLAYBACK_SCRIPT.contains("enterPictureInPicture"));
    assert!(PLAYBACK_SCRIPT.contains("Continue watching"));
    assert!(PLAYER_CSS.contains("body.pip"));
    assert!(PLAYBACK_SCRIPT.contains("mini-player-mode"));
    assert!(PLAYBACK_SCRIPT.contains("wireMiniBrowser"));
    assert!(PLAYBACK_SCRIPT.contains("setPlaybackState"));
    assert!(PLAYER_CSS.contains(".mini-player-dock"));
}

#[test]
fn player_and_gallery_share_a_persistent_up_next_queue() {
    assert!(PLAYBACK_SCRIPT.contains("rustdl:up-next:v1"));
    assert!(PLAYBACK_SCRIPT.contains("data-control-next"));
    assert!(PLAYBACK_SCRIPT.contains("Add to Up Next"));
    assert!(PLAYBACK_SCRIPT.contains("Remove from Up Next"));
    assert!(PLAYBACK_SCRIPT.contains("/__app/playback-order.json"));
    assert!(PLAYBACK_SCRIPT.contains("nexttrack:advanceToNext"));
    assert!(PLAYBACK_SCRIPT.contains("miniNext.addEventListener"));
    assert!(PLAYBACK_SCRIPT.contains("data-clear-up-next"));
    assert!(INDEX_CSS.contains(".playback-queue-status"));
}

#[test]
fn modern_controls_cover_anchor_scrubbing_media_session_and_queue() {
    assert!(PLAYER_CSS.contains("position-anchor: --speed-control"));
    assert!(PLAYER_CSS.contains(".control-island"));
    assert!(PLAYER_CSS.contains("position: relative; z-index: 8"));
    assert!(PLAYER_CSS.contains(".player-frame:fullscreen .control-island { position: absolute"));
    assert!(!PLAYER_CSS.contains(".player-frame.controls-idle:not(:focus-within)"));
    assert!(PLAYER_CSS.contains(".scrub-preview"));
    assert!(PLAYER_CSS.contains(".download-boundary"));
    assert!(PLAYBACK_SCRIPT.contains("navigator.mediaSession.setActionHandler"));
    assert!(PLAYBACK_SCRIPT.contains("Waiting for download"));
    assert!(PLAYBACK_SCRIPT.contains("event.preventDefault();event.stopPropagation()"));
    assert!(PLAYBACK_SCRIPT.contains("setPointerCapture"));
    assert!(PLAYBACK_SCRIPT.contains("seekFromPointer"));
    assert!(PLAYBACK_SCRIPT.contains("card-popover"));
    assert!(PLAYBACK_SCRIPT.contains("queue-mini"));
}

#[test]
fn changelog_covers_every_version_and_marks_the_current_release() {
    let current_patch = env!("CARGO_PKG_VERSION")
        .rsplit('.')
        .next()
        .expect("package patch version")
        .parse::<usize>()
        .expect("numeric package patch version");
    assert_eq!(CHANGELOG.len(), current_patch + 1);
    for (index, (version, changes)) in CHANGELOG.iter().rev().enumerate() {
        assert_eq!(*version, format!("0.1.{index}"));
        assert!(!changes.is_empty());
    }
    let html = local::pages::changelog::render();
    assert!(INDEX_HTML.contains(r#"href="/changelog""#));
    assert!(html.contains("Version 0.1.0"));
    assert!(html.contains(&format!("Version {}", env!("CARGO_PKG_VERSION"))));
    assert_eq!(html.matches(r#"class="current""#).count(), 1);
    assert_eq!(
        html.matches(r#"<option value="version-"#).count(),
        CHANGELOG.len()
    );
    assert_eq!(html.matches(r#"<article id="version-"#).count(), 5);
    assert!(html.contains(r#"id="version-jump""#));
    assert!(html.contains(r#"id="version-go""#));
    assert!(html.contains("scrollIntoView"));
    assert!(html.contains("history.replaceState"));
    assert_eq!(
        html.matches(r#"class="release-actions""#).count(),
        CHANGELOG
            .iter()
            .take(5)
            .filter(|(version, _)| !local::pages::changelog::destinations(version).is_empty())
            .count()
    );
    assert!(html.contains("Go to Inspection privacy"));
    assert!(html.contains("Go to Release history"));
    assert!(!html.contains(r#"href="/control"#));
    assert!(!html.contains(r#"<article id="version-0-1-0""#));
    assert!(html.contains(r#"id="load-releases""#));
    assert!(html.contains("/__app/changelog.json?"));
    assert!(INDEX_HTML.contains(r#"id="downloader""#));
}

#[test]
fn diagnostics_are_live_but_exclude_user_content() {
    let html = local::pages::diagnostics::render();
    let bridge = include_str!("../android/DiagnosticsBridge.java");
    assert!(INDEX_HTML.contains(r#"href="/diagnostics""#));
    assert!(html.contains("RustDLDiagnostics"));
    assert!(html.contains("bridge.diagnostics()"));
    assert!(html.contains("response.detail"));
    assert!(html.contains("Load average restricted by Android"));
    assert!(html.contains("copySnapshot"));
    assert!(html.contains(r#"aria-busy="true""#));
    assert!(html.contains("memoryAvailableBytes"));
    assert!(html.contains("Battery sensor"));
    assert!(!html.contains("companionPid"));
    assert!(!html.contains("navigator.clipboard"));
    assert!(bridge.contains("availableSources"));
    assert!(bridge.contains("memoryState()"));
    assert!(bridge.contains("storageState()"));
    assert!(bridge.contains("batteryState()"));
    assert!(bridge.contains("copySnapshot"));
    assert!(html.contains("Privacy boundary"));
    assert!(html.contains("media filenames"));
    assert!(!html.contains("logcat"));
    assert!(!html.contains("Wi-Fi SSID"));
    assert!(!html.contains("/media/"));
    assert!(!html.contains("savedVideos"));
}

#[test]
fn peer_addresses_are_restricted_to_local_ipv4() {
    assert_eq!(
        local::peers::peer_base_url("192.168.50.12:37660")
            .unwrap()
            .as_str(),
        "http://192.168.50.12:37660/"
    );
    assert!(local::peers::peer_base_url("127.0.0.1:18092").is_ok());
    assert!(local::peers::peer_base_url("8.8.8.8:37660").is_err());
    assert!(local::peers::peer_base_url("example.com:37660").is_err());
    assert!(local::peers::peer_base_url("user@192.168.1.2:37660").is_err());
}

#[test]
fn peer_pairing_keys_round_trip_hex() {
    let key = [0xabu8; 32];
    let encoded = local::format::hex_encode(&key);
    assert_eq!(encoded.len(), 64);
    assert_eq!(local::peers::decode_peer_key(&encoded).unwrap(), key);
    assert!(local::peers::decode_peer_key("short").is_err());
    assert!(local::peers::decode_peer_key(&"z".repeat(64)).is_err());
}

#[test]
fn qr_pairing_is_local_and_keeps_payload_out_of_svg() {
    let key = "ab".repeat(32);
    let payload = format!("rustdl://pair?address=192.168.50.12%3A37660&key={key}");
    let svg = local::peers::render_pairing_qr(&payload).expect("render pairing QR");
    assert!(svg.starts_with(r#"<svg class="pairing-qr""#));
    assert!(svg.contains("<path d=\"M"));
    assert!(!svg.contains(&key));

    local::peers::set_outbound_peer_pairing("192.168.50.12:37660", &key)
        .expect("store valid local pairing");
    let pairing = local::peers::current_outbound_peer_pairing().expect("active pairing");
    assert_eq!(pairing.address, "192.168.50.12:37660");
    assert_eq!(pairing.key, [0xab; 32]);
    assert!(local::peers::set_outbound_peer_pairing("8.8.8.8:37660", &key).is_err());
}

#[test]
fn pairing_refresh_keeps_geometry_and_swaps_only_stable_fields() {
    assert!(PEER_CSS.contains("min-height:344px"));
    assert!(PEER_CSS.contains("aspect-ratio:1"));
    assert!(PEER_CSS.contains("view-transition-name:pairing-code"));
    assert!(PEER_CSS.contains(":root{view-transition-name:none}"));
    assert!(PEER_CSS.contains("animation-duration:100ms"));
    assert!(!PEER_CSS.contains("opacity:.55"));
    assert!(PEER_PAIRING_SCRIPT.contains("event.preventDefault()"));
    assert!(PEER_PAIRING_SCRIPT.contains("fetch('/peers/refresh',{cache:'no-store'})"));
    assert!(PEER_PAIRING_SCRIPT.contains("response.json()"));
    assert!(PEER_PAIRING_SCRIPT.contains("document.startViewTransition(swap)"));
    assert!(PEER_PAIRING_SCRIPT.contains("outerHTML=next.qr"));
    assert!(!PEER_PAIRING_SCRIPT.contains("DOMParser"));
    assert!(!PEER_PAIRING_SCRIPT.contains("location.reload"));
}

#[test]
fn playlist_membership_persists_and_renders_as_one_gallery_folder() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let directory = env::temp_dir().join(format!(
        "rustdl-playlist-folder-test-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&directory).expect("create playlist folder test directory");
    let first = "youtube-AAAAAAAAAAA.mp4";
    let second = "youtube-BBBBBBBBBBB.mp4";
    fs::write(directory.join(first), b"first synthetic media").unwrap();
    fs::write(directory.join(second), b"second synthetic media").unwrap();
    let playlist_id = "PL1234567890_test";
    for (filename, position) in [(first, 2), (second, 1)] {
        local::gallery::record_playlist_membership(
            &directory,
            filename,
            PlaylistMembership {
                playlist_id: playlist_id.to_owned(),
                title: "Synthetic road trip".to_owned(),
                position,
                total: 8,
            },
        )
        .unwrap();
    }

    let root = local::pages::gallery::render(&directory).unwrap();
    assert!(root.contains(&format!(r#"href="/gallery/playlist/{playlist_id}""#)));
    assert!(root.contains("Synthetic road trip"));
    assert!(root.contains("2 saved of 8"));
    assert!(!root.contains(&format!(r#"href="/watch/{first}""#)));

    let folder = local::pages::gallery::render_playlist(&directory, Some(playlist_id)).unwrap();
    let second_position = folder.find(&format!(r#"href="/watch/{second}""#)).unwrap();
    let first_position = folder.find(&format!(r#"href="/watch/{first}""#)).unwrap();
    assert!(second_position < first_position);
    assert!(folder.contains("← All media"));

    fs::remove_file(directory.join(first)).unwrap();
    fs::remove_file(directory.join(second)).unwrap();
    fs::remove_file(local::gallery::playlist_memberships_path(&directory)).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn reads_playlist_title_from_youtube_metadata() {
    let data = serde_json::json!({
        "metadata": {"playlistMetadataRenderer": {"title": "Rust playlist"}}
    });
    assert_eq!(
        local::youtube::playlist_title(&data).as_deref(),
        Some("Rust playlist")
    );
}

#[test]
fn bulk_quality_options_expose_file_type_and_resolution() {
    let video = ResolvedVideo {
        filename: "youtube-AAAAAAAAAAA.mp4".to_owned(),
        media_url: "https://example.invalid/video".to_owned(),
        audio_url: None,
        extract_audio: false,
        quality_label: Some("Balanced · 720p".to_owned()),
        quality_height: Some(720),
    };
    let audio = ResolvedVideo {
        filename: "youtube-AAAAAAAAAAA.m4a".to_owned(),
        quality_label: Some("Audio only · M4A".to_owned()),
        ..video.clone()
    };
    let video_option = local::discovery::render_quality_option("token", 2, 0, &video);
    let audio_option = local::discovery::render_quality_option("token", 2, 1, &audio);
    assert!(video_option.contains(r#"data-kind="video" data-height="720""#));
    assert!(audio_option.contains(r#"data-kind="audio""#));
    assert!(BULK_QUALITY_SCRIPT.contains("below[0]||above[0]||videos[0]"));
    assert!(BULK_QUALITY_SCRIPT.contains("value==='audio'"));
}

#[test]
fn gallery_search_and_filters_are_wired_to_media_and_folders() {
    assert!(INDEX_CSS.contains(".gallery-tools"));
    assert!(PLAYBACK_SCRIPT.contains("syncGalleryFilter"));
    assert!(PLAYBACK_SCRIPT.contains("filter==='playlists'&&playlist"));
    assert!(PLAYBACK_SCRIPT.contains("filter==='audio'&&audio"));
    assert!(PLAYBACK_SCRIPT.contains("filter==='downloading'&&downloading"));
    assert!(PLAYBACK_SCRIPT.contains("entry.searchText"));
    assert!(PLAYBACK_SCRIPT.contains("renderBatch"));
    assert!(PLAYBACK_SCRIPT.contains("galleryEntries.forEach"));
    assert!(PLAYBACK_SCRIPT.contains("gallery-card-actions"));
    assert!(PLAYBACK_SCRIPT.contains("requestAnimationFrame"));
    assert!(PLAYBACK_SCRIPT.contains("sessionStorage.setItem(stateKey"));
}

#[test]
fn peer_resume_metadata_resets_only_for_different_content() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let directory = env::temp_dir().join(format!(
        "rustdl-peer-test-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let filename = "youtube-AAAAAAAAAAA.mp4";
    let manifest = PeerManifest {
        filename: filename.to_owned(),
        size: 8,
        hash: "a".repeat(64),
    };
    assert_eq!(
        local::peers::prepare_peer_receive(&directory, &manifest)
            .unwrap()
            .offset,
        0
    );
    fs::write(local::peers::peer_part_path(&directory, filename), b"part").unwrap();
    assert_eq!(
        local::peers::prepare_peer_receive(&directory, &manifest)
            .unwrap()
            .offset,
        4
    );
    let changed = PeerManifest {
        hash: "b".repeat(64),
        ..manifest
    };
    assert_eq!(
        local::peers::prepare_peer_receive(&directory, &changed)
            .unwrap()
            .offset,
        0
    );
    local::files::remove_if_exists(&local::peers::peer_part_path(&directory, filename)).unwrap();
    local::files::remove_if_exists(&local::peers::peer_manifest_path(&directory, filename))
        .unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn app_state_uses_stable_download_phase_names() {
    assert_eq!(
        local::queue::download_phase_name(DownloadPhase::Queued),
        "queued"
    );
    assert_eq!(
        local::queue::download_phase_name(DownloadPhase::Downloading),
        "downloading"
    );
    assert_eq!(
        local::queue::download_phase_name(DownloadPhase::Ready),
        "ready"
    );
    let job = DownloadJob {
        phase: DownloadPhase::Downloading,
        downloaded: 25,
        total: Some(100),
        error: None,
        source_url: Some("https://x.com/example/status/123".to_owned()),
        media_url: None,
        audio_url: None,
        extract_audio: false,
        quality_label: Some("Balanced · 720p".to_owned()),
        quality_height: Some(720),
    };
    let state = local::queue::app_state_job("123-1.mp4", &job);
    assert_eq!(state["phase"], "downloading");
    assert_eq!(state["downloaded"], 25);
    assert_eq!(state["quality"], "Balanced · 720p");
    assert!(state.get("media_url").is_none());
}

#[test]
fn view_transitions_feature_detect_and_select_one_shared_thumbnail() {
    assert!(VIEW_TRANSITION_SCRIPT.contains("document.startViewTransition"));
    assert!(VIEW_TRANSITION_SCRIPT.contains("selectSharedElement"));
    assert!(VIEW_TRANSITION_SCRIPT.contains("style.viewTransitionName='none'"));
    assert!(PLAYBACK_SCRIPT.contains("dataset.viewTransitionName"));
}

#[test]
fn creates_safe_transition_names() {
    assert_eq!(
        local::web_assets::view_transition_name("2091257067264733401-1.mp4"),
        "video-2091257067264733401-1"
    );
    assert_eq!(
        local::web_assets::view_transition_name("odd name.mp4"),
        "video-odd-name"
    );
}

#[test]
fn reads_bytes_as_a_download_grows() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let directory = env::temp_dir().join(format!(
        "rustdl-growing-test-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&directory).expect("create test directory");
    let filename = format!("{}-1.mp4", std::process::id());
    let output = directory.join(&filename);
    let temporary = local::files::part_path(&output);
    File::create(&temporary).expect("create partial file");
    local::queue::set_download_job(
        &filename,
        DownloadJob {
            phase: DownloadPhase::Downloading,
            downloaded: 0,
            total: Some(11),
            error: None,
            source_url: None,
            media_url: None,
            audio_url: None,
            extract_audio: false,
            quality_label: None,
            quality_height: None,
        },
    );

    let writer_filename = filename.clone();
    let writer_path = temporary.clone();
    let writer = thread::spawn(move || {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(writer_path)
            .expect("open partial file for append");
        file.write_all(b"hello").expect("write first chunk");
        file.flush().expect("flush first chunk");
        local::queue::update_download_progress(&writer_filename, 5, Some(11));
        thread::sleep(Duration::from_millis(100));
        file.write_all(b" world").expect("write second chunk");
        file.flush().expect("flush second chunk");
        local::queue::set_download_job(
            &writer_filename,
            DownloadJob {
                phase: DownloadPhase::Ready,
                downloaded: 11,
                total: Some(11),
                error: None,
                source_url: None,
                media_url: None,
                audio_url: None,
                extract_audio: false,
                quality_label: None,
                quality_height: None,
            },
        );
    });

    let mut reader =
        GrowingFile::open(&directory, &filename, 0, Some(10)).expect("open growing reader");
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).expect("read growing file");
    writer.join().expect("join writer");
    assert_eq!(bytes, b"hello world");

    local::queue::download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&filename);
    fs::remove_file(temporary).expect("remove partial file");
    fs::remove_dir(directory).expect("remove test directory");
}

#[test]
fn large_playlist_quality_and_post_import_keep_every_selection() {
    use super::local::models::DiscoveryCandidate;
    let directory = env::temp_dir().join(format!(
        "rustdl-select-all-{}",
        local::security::random_token()
    ));
    fs::create_dir_all(&directory).unwrap();
    let candidates = (0..500)
        .map(|index| {
            let resolved = ResolvedVideo {
                filename: format!("youtube-selall{index:05}.mp4"),
                media_url: "https://example.invalid/synthetic.mp4".to_owned(),
                audio_url: None,
                extract_audio: false,
                quality_label: None,
                quality_height: None,
            };
            fs::write(
                directory.join(&resolved.filename),
                b"synthetic completed fixture",
            )
            .unwrap();
            DiscoveryCandidate {
                qualities: vec![resolved.clone()],
                resolved,
                source_url: "https://example.invalid/synthetic".to_owned(),
                author: "Synthetic".to_owned(),
                text: String::new(),
                playlist: None,
            }
        })
        .collect();
    let token = local::discovery::store_discovery_session(candidates);
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let address = format!("http://{}", server.server_addr());
    let server_directory = directory.clone();
    let worker = thread::spawn(move || {
        let client = super::external::http::build_client().unwrap();
        for _ in 0..2 {
            let request = server
                .recv_timeout(Duration::from_secs(10))
                .unwrap()
                .unwrap();
            super::workflows::server::handle_request(request, &client, &server_directory).unwrap();
        }
    });
    let client = super::external::http::build_client().unwrap();
    let picks = (0..500)
        .map(|index| format!("pick={token}:{index}"))
        .collect::<Vec<_>>()
        .join("&");
    let response = client
        .get(format!("{address}/quality?{picks}"))
        .send()
        .unwrap();
    assert!(response.status().is_success());
    let html = response.text().unwrap();
    assert_eq!(html.matches("class=\"quality-card\"").count(), 500);
    assert!(html.contains("Add 500 to queue"));
    let picks = (0..500)
        .map(|index| format!("pick={token}%3A{index}%3A0"))
        .collect::<Vec<_>>()
        .join("&");
    let response = client
        .post(format!("{address}/import"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(picks)
        .send()
        .unwrap();
    assert!(response.status().is_success());
    let html = response.text().unwrap();
    for index in 0..500 {
        assert!(html.contains(&format!("data-filename=\"youtube-selall{index:05}.mp4\"")));
    }
    worker.join().unwrap();
    let mut jobs = local::queue::download_jobs().lock().unwrap();
    for index in 0..500 {
        jobs.remove(&format!("youtube-selall{index:05}.mp4"));
    }
    drop(jobs);
    local::discovery::DISCOVERY_SESSIONS
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .remove(&token);
    fs::remove_dir_all(directory).unwrap();
}
