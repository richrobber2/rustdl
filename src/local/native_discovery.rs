//! Native discovery adapters use typed provider sessions, never HTML responses.
use super::super::{external, workflows};
use super::{discovery, models::DiscoveryCandidate};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    #[serde(default)]
    source: String,
    #[serde(default)]
    token: String,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    picks: Vec<String>,
    #[serde(default)]
    all: bool,
    #[serde(default)]
    format: String,
}

fn candidate_row(
    candidate: &DiscoveryCandidate,
    token: &str,
    index: usize,
    privacy: bool,
) -> Value {
    let qualities = candidate
        .qualities
        .iter()
        .enumerate()
        .map(|(quality, resolved)| {
            let label = if privacy {
                resolved
                    .quality_height
                    .map(|height| format!("{height}p"))
                    .unwrap_or_else(|| {
                        if resolved.extract_audio {
                            "Audio"
                        } else {
                            "Video"
                        }
                        .to_owned()
                    })
            } else {
                resolved
                    .quality_label
                    .clone()
                    .unwrap_or_else(|| "Original".to_owned())
            };
            json!({"id":format!("{token}:{index}:{quality}"), "label":label})
        })
        .collect::<Vec<_>>();
    json!({"id":format!("{token}:{index}"),
        "title":if privacy { "Media candidate" } else { &candidate.author },
        "detail":if privacy { "".to_owned() } else { super::format::truncate_text(&candidate.text,180) },
        "qualities":qualities})
}

fn playlist_row(
    entry: &super::youtube::YouTubePlaylistEntry,
    token: &str,
    index: usize,
    privacy: bool,
) -> Value {
    json!({"id":format!("{token}:{index}"),"title":if privacy {"Playlist item"} else {&entry.title},
        "detail":if privacy {""} else {&entry.author},"qualities":[]})
}

fn page(token: &str, offset: usize, privacy: bool) -> Result<Value, &'static str> {
    let playlists = discovery::PLAYLIST_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let playlists = playlists.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(session) = playlists.get(token) {
        let total = session.entries.len();
        let offset = offset.min(total.saturating_sub(1) / 25 * 25);
        let items = session
            .entries
            .iter()
            .enumerate()
            .skip(offset)
            .take(25)
            .map(|(index, entry)| playlist_row(entry, token, index, privacy))
            .collect::<Vec<_>>();
        return Ok(
            json!({"ok":true,"kind":"playlist","token":token,"offset":offset,"total":total,"items":items,
            "nextOffset":if offset+25<total {Some(offset+25)} else {None},
            "previousOffset":if offset>0 {Some(offset.saturating_sub(25))} else {None}}),
        );
    }
    drop(playlists);
    let sessions = discovery::DISCOVERY_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let sessions = sessions.lock().unwrap_or_else(|p| p.into_inner());
    let session = sessions
        .get(token)
        .ok_or("Selection expired. Discover the links again.")?;
    let total = session.candidates.len();
    let offset = offset.min(total.saturating_sub(1) / 25 * 25);
    let items = session
        .candidates
        .iter()
        .enumerate()
        .skip(offset)
        .take(25)
        .map(|(index, candidate)| candidate_row(candidate, token, index, privacy))
        .collect::<Vec<_>>();
    Ok(
        json!({"ok":true,"kind":"qualities","token":token,"offset":offset,"total":total,"items":items,
        "nextOffset":if offset+25<total {Some(offset+25)} else {None},
        "previousOffset":if offset>0 {Some(offset.saturating_sub(25))} else {None}}),
    )
}

fn chosen_quality(candidate: &DiscoveryCandidate, format: &str) -> usize {
    if format == "audio" {
        return candidate
            .qualities
            .iter()
            .position(|q| q.extract_audio)
            .unwrap_or(0);
    }
    if let Some(height) = format
        .strip_suffix('p')
        .and_then(|value| value.parse::<u32>().ok())
    {
        return candidate
            .qualities
            .iter()
            .enumerate()
            .filter(|(_, q)| !q.extract_audio)
            .min_by_key(|(_, q)| q.quality_height.unwrap_or(0).abs_diff(height))
            .map(|(i, _)| i)
            .unwrap_or(0);
    }
    0
}
fn bulk_selection(
    token: &str,
    all: bool,
    picks: &[String],
    format: &str,
) -> Result<Vec<String>, &'static str> {
    if !matches!(
        format,
        "original" | "audio" | "360p" | "480p" | "720p" | "1080p"
    ) {
        return Err("Invalid bulk format");
    }
    let included = |index: usize| {
        all || picks.iter().any(|pick| {
            pick == &format!("{token}:{index}") || pick.starts_with(&format!("{token}:{index}:"))
        })
    };
    let playlists = discovery::PLAYLIST_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let playlists = playlists.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(session) = playlists.get(token) {
        let selected = (0..session.entries.len())
            .filter(|index| included(*index))
            .map(|index| format!("{token}:{index}"))
            .collect::<Vec<_>>();
        return if selected.len() > 500 {
            Err("Select at most 500 items per batch")
        } else {
            Ok(selected)
        };
    }
    drop(playlists);
    let sessions = discovery::DISCOVERY_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let sessions = sessions.lock().unwrap_or_else(|p| p.into_inner());
    let session = sessions.get(token).ok_or("Selection expired")?;
    Ok(session
        .candidates
        .iter()
        .enumerate()
        .filter(|(index, c)| included(*index) && !c.qualities.is_empty())
        .map(|(index, c)| format!("{token}:{index}:{}", chosen_quality(c, format)))
        .collect())
}

fn preparation(token: &str) -> Result<Value, &'static str> {
    let job = super::playlist_resolution::get(token).ok_or("Preparation expired")?;
    let job = job.lock().unwrap_or_else(|p| p.into_inner());
    Ok(
        json!({"ok":true,"kind":"preparation","token":token,"total":job.selections.len(),
        "completed":job.completed,"ready":job.completed.saturating_sub(job.failures.len()),
        "issueCount":job.failures.len(),"finished":job.workers==0,"cancelled":job.cancelled,
        "canContinue":job.quality_token.is_some()}),
    )
}

/// Called only by the app's bounded worker; provider data never enters agent inspection.
pub(crate) fn command(command: &str, payload: &str, privacy: bool) -> String {
    let result = (|| -> Result<Value, &'static str> {
        if payload.len() > 131072 {
            return Err("Discovery request is too large");
        }
        let request: Request =
            serde_json::from_str(payload).map_err(|_| "Invalid discovery request")?;
        if request.picks.len() > 500 {
            return Err("Too many selections");
        }
        match command {
            "start" => {
                if request.source.len() > 16384 {
                    return Err("Too many links. Use a smaller batch.");
                }
                let client =
                    external::http::build_client().map_err(|_| "Could not start discovery")?;
                let sources = super::sources::extract_supported_urls(&request.source);
                if sources.len() == 1
                    && let super::sources::SourceUrl::YouTubePlaylist { playlist_id } =
                        super::sources::classify_url(&sources[0])
                {
                    let playlist = external::youtube::fetch_playlist_entries(&client, &playlist_id)
                        .map_err(|_| "Could not load this playlist. Try again.")?;
                    let token = discovery::store_playlist_session(&playlist);
                    return page(&token, 0, privacy);
                }
                let candidates = workflows::discovery::discover_videos(&client, &request.source)
                    .map_err(
                        |_| "Could not discover these links. Check the source and try again.",
                    )?;
                if candidates.is_empty() {
                    return Err("No downloadable media was found");
                }
                let token = discovery::store_discovery_session(candidates);
                page(&token, 0, privacy)
            }
            "page" => page(&request.token, request.offset, privacy),
            "bulk" => {
                let mut response = page(&request.token, request.offset, privacy)?;
                let picks =
                    bulk_selection(&request.token, request.all, &request.picks, &request.format)?;
                response["selection"] = json!(picks);
                Ok(response)
            }
            "prepare" => {
                if request.picks.len() > 500 {
                    return Err("Too many selections");
                }
                let token = workflows::discovery::prepare_playlist_picks(&request.picks).map_err(
                    |_| "Could not prepare selected items. Check selection and try again.",
                )?;
                preparation(&token)
            }
            "retry" => {
                let job =
                    super::playlist_resolution::get(&request.token).ok_or("Preparation expired")?;
                let selections = {
                    let job = job.lock().unwrap_or_else(|p| p.into_inner());
                    if job.workers != 0 {
                        return Err("Wait for preparation workers to stop before retrying");
                    }
                    job.selections.clone()
                };
                let token = workflows::playlist_resolution::start(selections)
                    .map_err(|_| "Could not retry preparation. Try again later.")?;
                preparation(&token)
            }
            "status" => preparation(&request.token),
            "cancel" => {
                let job =
                    super::playlist_resolution::get(&request.token).ok_or("Preparation expired")?;
                job.lock().unwrap_or_else(|p| p.into_inner()).cancelled = true;
                preparation(&request.token)
            }
            "formats" => {
                let job =
                    super::playlist_resolution::get(&request.token).ok_or("Preparation expired")?;
                let token = job
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .quality_token
                    .clone()
                    .ok_or("Formats are not ready")?;
                page(&token, 0, privacy)
            }
            "import" => {
                if request.picks.len() > 500 {
                    return Err("Too many selections");
                }
                let output = super::queue::QUEUE_OUTPUT_DIR
                    .get()
                    .ok_or("Download engine is starting")?;
                let client =
                    external::http::build_client().map_err(|_| "Could not start downloads")?;
                let outcome =
                    workflows::discovery::import_discovery_picks(&client, output, &request.picks);
                if outcome.selected == 0 {
                    return Err("Select at least one available format");
                }
                Ok(
                    json!({"ok":true,"kind":"imported","selected":outcome.selected,
                    "issueCount":outcome.errors.len(),
                    "detail":if outcome.errors.is_empty() {"Downloads added"} else {"Some items could not be added. Check the queue."}}),
                )
            }
            _ => Err("Unsupported discovery action"),
        }
    })();
    result
        .unwrap_or_else(|detail| json!({"ok":false,"detail":detail}))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::super::models::ResolvedVideo;
    use super::*;
    #[test]
    fn native_discovery_rows_hide_metadata_and_provider_addresses() {
        let resolved = ResolvedVideo {
            filename: "123-1.mp4".into(),
            media_url: "https://example.invalid/private-stream".into(),
            audio_url: None,
            extract_audio: false,
            quality_label: Some("private quality".into()),
            quality_height: Some(720),
        };
        let candidate = DiscoveryCandidate {
            resolved: resolved.clone(),
            qualities: vec![resolved],
            source_url: "https://example.invalid/private-source".into(),
            author: "Private author".into(),
            text: "Private title".into(),
            playlist: None,
        };
        let mut formats = candidate.clone();
        let mut low = formats.resolved.clone();
        low.quality_height = Some(360);
        let mut audio = formats.resolved.clone();
        audio.extract_audio = true;
        audio.quality_height = None;
        formats.qualities = vec![low, formats.resolved.clone(), audio];
        assert_eq!(chosen_quality(&formats, "audio"), 2);
        assert_eq!(chosen_quality(&formats, "720p"), 1);
        assert_eq!(chosen_quality(&formats, "480p"), 0);
        assert_eq!(chosen_quality(&formats, "original"), 0);
        let row = candidate_row(&candidate, "opaque", 0, true).to_string();
        for hidden in ["Private", "private", "123-1", "example.invalid"] {
            assert!(!row.contains(hidden));
        }
        assert!(row.contains("720p"));
        let entry = super::super::youtube::YouTubePlaylistEntry {
            video_id: "PrivateVideo".into(),
            title: "Private title".into(),
            author: "Private author".into(),
        };
        let playlist = playlist_row(&entry, "opaque", 499, true);
        assert_eq!(playlist["id"], "opaque:499");
        assert!(!playlist.to_string().contains("Private"));
        assert_eq!(
            playlist_row(&entry, "opaque", 499, false)["title"],
            "Private title"
        );

        assert!(
            candidate_row(&candidate, "opaque", 0, false)
                .to_string()
                .contains("Private author")
        );
        assert_eq!(
            serde_json::from_str::<Value>(&command("invalid", "{}", true)).unwrap()["ok"],
            false
        );
        assert_eq!(
            serde_json::from_str::<Value>(&command("start", "{\"unexpected\":true}", true))
                .unwrap()["ok"],
            false
        );
    }
}
