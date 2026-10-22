//! Local discovery sessions and selection-page rendering.

use super::super::local;
use super::super::local::models::{DiscoveryCandidate, ResolvedVideo};
use super::super::local::youtube::{YouTubePlaylist, YouTubePlaylistEntry};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tiny_http::{Request, Response, StatusCode};

#[derive(Debug)]
pub(in super::super) struct DiscoverySession {
    pub(in super::super) created: u64,
    pub(in super::super) candidates: Vec<DiscoveryCandidate>,
}

#[derive(Debug)]
pub(in super::super) struct PlaylistSession {
    pub(in super::super) created: u64,
    pub(in super::super) playlist_id: String,
    pub(in super::super) title: String,
    pub(in super::super) entries: Vec<YouTubePlaylistEntry>,
}

pub(in super::super) type DiscoverySessions = HashMap<String, Arc<DiscoverySession>>;

pub(in super::super) static DISCOVERY_SESSIONS: OnceLock<Mutex<DiscoverySessions>> =
    OnceLock::new();

pub(in super::super) static PLAYLIST_SESSIONS: OnceLock<
    Mutex<HashMap<String, Arc<PlaylistSession>>>,
> = OnceLock::new();

pub(in super::super) fn store_discovery_session(candidates: Vec<DiscoveryCandidate>) -> String {
    store_shared_discovery_session(candidates).0
}

pub(in super::super) fn store_shared_discovery_session(
    candidates: Vec<DiscoveryCandidate>,
) -> (String, Arc<DiscoverySession>) {
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
    let session = Arc::new(DiscoverySession {
        created,
        candidates,
    });
    sessions.insert(token.clone(), Arc::clone(&session));
    (token, session)
}

pub(in super::super) fn store_playlist_session(
    playlist: YouTubePlaylist,
) -> (String, Arc<PlaylistSession>) {
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
    let session = Arc::new(PlaylistSession {
        created,
        playlist_id: playlist.playlist_id,
        title: playlist.title,
        entries: playlist.entries,
    });
    sessions.insert(token.clone(), Arc::clone(&session));
    (token, session)
}

pub(in super::super) fn respond_playlist_selection_page(
    request: Request,
    playlist: YouTubePlaylist,
) -> Result<(), Box<dyn Error>> {
    if playlist.entries.is_empty() {
        return local::html::respond_text(request, 404, "No selectable playlist videos were found");
    }
    let (token, session) = local::discovery::store_playlist_session(playlist);
    let body = local::discovery::render_playlist_selection(&token, &session)?;
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

// Keep a compact data model in the page; playlist.js materializes only 32 rows.
pub(in super::super) fn render_playlist_selection(
    token: &str,
    session: &PlaylistSession,
) -> Result<String, Box<dyn Error>> {
    let playlist_title = local::html::escape_html(&session.title);
    let entries = local::html::safe_script_json(&session.entries)?;
    Ok(format!(
        include_str!("../../assets/html/playlist.html"),
        DISCOVERY_CSS = DISCOVERY_CSS,
        playlist_title = playlist_title,
        dev_reload = local::dev::dev_reload_script(),
        page_css = include_str!("../../assets/css/playlist.css"),
        page_script = include_js!("playlist.js"),
        entries = entries,
        token = local::html::escape_html(token),
        count = session.entries.len(),
    ))
}

pub(in super::super) const DISCOVERY_CSS: &str = include_str!("../../assets/css/discovery.css");

pub(in super::super) const BULK_QUALITY_SCRIPT: &str = include_js!("bulk-quality.js");

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
    let Some(body) = render_quality_selection(picks) else {
        return local::html::respond_text(request, 400, "Select at least one video");
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

struct QualitySelection {
    token: String,
    index: usize,
    session: Arc<DiscoverySession>,
}

impl QualitySelection {
    fn candidate(&self) -> &DiscoveryCandidate {
        &self.session.candidates[self.index]
    }
}

fn select_quality_candidates(
    picks: &[String],
    sessions: &DiscoverySessions,
) -> Vec<QualitySelection> {
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for pick in picks {
        let Some((token, index)) = pick.split_once(':') else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        let Some(session) = sessions.get(token) else {
            continue;
        };
        let Some(candidate) = session.candidates.get(index) else {
            continue;
        };
        if seen.insert(candidate.resolved.filename.as_str()) {
            selected.push(QualitySelection {
                token: token.to_owned(),
                index,
                session: Arc::clone(session),
            });
        }
    }
    selected
}

// Retain immutable sessions while rendering, then release the map lock. Large
// candidate format lists and provider URLs stay in their original allocation.
pub(in super::super) fn render_quality_selection(picks: &[String]) -> Option<String> {
    let selected = {
        let sessions = DISCOVERY_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
        let sessions = sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        select_quality_candidates(picks, &sessions)
    };
    render_selected_quality(&selected)
}

fn render_selected_quality(selected: &[QualitySelection]) -> Option<String> {
    if selected.is_empty() {
        return None;
    }
    let count = selected.len();
    let cards = selected
        .iter()
        .map(|selection| {
            let candidate = selection.candidate();
            let options = candidate
                .qualities
                .iter()
                .enumerate()
                .map(|(quality_index, quality)| {
                    local::discovery::render_quality_option(&selection.token, selection.index, quality_index, quality)
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
                include_js!("list-pagination.js"),
                BULK_QUALITY_SCRIPT
            ),
            DISCOVERY_CSS = DISCOVERY_CSS,
            dev_reload = dev_reload,
            page_css = include_str!("../../assets/css/quality.css"),
            cards = cards,
            count = count
        )
    };
    Some(body)
}

#[cfg(test)]
mod playlist_preview_tests {
    use super::*;

    #[test]
    fn preview_moves_entries_into_one_shared_session_and_embeds_safe_data() {
        let entries = vec![YouTubePlaylistEntry {
            video_id: "AAAAAAAAAAA".to_owned(),
            title: "</script><img src=x onerror=1> & 雪".to_owned(),
            author: "Creator".to_owned(),
        }];
        let pointer = entries.as_ptr();
        let (token, session) = store_playlist_session(YouTubePlaylist {
            playlist_id: "PLsynthetic".to_owned(),
            title: "A <playlist>".to_owned(),
            entries,
        });
        assert_eq!(session.entries.as_ptr(), pointer);
        let html = render_playlist_selection(&token, &session).unwrap();
        assert!(html.contains("A &lt;playlist&gt;"));
        assert!(!html.contains("</script><img"));
        assert!(html.contains("\\u003c/script\\u003e"));
        assert!(!html.contains("<label class=\"candidate\">"));
        assert!(html.contains("id=\"playlist-data\""));
        let mut sessions = PLAYLIST_SESSIONS.get().unwrap().lock().unwrap();
        assert!(Arc::ptr_eq(sessions.get(&token).unwrap(), &session));
        sessions.remove(&token);
    }
}

#[cfg(test)]
mod quality_memory_tests {
    use super::*;

    fn candidate(filename: &str) -> DiscoveryCandidate {
        let resolved = ResolvedVideo {
            filename: filename.to_owned(),
            media_url: "https://example.invalid/private-stream".to_owned(),
            audio_url: None,
            extract_audio: false,
            quality_label: Some("720p <video>".to_owned()),
            quality_height: Some(720),
        };
        let mut audio = resolved.clone();
        audio.filename = filename.replace(".mp4", ".m4a");
        audio.extract_audio = true;
        audio.quality_label = Some("Audio & sound".to_owned());
        audio.quality_height = None;
        DiscoveryCandidate {
            qualities: vec![resolved.clone(), audio],
            resolved,
            source_url: "https://example.invalid/private-source".to_owned(),
            author: "Synthetic <creator>".to_owned(),
            text: String::new(),
            playlist: None,
        }
    }

    #[test]
    fn discovery_storage_moves_candidates_into_one_shared_session() {
        let candidates = vec![candidate("synthetic-owned.mp4")];
        let buffer = candidates.as_ptr();
        let qualities = candidates[0].qualities.as_ptr();
        let (token, session) = store_shared_discovery_session(candidates);
        assert_eq!(session.candidates.as_ptr(), buffer);
        assert_eq!(session.candidates[0].qualities.as_ptr(), qualities);
        let mut sessions = DISCOVERY_SESSIONS.get().unwrap().lock().unwrap();
        assert!(Arc::ptr_eq(sessions.get(&token).unwrap(), &session));
        sessions.remove(&token);
    }

    #[test]
    fn quality_selection_shares_formats_preserves_order_and_survives_eviction() {
        let session = Arc::new(DiscoverySession {
            created: 0,
            candidates: vec![
                candidate("synthetic-same.mp4"),
                candidate("synthetic-second.mp4"),
            ],
        });
        let other = Arc::new(DiscoverySession {
            created: 0,
            candidates: vec![candidate("synthetic-same.mp4")],
        });
        let sessions = DiscoverySessions::from([
            ("opaque".to_owned(), Arc::clone(&session)),
            ("other".to_owned(), Arc::clone(&other)),
        ]);
        let picks = [
            "missing:0",
            "opaque:x",
            "opaque:99",
            "opaque:0:2",
            "opaque:1",
            "other:0",
            "opaque:0",
            "opaque:1",
        ]
        .map(str::to_owned);
        let selected = select_quality_candidates(&picks, &sessions);
        assert_eq!(selected.len(), 2);
        assert_eq!((&*selected[0].token, selected[0].index), ("opaque", 1));
        assert_eq!((&*selected[1].token, selected[1].index), ("other", 0));
        assert!(Arc::ptr_eq(&selected[0].session, &session));
        assert!(Arc::ptr_eq(&selected[1].session, &other));
        assert_eq!(
            selected[0].candidate().qualities.as_ptr(),
            session.candidates[1].qualities.as_ptr()
        );
        drop(sessions);
        drop(session);
        drop(other);
        let html = render_selected_quality(&selected).unwrap();
        assert_eq!(html.matches("class=\"quality-card\"").count(), 2);
        assert!(html.contains("value=\"opaque:1:0\""));
        assert!(html.contains("value=\"other:0:1\""));
        assert!(html.contains("Synthetic &lt;creator&gt;"));
        assert!(html.contains("720p &lt;video&gt;"));
        assert!(html.contains("Audio &amp; sound"));
        assert!(!html.contains("private-stream"));
        assert!(!html.contains("private-source"));
    }

    #[test]
    fn empty_or_expired_quality_selections_remain_empty() {
        let sessions = DiscoverySessions::new();
        let picks = ["expired:0".to_owned()];
        assert!(select_quality_candidates(&picks, &sessions).is_empty());
        assert!(render_selected_quality(&[]).is_none());
        assert!(render_quality_selection(&[]).is_none());
    }
}
