//! Player page rendering and responses.

use super::super::super::local;
use super::super::super::local::queue::DownloadPhase;
use std::error::Error;
use std::path::Path;
use tiny_http::Request;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super::super) enum PlaybackState {
    Complete,
    Growing,
}

pub(in super::super::super) fn respond(
    request: Request,
    output_dir: &Path,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    if !local::media::valid_video_filename(filename) {
        return local::html::respond_text(request, 404, "Video not found");
    }
    let ready = local::files::is_complete_download(&output_dir.join(filename))?;
    let active = local::queue::download_job(filename).is_some_and(|job| {
        matches!(
            job.phase,
            DownloadPhase::Queued
                | DownloadPhase::Starting
                | DownloadPhase::Downloading
                | DownloadPhase::Paused
        )
    });
    if !ready && !active {
        return local::html::respond_text(request, 404, "Video not found");
    }
    let state = if ready {
        PlaybackState::Complete
    } else {
        PlaybackState::Growing
    };
    request.respond(local::html::html_response(local::pages::player::render(
        filename, state,
    )))?;
    Ok(())
}

/// Render a filename already validated by the caller, with its completion state.
pub(in super::super::super) fn render(filename: &str, state: PlaybackState) -> String {
    let (status, toolbar, detail, media_route, growing) = match state {
        PlaybackState::Complete => (
            "Local library",
            "Now playing",
            "Streamed directly from your saved RustDL library.",
            "media",
            "false",
        ),
        PlaybackState::Growing => (
            "Downloading",
            "Building while you watch",
            "Playback continues while RustDL writes the remaining bytes.",
            "stream",
            "true",
        ),
    };
    let display_filename = local::html::escape_html(filename);
    let transition_name = local::web_assets::view_transition_name(filename);
    let dev_reload = local::dev::dev_reload_script();
    let audio_only = local::media::is_audio_filename(filename);
    let codec = if audio_only { "M4A" } else { "MP4" };
    let media_element = if audio_only {
        format!(
            r#"<audio class="audio-player" controls autoplay preload="auto" data-filename="{filename}" data-growing="{growing}" src="/{media_route}/{filename}">Your browser does not support HTML5 audio.</audio>"#
        )
    } else {
        format!(
            r#"<video controls autoplay playsinline preload="auto" data-filename="{filename}" data-growing="{growing}" poster="/thumbnail/{filename}.jpg" src="/{media_route}/{filename}">Your browser does not support HTML5 video.</video>"#
        )
    };
    let saved_kind = if audio_only {
        "Saved audio"
    } else {
        "Saved video"
    };
    let player_css = local::web_assets::player_css_path();
    let playback_script = local::web_assets::playback_script_tag();
    let view_transition_script = local::web_assets::view_transition_script_tag();
    format!(
        include_str!("../../../assets/html/player.html"),
        codec = codec,
        detail = detail,
        dev_reload = dev_reload,
        display_filename = display_filename,
        media_element = media_element,
        playback_script = playback_script,
        player_css = player_css,
        saved_kind = saved_kind,
        status = status,
        toolbar = toolbar,
        transition_name = transition_name,
        view_transition_script = view_transition_script
    )
}
