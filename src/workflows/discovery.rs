//! Provider discovery and selection flows that can start downloads.

use super::super::local::discovery::{DISCOVERY_CSS, DISCOVERY_SESSIONS, PLAYLIST_SESSIONS};
use super::super::local::gallery::PlaylistMembership;
use super::super::local::models::{DiscoveryCandidate, ResolvedVideo};
use super::super::local::sources::SourceUrl;
use super::super::{external, local, workflows};
use reqwest::blocking::Client;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::Path;
use std::sync::Mutex;
use tiny_http::{Request, Response, StatusCode};

pub(in super::super) fn resolve_video(
    client: &Client,
    source_url: &str,
) -> Result<ResolvedVideo, Box<dyn Error>> {
    match local::sources::classify_url(source_url) {
        SourceUrl::YouTubeVideo { .. } => {
            Ok(external::youtube::resolve_candidate(source_url)?.resolved)
        }
        SourceUrl::SnapchatSpotlight { .. } => {
            Ok(external::snapchat::resolve_candidate(client, source_url)?.resolved)
        }
        SourceUrl::XPost { .. } => external::x::resolve_video(client, source_url),
        SourceUrl::YouTubePlaylist { .. } | SourceUrl::XProfile { .. } => {
            Err("Open this collection in discovery and select videos to download".into())
        }
        SourceUrl::Unsupported => {
            Err("expected a supported X, YouTube, or Snapchat Spotlight URL".into())
        }
    }
}

pub(in super::super) fn discover_videos(
    client: &Client,
    submitted: &str,
) -> Result<Vec<DiscoveryCandidate>, Box<dyn Error>> {
    let sources = local::sources::extract_supported_urls(submitted);
    if sources.is_empty() {
        return Err("no public X, YouTube, or Snapchat Spotlight links were found".into());
    }
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for source in sources.into_iter().take(10) {
        let candidate = match local::sources::classify_url(&source) {
            SourceUrl::YouTubeVideo { .. } => external::youtube::resolve_candidate(&source)?,
            SourceUrl::SnapchatSpotlight { .. } => {
                external::snapchat::resolve_candidate(client, &source)?
            }
            SourceUrl::XPost { .. } | SourceUrl::XProfile { .. } => {
                let statuses = external::x::fetch_statuses(client, &source)?;
                for status in statuses {
                    local::x::append_status_candidates(&status, &mut candidates, &mut seen);
                    if candidates.len() >= 50 {
                        break;
                    }
                }
                if candidates.len() >= 50 {
                    break;
                }
                continue;
            }
            SourceUrl::YouTubePlaylist { .. } => {
                return Err("Open one playlist at a time so its entries can be selected".into());
            }
            SourceUrl::Unsupported => continue,
        };
        if seen.insert(candidate.resolved.filename()) {
            candidates.push(candidate);
        }
    }
    Ok(candidates)
}

pub(in super::super) fn prepare_playlist_picks(picks: &[String]) -> Result<String, String> {
    let sessions = PLAYLIST_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
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
        let Some(session) = sessions.get(token) else {
            continue;
        };
        let Some(entry) = session.entries.get(index) else {
            continue;
        };
        if seen.insert(entry.video_id.clone()) {
            selected.push((
                entry.clone(),
                PlaylistMembership {
                    playlist_id: session.playlist_id.clone(),
                    title: session.title.clone(),
                    position: index + 1,
                    total: session.entries.len(),
                },
            ));
        }
    }
    drop(sessions);
    if selected.is_empty() {
        return Err("Select at least one playlist video".to_owned());
    }
    selected.sort_by_key(|(_, membership)| (membership.playlist_id.clone(), membership.position));
    workflows::playlist_resolution::start(selected)
}

pub(in super::super) fn respond_playlist_quality_page(
    request: Request,
    picks: &[String],
) -> Result<(), Box<dyn Error>> {
    match prepare_playlist_picks(picks) {
        Ok(token) => {
            request.respond(
                Response::empty(StatusCode(303)).with_header(local::html::header(
                    "Location",
                    &format!("/playlist/resolution?job={token}"),
                )),
            )?;
            Ok(())
        }
        Err(error) => local::html::respond_text(
            request,
            if error == "Select at least one playlist video" {
                400
            } else {
                429
            },
            &error,
        ),
    }
}

pub(in super::super) fn respond_discovery_page(
    request: Request,
    candidates: Vec<DiscoveryCandidate>,
) -> Result<(), Box<dyn Error>> {
    if candidates.is_empty() {
        return local::html::respond_text(request, 404, "No downloadable media was found");
    }
    let count = candidates.len();
    let token = local::discovery::store_discovery_session(candidates.clone());
    let cards = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let text = if candidate.text.trim().is_empty() {
                "Video post".to_owned()
            } else {
                local::format::truncate_text(&candidate.text, 180)
            };
            format!(
                r#"<label class="candidate"><input type="checkbox" name="pick" value="{token}:{index}" checked><span class="check">✓</span><span class="candidate-copy"><strong>{}</strong><span>{}</span><code>{}</code></span></label>"#,
                local::html::escape_html(&candidate.author),
                local::html::escape_html(&text),
                local::html::escape_html(&candidate.resolved.filename())
            )
        })
        .collect::<String>();
    let body = {
        let dev_reload = &(local::dev::dev_reload_script());
        format!(
            include_str!("../../assets/html/discovery.html"),
            DISCOVERY_CSS = DISCOVERY_CSS,
            dev_reload = dev_reload,
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

type SelectedDownload = (String, ResolvedVideo, Option<PlaylistMembership>);

fn selected_downloads(
    picks: &[String],
    sessions: &HashMap<String, local::discovery::DiscoverySession>,
) -> Vec<SelectedDownload> {
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for pick in picks {
        let mut parts = pick.split(':');
        let (Some(token), Some(index), Some(quality_index)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        let Ok(quality_index) = quality_index.parse::<usize>() else {
            continue;
        };
        let Some(candidate) = sessions
            .get(token)
            .and_then(|session| session.candidates.get(index))
        else {
            continue;
        };
        let Some(resolved) = candidate.qualities.get(quality_index) else {
            continue;
        };
        if seen.insert(resolved.filename()) {
            selected.push((
                candidate.source_url.clone(),
                resolved.clone(),
                candidate.playlist.clone(),
            ));
        }
    }
    selected
}

/// Shared import outcome, independent of HTML and HTTP request ownership.
pub(in super::super) struct ImportOutcome {
    pub(in super::super) selected: usize,
    pub(in super::super) errors: Vec<String>,
}

pub(in super::super) fn import_discovery_picks(
    client: &Client,
    output_dir: &Path,
    picks: &[String],
) -> ImportOutcome {
    let selected = {
        let sessions = DISCOVERY_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
        let sessions = sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        selected_downloads(picks, &sessions)
    };
    let count = selected.len();
    let mut errors = Vec::new();
    for (source_url, resolved, membership) in selected {
        let filename = resolved.filename();
        match workflows::downloads::start_resolved_download(
            client,
            &source_url,
            resolved,
            output_dir,
        ) {
            Ok(_) => {
                if let Some(membership) = membership
                    && let Err(error) = local::gallery::record_playlist_membership(
                        output_dir, &filename, membership,
                    )
                {
                    errors.push(format!(
                        "Could not group {filename} into its playlist: {error}"
                    ));
                }
            }
            Err(error) => errors.push(error.to_string()),
        }
    }
    ImportOutcome {
        selected: count,
        errors,
    }
}

pub(in super::super) fn respond_discovery_import(
    request: Request,
    client: &Client,
    output_dir: &Path,
    picks: &[String],
) -> Result<(), Box<dyn Error>> {
    let outcome = import_discovery_picks(client, output_dir, picks);
    if outcome.selected == 0 {
        return local::html::respond_text(request, 400, "Select at least one video");
    }
    let errors = outcome
        .errors
        .iter()
        .map(|error| local::html::escape_html(error))
        .collect::<Vec<_>>();
    local::queue::respond_queue_page(request, &errors)
}

#[cfg(test)]
mod native_import_tests {
    use super::*;
    #[test]
    fn native_import_selection_preserves_quality_and_membership_and_deduplicates() {
        let quality = ResolvedVideo {
            filename: "123-1.mp4".into(),
            media_url: "https://example.invalid/synthetic".into(),
            audio_url: None,
            extract_audio: false,
            quality_label: Some("720p".into()),
            quality_height: Some(720),
        };
        let membership = PlaylistMembership {
            playlist_id: "PLsynthetic".into(),
            title: "Synthetic collection".into(),
            position: 2,
            total: 3,
        };
        let candidate = DiscoveryCandidate {
            resolved: quality.clone(),
            qualities: vec![quality],
            source_url: "https://example.invalid/source".into(),
            author: "Synthetic".into(),
            text: "Synthetic candidate".into(),
            playlist: Some(membership),
        };
        let sessions = HashMap::from([(
            "opaque".into(),
            local::discovery::DiscoverySession {
                created: 0,
                candidates: vec![candidate],
            },
        )]);
        let picks = [
            "missing:0:0",
            "opaque:99:0",
            "opaque:0:99",
            "malformed",
            "opaque:0:0",
            "opaque:0:0",
        ]
        .map(str::to_owned);
        let selected = selected_downloads(&picks, &sessions);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].1.quality_height, Some(720));
        let membership = selected[0].2.as_ref().unwrap();
        assert_eq!(membership.position, 2);
        assert_eq!(membership.total, 3);
        assert!(selected_downloads(&[], &sessions).is_empty());
    }
}
