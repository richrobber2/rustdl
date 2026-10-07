use super::super::super::local;
use super::super::super::local::queue::DownloadOutcome;
use super::player::PlaybackState;
use std::path::Path;

#[test]
fn player_rendering_preserves_media_type_and_completion_state() {
    for (filename, element) in [("123-1.mp4", "video"), ("123-1.m4a", "audio")] {
        for (state, route, growing) in [
            (PlaybackState::Complete, "media", "false"),
            (PlaybackState::Growing, "stream", "true"),
        ] {
            let html = local::pages::player::render(filename, state);
            assert!(html.contains(&format!("<{element} ")));
            assert!(html.contains(&format!(r#"src="/{route}/{filename}""#)));
            assert!(html.contains(&format!(r#"data-growing="{growing}""#)));
            assert!(html.contains(&local::web_assets::view_transition_name(filename)));
        }
    }
}

#[test]
fn download_result_rendering_preserves_outcome_and_filename_errors() {
    for (outcome, route, growing, status) in [
        (
            DownloadOutcome::Duplicate,
            "media",
            "false",
            "Ready locally",
        ),
        (
            DownloadOutcome::InProgress,
            "stream",
            "true",
            "Download active",
        ),
        (DownloadOutcome::Started, "stream", "true", "Downloading"),
    ] {
        let html = local::pages::download_result::render(Path::new("123-1.mp4"), outcome).unwrap();
        assert!(html.contains(&format!(r#"src="/{route}/123-1.mp4""#)));
        assert!(html.contains(&format!(r#"data-growing="{growing}""#)));
        assert!(html.contains(status));
    }
    assert!(
        local::pages::download_result::render(Path::new("/"), DownloadOutcome::Started).is_err()
    );
}

#[test]
fn native_library_model_preserves_playlist_grouping_and_order() {
    let directory = std::env::temp_dir().join(format!(
        "rustdl-native-model-{}",
        local::security::random_token()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let playlist = "PLsynthetic1234";
    for (filename, position) in [("123-1.mp4", 2), ("123-2.mp4", 1)] {
        // Empty synthetic files exercise listing only, never media decoding.
        std::fs::write(directory.join(filename), []).unwrap();
        local::gallery::record_playlist_membership(
            &directory,
            filename,
            local::gallery::PlaylistMembership {
                playlist_id: playlist.to_owned(),
                title: "Synthetic collection".to_owned(),
                position,
                total: 2,
            },
        )
        .unwrap();
    }
    let all = local::pages::gallery::model(&directory, None).unwrap();
    assert_eq!(all.item_count, 2);
    assert_eq!(all.entries.len(), 1);
    assert_eq!(all.entries[0].kind, "playlist");
    assert_eq!(all.entries[0].title, "Synthetic collection");
    let selected = local::pages::gallery::model(&directory, Some(playlist)).unwrap();
    assert_eq!(selected.library_title, "Synthetic collection");
    assert_eq!(selected.entries.len(), 2);
    assert_eq!(selected.entries[0].filename.as_deref(), Some("123-2.mp4"));
    assert_eq!(selected.entries[1].filename.as_deref(), Some("123-1.mp4"));
    let html = local::pages::gallery::render_playlist(&directory, Some(playlist)).unwrap();
    assert!(html.contains("Synthetic collection"));
    assert!(html.contains("/watch/123-2.mp4"));
    std::fs::remove_dir_all(directory).unwrap();
}
