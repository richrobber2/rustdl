//! Streaming catalog, calendar, and manifest request orchestration.

use super::super::{external, local, workflows};
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use tiny_http::{Request, Response, StatusCode};

pub(in super::super) fn respond_aniwaves_catalog_page(
    request: Request,
    client: &Client,
    state_dir: &Path,
    view: local::aniwaves::CatalogView,
    page: u16,
    refresh: bool,
) -> Result<(), Box<dyn Error>> {
    let library = local::streaming_library::load(state_dir)?;
    let watchlisted = local::streaming_library::urls(&library.entries);
    let html = match external::aniwaves::load_catalog(client, view.clone(), page, refresh) {
        Ok(catalog) => {
            local::aniwaves::render_catalog(&catalog, &watchlisted, local::security::action_token())
        }
        Err(error) => local::aniwaves::render_error(&error.to_string(), &view),
    };
    let response = Response::from_string(local::web_assets::decorate_app_html(html))
        .with_status_code(StatusCode(200))
        .with_header(local::html::header("Content-Type", "text/html; charset=utf-8"))
        .with_header(local::html::header(
            "Content-Security-Policy",
            "default-src 'none'; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; img-src 'self' https://static.aniwaves.ru; connect-src 'self'; form-action 'self'; base-uri 'none'",
        ))
        .with_header(local::html::header("Cache-Control", "no-store"))
        .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

pub(in super::super) fn load_watchlist_schedules(
    client: &Client,
    entries: &[local::streaming_library::WatchlistEntry],
    refresh: bool,
) -> HashMap<String, Result<local::aniwaves::StreamSchedule, String>> {
    let urls = entries
        .iter()
        .map(|entry| entry.watch_url.clone())
        .collect::<Vec<_>>();
    if urls.is_empty() {
        return HashMap::new();
    }
    let next = AtomicUsize::new(0);
    let results = Mutex::new(HashMap::with_capacity(urls.len()));
    thread::scope(|scope| {
        for _ in 0..urls.len().min(4) {
            let next = &next;
            let results = &results;
            let urls = &urls;
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(watch_url) = urls.get(index) else {
                        break;
                    };
                    let schedule =
                        external::aniwaves::load_stream_schedule(client, watch_url, refresh)
                            .map_err(|error| error.to_string());
                    results
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .insert(watch_url.clone(), schedule);
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(in super::super) fn respond_streaming_calendar_page(
    request: Request,
    client: &Client,
    state_dir: &Path,
    refresh: bool,
) -> Result<(), Box<dyn Error>> {
    let library = local::streaming_library::load(state_dir)?;
    let schedules =
        workflows::streaming::load_watchlist_schedules(client, &library.entries, refresh);
    let now = local::streaming_library::now_unix();
    let (html, seen) = local::streaming_library::render_calendar(
        &library.entries,
        &schedules,
        now,
        library.calendar_visited_at,
    );
    local::streaming_library::mark_calendar_seen(state_dir, &seen, now)?;
    local::streaming_library::streaming_page_response(request, html)
}

pub(in super::super) fn respond_stream_manifest(
    request: Request,
    client: &Client,
    state_dir: &Path,
    watch_url: Option<&str>,
    episode: Option<&str>,
    refresh: bool,
    native_token: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let result = watch_url
        .ok_or_else(|| "missing AniWaves watch URL".into())
        .and_then(|watch_url| {
            if let Some(token) = native_token.filter(|_| !refresh) {
                local::native_streaming::manifest_for_decoder(token, watch_url, episode).ok_or_else(
                    || "Streaming selection expired. Return to anime and refresh.".into(),
                )
            } else {
                external::aniwaves::load_stream_manifest(client, watch_url, episode, refresh)
            }
        });
    let (status, body) = match result {
        Ok(manifest) => {
            let watchlisted = watch_url.is_some_and(|watch_url| {
                local::streaming_library::load(state_dir).is_ok_and(|library| {
                    library
                        .entries
                        .iter()
                        .any(|entry| entry.watch_url == watch_url)
                })
            });
            let mut value = serde_json::to_value(manifest)?;
            if let Some(object) = value.as_object_mut() {
                object.insert("watchlisted".to_owned(), watchlisted.into());
                object.insert(
                    "watchlistToken".to_owned(),
                    local::security::action_token().to_owned().into(),
                );
            }
            (StatusCode(200), serde_json::to_string(&value)?)
        }
        Err(error) => (
            StatusCode(502),
            serde_json::json!({ "error": error.to_string() }).to_string(),
        ),
    };
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(local::html::header(
            "Content-Type",
            "application/json; charset=utf-8",
        ))
        .with_header(local::html::header("Cache-Control", "no-store"))
        .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}
