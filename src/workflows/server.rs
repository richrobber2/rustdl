//! Application server startup and request dispatch.

use super::super::local::cli::ServeArgs;
use super::super::local::inspection::InspectionScreen;
use super::super::local::web_assets::{APPEARANCE_SCRIPT, DARK_SPACE_BACKGROUND, PLAYER_CSS};
use super::super::{external, local, workflows};
use reqwest::Url;
use reqwest::blocking::Client;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::{fs, thread};
use tiny_http::{Method, Request, Response, Server, StatusCode};

pub(in super::super) fn serve(args: ServeArgs) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(&args.output_dir)?;
    let server = Server::http(&args.bind)
        .map_err(|error| format!("could not bind {}: {error}", args.bind))?;
    let client = external::http::build_client()?;
    workflows::downloads::initialize_download_queue(&client, &args.output_dir)?;
    if !local::runtime::inspection_mode() {
        workflows::peers::start_peer_server(
            local::peers::peer_bind_for(&args.bind)?,
            args.output_dir.clone(),
        );
    }
    eprintln!("rustdl web server listening at http://{}", args.bind);
    eprintln!("Downloads will be saved in {}", args.output_dir.display());
    eprintln!("Press Ctrl+C to stop.");

    for request in server.incoming_requests() {
        let request_client = client.clone();
        let output_dir = args.output_dir.clone();
        thread::spawn(move || {
            if let Err(error) =
                workflows::server::handle_request(request, &request_client, &output_dir)
            {
                eprintln!("request error: {error}");
            }
        });
    }
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn run_embedded_server(bind: String, output_dir: PathBuf) {
    if let Err(error) = workflows::server::serve(ServeArgs { bind, output_dir }) {
        eprintln!("embedded server error: {error}");
    }
}

pub(in super::super) fn handle_request(
    mut request: Request,
    client: &Client,
    output_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    let method = request.method().clone();
    let request_url = request.url().to_owned();
    eprintln!("{method} {request_url}");

    let parsed = Url::parse(&format!("http://localhost{request_url}"))?;
    if method == Method::Post
        && parsed.path() == "/storage/action"
        && !local::runtime::inspection_mode()
    {
        return local::storage::respond_storage_action(request, output_dir);
    }
    if method == Method::Post
        && parsed.path() == "/peers/send/start"
        && !local::runtime::inspection_mode()
    {
        return workflows::peers::respond_peer_send_post(request, client, output_dir);
    }
    if method == Method::Post
        && parsed.path() == "/peers/send/paired"
        && !local::runtime::inspection_mode()
    {
        return workflows::peers::respond_peer_send_paired_post(request, client, output_dir);
    }
    if method == Method::Post
        && parsed.path() == "/__app/watchlist"
        && !local::runtime::inspection_mode()
    {
        return local::streaming_library::respond_watchlist_action(request, output_dir);
    }
    if method == Method::Post
        && matches!(
            parsed.path(),
            "/playlist/quality" | "/import" | "/playlist/cancel"
        )
        && !local::runtime::inspection_mode()
    {
        use std::io::Read;
        const MAX_SELECTION_BODY: u64 = 8 * 1024 * 1024;
        let mut body = String::new();
        request
            .as_reader()
            .take(MAX_SELECTION_BODY + 1)
            .read_to_string(&mut body)?;
        if body.len() as u64 > MAX_SELECTION_BODY {
            return local::html::respond_text(request, 413, "Selection is too large");
        }
        let form = Url::parse(&format!("http://localhost/?{body}"))?;
        if parsed.path() == "/playlist/cancel" {
            let token = form
                .query_pairs()
                .find(|(key, _)| key == "job")
                .map(|(_, value)| value.into_owned())
                .unwrap_or_default();
            return local::playlist_resolution::respond_cancel(request, &token);
        }
        let picks = form
            .query_pairs()
            .filter(|(key, _)| key == "pick")
            .map(|(_, value)| value.into_owned())
            .collect::<Vec<_>>();
        return if parsed.path() == "/playlist/quality" {
            workflows::discovery::respond_playlist_quality_page(request, &picks)
        } else {
            workflows::discovery::respond_discovery_import(request, client, output_dir, &picks)
        };
    }
    if method != Method::Get {
        return local::html::respond_text(request, 405, "Method not allowed");
    }
    match parsed.path() {
        "/__app/mode" => local::html::respond_text(
            request,
            200,
            if local::runtime::inspection_mode() {
                "inspection"
            } else {
                "normal"
            },
        ),
        "/__dev/version" => local::dev::respond_dev_version(request),
        "/__app/activity.json" if !local::runtime::inspection_mode() => {
            local::activity_state::respond_state(request)
        }
        "/__app/state.json" => {
            let filename = parsed
                .query_pairs()
                .find(|(key, _)| key == "file")
                .map(|(_, value)| value.into_owned());
            local::app_state::respond_app_state(request, output_dir, filename.as_deref())
        }
        path if path == local::web_assets::view_transition_script_path() => {
            local::web_assets::respond_view_transition_script(request)
        }
        path if path == local::web_assets::appearance_script_path() => {
            local::web_assets::respond_immutable_asset(
                request,
                APPEARANCE_SCRIPT,
                "application/javascript; charset=utf-8",
            )
        }
        path if path == local::web_assets::dark_space_background_path() => {
            local::web_assets::respond_immutable_binary_asset(
                request,
                DARK_SPACE_BACKGROUND,
                "image/png",
            )
        }
        path if path == local::web_assets::rainy_city_background_path() => {
            local::web_assets::respond_immutable_binary_asset(
                request,
                local::web_assets::RAINY_CITY_BACKGROUND,
                "image/webp",
            )
        }
        path if path == local::web_assets::rainy_city_light_background_path() => {
            local::web_assets::respond_immutable_binary_asset(
                request,
                local::web_assets::RAINY_CITY_LIGHT_BACKGROUND,
                "image/webp",
            )
        }
        path if local::web_assets::sunrise_paths()
            .iter()
            .any(|asset| asset == path) =>
        {
            let index = local::web_assets::sunrise_paths()
                .iter()
                .position(|asset| asset == path)
                .unwrap();
            local::web_assets::respond_immutable_binary_asset(
                request,
                local::web_assets::SUNRISE_FRAMES[index],
                "image/webp",
            )
        }
        path if path == local::web_assets::appearance_css_path() => {
            local::web_assets::respond_immutable_asset(
                request,
                local::web_assets::appearance_css(),
                "text/css; charset=utf-8",
            )
        }
        path if path == local::web_assets::playback_script_path()
            && !local::runtime::inspection_mode() =>
        {
            local::web_assets::respond_playback_script(request)
        }
        path if path == local::web_assets::index_css_path() => {
            local::web_assets::respond_immutable_asset(
                request,
                local::web_assets::index_css(),
                "text/css; charset=utf-8",
            )
        }
        path if path == local::web_assets::player_css_path() => {
            local::web_assets::respond_immutable_asset(
                request,
                PLAYER_CSS,
                "text/css; charset=utf-8",
            )
        }
        "/__app/gallery-metrics.json" if !local::runtime::inspection_mode() => {
            local::gallery::respond_gallery_metrics(request)
        }
        "/__app/playback-order.json" if !local::runtime::inspection_mode() => {
            local::gallery::respond_playback_order(request, output_dir)
        }
        "/__app/stream-manifest.json" if !local::runtime::inspection_mode() => {
            let watch_url = parsed
                .query_pairs()
                .find(|(key, _)| key == "url")
                .map(|(_, value)| value.into_owned());
            let episode = parsed
                .query_pairs()
                .find(|(key, _)| key == "episode")
                .map(|(_, value)| value.into_owned());
            let refresh = parsed
                .query_pairs()
                .any(|(key, value)| key == "refresh" && value == "1");
            workflows::streaming::respond_stream_manifest(
                request,
                client,
                output_dir,
                watch_url.as_deref(),
                episode.as_deref(),
                refresh,
            )
        }
        "/__inspect/result" if local::runtime::inspection_mode() => {
            local::inspection::respond_inspection_page(request, InspectionScreen::Result)
        }
        "/__inspect/player" if local::runtime::inspection_mode() => {
            local::inspection::respond_inspection_page(request, InspectionScreen::Player)
        }
        "/__inspect/poster.svg" if local::runtime::inspection_mode() => {
            local::inspection::respond_inspection_poster(request)
        }
        "/__inspect/capture.png" if local::runtime::inspection_mode() => {
            local::inspection::respond_inspection_capture(request, output_dir)
        }
        path if path.starts_with("/thumbnail/") && !local::runtime::inspection_mode() => {
            local::thumbnails::respond_thumbnail(request, output_dir, &path[11..])
        }
        "/" => {
            let response = Response::from_string(local::web_assets::decorate_app_html(
                local::pages::gallery::render(output_dir)?,
            ))
            .with_status_code(StatusCode(200))
            .with_header(local::html::header(
                "Content-Type",
                "text/html; charset=utf-8",
            ))
            .with_header(local::html::html_csp())
            .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
            request.respond(response)?;
            Ok(())
        }
        path if path.starts_with("/gallery/playlist/") && !local::runtime::inspection_mode() => {
            let playlist_id = &path[18..];
            if !local::youtube::valid_playlist_id(playlist_id) {
                return local::html::respond_text(request, 404, "Playlist folder not found");
            }
            let response = Response::from_string(local::web_assets::decorate_app_html(
                local::pages::gallery::render_playlist(output_dir, Some(playlist_id))?,
            ))
            .with_status_code(StatusCode(200))
            .with_header(local::html::header(
                "Content-Type",
                "text/html; charset=utf-8",
            ))
            .with_header(local::html::html_csp())
            .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
            request.respond(response)?;
            Ok(())
        }
        "/activity" if !local::runtime::inspection_mode() => {
            local::activity_state::respond_page(request)
        }
        "/diagnostics" if !local::runtime::inspection_mode() => {
            local::pages::diagnostics::respond(request)
        }
        "/settings" if !local::runtime::inspection_mode() => {
            local::pages::settings::respond(request)
        }
        "/changelog" => local::pages::changelog::respond(request),
        "/streaming" if !local::runtime::inspection_mode() => {
            let page = parsed
                .query_pairs()
                .find(|(key, _)| key == "page")
                .and_then(|(_, value)| value.parse::<u16>().ok())
                .unwrap_or(1);
            let section = parsed
                .query_pairs()
                .find(|(key, _)| key == "section")
                .map(|(_, value)| value.into_owned());
            let query = parsed
                .query_pairs()
                .find(|(key, _)| key == "q")
                .map(|(_, value)| value.into_owned());
            let view = match local::aniwaves::CatalogView::from_params(
                section.as_deref(),
                query.as_deref(),
            ) {
                Ok(view) => view,
                Err(error) => return local::html::respond_text(request, 400, error),
            };
            let refresh = parsed
                .query_pairs()
                .any(|(key, value)| key == "refresh" && value == "1");
            workflows::streaming::respond_aniwaves_catalog_page(
                request, client, output_dir, view, page, refresh,
            )
        }
        "/streaming/watchlist" if !local::runtime::inspection_mode() => {
            local::streaming_library::respond_streaming_watchlist_page(request, output_dir)
        }
        "/streaming/calendar" if !local::runtime::inspection_mode() => {
            let refresh = parsed
                .query_pairs()
                .any(|(key, value)| key == "refresh" && value == "1");
            workflows::streaming::respond_streaming_calendar_page(
                request, client, output_dir, refresh,
            )
        }
        "/peers/refresh" if !local::runtime::inspection_mode() => {
            local::peers::respond_peer_pairing_refresh(request, local::peers::args_peer_port())
        }
        "/peers" if !local::runtime::inspection_mode() => {
            local::peers::respond_peer_receive_page(request, local::peers::args_peer_port())
        }
        "/peers/connected" if !local::runtime::inspection_mode() => {
            local::peers::respond_peer_connected_page(request, output_dir)
        }
        "/peers/send" if !local::runtime::inspection_mode() => {
            let filename = parsed
                .query_pairs()
                .find(|(key, _)| key == "file")
                .map(|(_, value)| value.into_owned());
            local::peers::respond_peer_send_page(request, output_dir, filename.as_deref())
        }
        "/__peer/state" if !local::runtime::inspection_mode() => {
            local::peers::respond_peer_send_state(request)
        }
        "/download" => {
            if local::runtime::inspection_mode() {
                return local::inspection::respond_inspection_page(
                    request,
                    InspectionScreen::Result,
                );
            }
            let submitted = parsed
                .query_pairs()
                .find(|(key, _)| key == "urls" || key == "url")
                .map(|(_, value)| value.into_owned());
            let Some(submitted) = submitted.filter(|value| !value.trim().is_empty()) else {
                return local::html::respond_text(request, 400, "Missing video URL");
            };
            let urls = local::sources::extract_download_urls(&submitted);
            if urls.is_empty() {
                return local::html::respond_text(
                    request,
                    422,
                    "No supported video URLs were found",
                );
            }
            if urls.len() == 1 {
                return match workflows::downloads::start_web_download(client, &urls[0], output_dir)
                {
                    Ok((output, outcome)) => {
                        local::pages::download_result::respond(request, &output, outcome)
                    }
                    Err(error) => local::html::respond_text(
                        request,
                        422,
                        &format!("Download failed: {error}"),
                    ),
                };
            }
            let mut errors = Vec::new();
            for source_url in urls {
                if let Err(error) =
                    workflows::downloads::start_web_download(client, &source_url, output_dir)
                {
                    errors.push(format!(
                        "{}: {error}",
                        local::html::escape_html(&source_url)
                    ));
                }
            }
            local::queue::respond_queue_page(request, &errors)
        }
        "/discover" if !local::runtime::inspection_mode() => {
            let submitted = parsed
                .query_pairs()
                .find(|(key, _)| key == "source" || key == "urls" || key == "url")
                .map(|(_, value)| value.into_owned());
            let Some(submitted) = submitted.filter(|value| !value.trim().is_empty()) else {
                return local::html::respond_text(request, 400, "Missing video link");
            };
            if let Some(target) = local::aniwaves::catalog_target_from_text(&submitted) {
                return workflows::streaming::respond_aniwaves_catalog_page(
                    request,
                    client,
                    output_dir,
                    target.view,
                    target.page,
                    false,
                );
            }
            let sources = local::sources::extract_supported_urls(&submitted);
            if let Some(playlist_id) =
                sources
                    .iter()
                    .find_map(|source| match local::sources::classify_url(source) {
                        local::sources::SourceUrl::YouTubePlaylist { playlist_id } => {
                            Some(playlist_id)
                        }
                        _ => None,
                    })
            {
                if sources.len() != 1 {
                    return local::html::respond_text(
                        request,
                        422,
                        "Open one playlist at a time so its entries can be selected",
                    );
                }
                return match external::youtube::fetch_playlist_entries(client, &playlist_id) {
                    Ok(playlist) => {
                        local::discovery::respond_playlist_selection_page(request, playlist)
                    }
                    Err(error) => local::html::respond_text(
                        request,
                        422,
                        &format!("Playlist discovery failed: {error}"),
                    ),
                };
            }
            match workflows::discovery::discover_videos(client, &submitted) {
                Ok(candidates) => workflows::discovery::respond_discovery_page(request, candidates),
                Err(error) => {
                    local::html::respond_text(request, 422, &format!("Discovery failed: {error}"))
                }
            }
        }
        "/playlist/resolution" | "/__app/playlist-resolution.json" | "/playlist/formats"
            if !local::runtime::inspection_mode() =>
        {
            let token = parsed
                .query_pairs()
                .find(|(key, _)| key == "job")
                .map(|(_, value)| value.into_owned())
                .unwrap_or_default();
            match parsed.path() {
                "/playlist/resolution" => local::playlist_resolution::respond_page(request, &token),
                "/playlist/formats" => local::playlist_resolution::respond_formats(request, &token),
                _ => local::playlist_resolution::respond_status(request, &token),
            }
        }
        "/playlist/quality" if !local::runtime::inspection_mode() => {
            let picks = parsed
                .query_pairs()
                .filter(|(key, _)| key == "pick")
                .map(|(_, value)| value.into_owned())
                .collect::<Vec<_>>();
            workflows::discovery::respond_playlist_quality_page(request, &picks)
        }
        "/quality" if !local::runtime::inspection_mode() => {
            let picks = parsed
                .query_pairs()
                .filter(|(key, _)| key == "pick")
                .map(|(_, value)| value.into_owned())
                .collect::<Vec<_>>();
            local::discovery::respond_quality_page(request, &picks)
        }
        "/import" if !local::runtime::inspection_mode() => {
            let picks = parsed
                .query_pairs()
                .filter(|(key, _)| key == "pick")
                .map(|(_, value)| value.into_owned())
                .collect::<Vec<_>>();
            workflows::discovery::respond_discovery_import(request, client, output_dir, &picks)
        }
        "/queue" if !local::runtime::inspection_mode() => {
            local::queue::respond_queue_page(request, &[])
        }
        "/queue/action" if !local::runtime::inspection_mode() => {
            let filename = parsed
                .query_pairs()
                .find(|(key, _)| key == "file")
                .map(|(_, value)| value.into_owned());
            let action = parsed
                .query_pairs()
                .find(|(key, _)| key == "action")
                .map(|(_, value)| value.into_owned());
            let (Some(filename), Some(action)) = (filename, action) else {
                return local::html::respond_text(request, 400, "Missing queue action");
            };
            if let Err(error) =
                workflows::downloads::apply_queue_action(client, output_dir, &filename, &action)
            {
                return local::html::respond_text(request, 422, &error);
            }
            local::queue::respond_queue_page(request, &[])
        }
        "/storage" if !local::runtime::inspection_mode() => {
            local::storage::respond_storage_page(request, output_dir, None)
        }
        "/storage/confirm" if !local::runtime::inspection_mode() => {
            let action = parsed
                .query_pairs()
                .find(|(key, _)| key == "action")
                .map(|(_, value)| value.into_owned());
            let filename = parsed
                .query_pairs()
                .find(|(key, _)| key == "file")
                .map(|(_, value)| value.into_owned());
            local::storage::respond_storage_confirmation(
                request,
                output_dir,
                action.as_deref(),
                filename.as_deref(),
            )
        }
        path if path.starts_with("/watch/") => {
            let filename = path.trim_start_matches("/watch/");
            local::pages::player::respond(request, output_dir, filename)
        }
        path if path.starts_with("/media/") => {
            let filename = path.trim_start_matches("/media/");
            local::media::respond_media(request, output_dir, filename)
        }
        path if path.starts_with("/stream/") => {
            let filename = path.trim_start_matches("/stream/");
            local::media::respond_growing_media(request, output_dir, filename)
        }
        _ => local::html::respond_text(request, 404, "Not found"),
    }
}
