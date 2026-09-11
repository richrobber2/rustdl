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
