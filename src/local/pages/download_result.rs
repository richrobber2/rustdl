//! Download result page rendering and responses.

use super::super::super::local;
use super::super::super::local::queue::DownloadOutcome;
use std::error::Error;
use std::path::Path;
use tiny_http::Request;

pub(in super::super::super) fn render(
    output: &Path,
    outcome: DownloadOutcome,
) -> Result<String, Box<dyn Error>> {
    let (heading, detail, status, toolbar, media_route) = match outcome {
        DownloadOutcome::Duplicate => (
            "Already downloaded",
            "The same X video was already in your RustDL folder, so it was not downloaded twice.",
            "Ready locally",
            "Ready to play",
            "media",
        ),
        DownloadOutcome::InProgress => (
            "Streaming now.",
            "This video is already downloading. Playback reads from the file as new bytes arrive.",
            "Download active",
            "Building while you watch",
            "stream",
        ),
        DownloadOutcome::Started => (
            "Streaming now.",
            "RustDL is saving the video in the background. You can start watching while the rest downloads.",
            "Downloading",
            "Building while you watch",
            "stream",
        ),
    };
    let growing = if outcome == DownloadOutcome::Duplicate {
        "false"
    } else {
        "true"
    };
    let filename = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("downloaded file has an invalid name")?;
    let transition_name = local::web_assets::view_transition_name(filename);
    let saved_path = local::html::escape_html(&local::files::display_output_path(output));
    let dev_reload = local::dev::dev_reload_script();
    let player_css = local::web_assets::player_css_path();
    let playback_script = local::web_assets::playback_script_tag();
    let view_transition_script = local::web_assets::view_transition_script_tag();
    let body = format!(
        include_str!("../../../assets/html/download-result.html"),
        detail = detail,
        dev_reload = dev_reload,
        filename = filename,
        growing = growing,
        heading = heading,
        media_route = media_route,
        playback_script = playback_script,
        player_css = player_css,
        saved_path = saved_path,
        status = status,
        toolbar = toolbar,
        transition_name = transition_name,
        view_transition_script = view_transition_script
    );
    Ok(body)
}

pub(in super::super::super) fn respond(
    request: Request,
    output: &Path,
    outcome: DownloadOutcome,
) -> Result<(), Box<dyn Error>> {
    let body = local::pages::download_result::render(output, outcome)?;
    request.respond(local::html::html_response(body))?;
    Ok(())
}
