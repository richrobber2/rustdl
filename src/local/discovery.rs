//! Local discovery sessions and selection-page rendering.

use super::super::local;
use super::super::local::models::{DiscoveryCandidate, ResolvedVideo};
use super::super::local::youtube::{YouTubePlaylist, YouTubePlaylistEntry};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tiny_http::{Request, Response, StatusCode};

#[derive(Clone, Debug)]
pub(in super::super) struct DiscoverySession {
    pub(in super::super) created: u64,
    pub(in super::super) candidates: Vec<DiscoveryCandidate>,
}

#[derive(Clone, Debug)]
pub(in super::super) struct PlaylistSession {
    pub(in super::super) created: u64,
    pub(in super::super) playlist_id: String,
    pub(in super::super) title: String,
    pub(in super::super) entries: Vec<YouTubePlaylistEntry>,
}

pub(in super::super) static DISCOVERY_SESSIONS: OnceLock<Mutex<HashMap<String, DiscoverySession>>> =
    OnceLock::new();

pub(in super::super) static PLAYLIST_SESSIONS: OnceLock<Mutex<HashMap<String, PlaylistSession>>> =
    OnceLock::new();

pub(in super::super) fn store_discovery_session(candidates: Vec<DiscoveryCandidate>) -> String {
    let token = local::security::random_token();
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let sessions = DISCOVERY_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut sessions = sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    while sessions.len() >= 8 {
        let Some(oldest) = sessions
            .iter()
            .min_by_key(|(_, session)| session.created)
            .map(|(token, _)| token.clone())
        else {
            break;
        };
        sessions.remove(&oldest);
    }
    sessions.insert(
        token.clone(),
        DiscoverySession {
            created,
            candidates,
        },
    );
    token
}

pub(in super::super) fn store_playlist_session(playlist: &YouTubePlaylist) -> String {
    let token = local::security::random_token();
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let sessions = PLAYLIST_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut sessions = sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    while sessions.len() >= 8 {
        let Some(oldest) = sessions
            .iter()
            .min_by_key(|(_, session)| session.created)
            .map(|(token, _)| token.clone())
        else {
            break;
        };
        sessions.remove(&oldest);
    }
    sessions.insert(
        token.clone(),
        PlaylistSession {
            created,
            playlist_id: playlist.playlist_id.clone(),
            title: playlist.title.clone(),
            entries: playlist.entries.clone(),
        },
    );
    token
}

pub(in super::super) fn respond_playlist_selection_page(
    request: Request,
    playlist: YouTubePlaylist,
) -> Result<(), Box<dyn Error>> {
    if playlist.entries.is_empty() {
        return local::html::respond_text(request, 404, "No selectable playlist videos were found");
    }
    let count = playlist.entries.len();
    let token = local::discovery::store_playlist_session(&playlist);
    let cards = playlist
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            format!(
                r#"<label class="candidate"><input type="checkbox" name="pick" value="{token}:{index}"><span class="check">✓</span><span class="candidate-copy"><strong><i>#{}</i> {}</strong><span>{}</span><code>youtube-{}.mp4</code></span></label>"#,
                index + 1,
                local::html::escape_html(&entry.title),
                local::html::escape_html(&entry.author),
                entry.video_id
            )
        })
        .collect::<String>();
    let body = {
        let playlist_title = &(local::html::escape_html(&playlist.title));
        let dev_reload = &(local::dev::dev_reload_script());
        format!(
            include_str!("../../assets/html/playlist.html"),
            DISCOVERY_CSS = DISCOVERY_CSS,
            playlist_title = playlist_title,
            dev_reload = dev_reload,
            page_css = include_str!("../../assets/css/playlist.css"),
            page_script = format!(
                "{}\n{}",
                include_str!("../../assets/js/list-pagination.js"),
                include_str!("../../assets/js/playlist.js")
            ),
            cards = cards,
            count = count
        )
    };
    let response = Response::from_string(local::web_assets::decorate_app_html(body))
        .with_status_code(StatusCode(200))
        .with_header(local::html::header(
            "Content-Type",
            "text/html; charset=utf-8",
        ))
        .with_header(local::html::html_csp())
        .with_header(local::html::header("Cache-Control", "no-store"))
        .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

pub(in super::super) const DISCOVERY_CSS: &str = include_str!("../../assets/css/discovery.css");

pub(in super::super) const BULK_QUALITY_SCRIPT: &str =
    include_str!("../../assets/js/bulk-quality.js");

pub(in super::super) fn render_quality_option(
    token: &str,
    index: usize,
    quality_index: usize,
    quality: &ResolvedVideo,
) -> String {
    let kind = if local::media::is_audio_filename(&quality.filename) {
        "audio"
    } else {
        "video"
    };
    let height = quality.quality_height.unwrap_or(0);
    format!(
        r#"<option value="{token}:{index}:{quality_index}" data-kind="{kind}" data-height="{height}">{}</option>"#,
        local::html::escape_html(quality.quality_label.as_deref().unwrap_or("Original"))
    )
}

pub(in super::super) fn respond_quality_page(
    request: Request,
    picks: &[String],
) -> Result<(), Box<dyn Error>> {
    let sessions = DISCOVERY_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let sessions = sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for pick in picks {
        let Some((token, index)) = pick.split_once(':') else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        let Some(candidate) = sessions
            .get(token)
            .and_then(|session| session.candidates.get(index))
        else {
            continue;
        };
        if seen.insert(candidate.resolved.filename()) {
            selected.push((token.to_owned(), index, candidate.clone()));
        }
    }
    drop(sessions);
    if selected.is_empty() {
        return local::html::respond_text(request, 400, "Select at least one video");
    }
    let count = selected.len();
    let cards = selected
        .iter()
        .map(|(token, index, candidate)| {
            let options = candidate
                .qualities
                .iter()
                .enumerate()
                .map(|(quality_index, quality)| {
                    local::discovery::render_quality_option(token, *index, quality_index, quality)
                })
                .collect::<String>();
            format!(
                r#"<article class="quality-card"><div><strong>{}</strong><code>{}</code></div><label>Format &amp; quality<select name="pick">{options}</select></label></article>"#,
                local::html::escape_html(&candidate.author),
                local::html::escape_html(&candidate.resolved.filename()),
            )
        })
        .collect::<String>();
    let body = {
        let dev_reload = &(local::dev::dev_reload_script());
        format!(
            include_str!("../../assets/html/quality.html"),
            BULK_QUALITY_SCRIPT = format!(
                "{}\n{}",
                include_str!("../../assets/js/list-pagination.js"),
                BULK_QUALITY_SCRIPT
            ),
            DISCOVERY_CSS = DISCOVERY_CSS,
            dev_reload = dev_reload,
            page_css = include_str!("../../assets/css/quality.css"),
            cards = cards,
            count = count
        )
    };
    let response = Response::from_string(local::web_assets::decorate_app_html(body))
        .with_status_code(StatusCode(200))
        .with_header(local::html::header(
            "Content-Type",
            "text/html; charset=utf-8",
        ))
        .with_header(local::html::html_csp())
        .with_header(local::html::header("Cache-Control", "no-store"))
        .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}
