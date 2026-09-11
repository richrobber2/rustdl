//! Local streaming watchlist, release-calendar state, and UI.

use super::super::local;
use super::super::local::aniwaves::StreamSchedule;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io::Read;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{array, fs, io};
use tiny_http::{Request, Response, StatusCode};

pub(in super::super) fn respond_watchlist_action(
    mut request: Request,
    state_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut body = String::new();
    request
        .as_reader()
        .take(32 * 1024)
        .read_to_string(&mut body)?;
    let form = Url::parse(&format!("http://localhost/?{body}"))?;
    let value = |key: &str| {
        form.query_pairs()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.into_owned())
    };
    let wants_json = value("response").as_deref() == Some("json");
    if value("token").as_deref() != Some(local::security::action_token()) {
        if wants_json {
            let response = Response::from_string(
                serde_json::json!({ "ok": false, "error": "Watchlist action expired. Reopen the episode and try again." }).to_string(),
            )
            .with_status_code(StatusCode(403))
            .with_header(local::html::header("Content-Type", "application/json; charset=utf-8"))
            .with_header(local::html::header("Cache-Control", "no-store"));
            request.respond(response)?;
            return Ok(());
        }
        return local::html::respond_text(request, 403, "Watchlist action expired");
    }
    let action = value("action").unwrap_or_default();
    let watch_url = value("url").unwrap_or_default();
    let result = match action.as_str() {
        "remove" => local::streaming_library::remove(state_dir, &watch_url),
        "add" => {
            let title = value("title").unwrap_or_default();
            let poster = value("poster");
            let sub = value("sub");
            let dub = value("dub");
            let total = value("total");
            let media_type = value("type");
            local::streaming_library::WatchlistInput::validated(
                &title,
                &watch_url,
                poster.as_deref(),
                sub.as_deref(),
                dub.as_deref(),
                total.as_deref(),
                media_type.as_deref(),
            )
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
            .and_then(|input| local::streaming_library::add(state_dir, input))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid watchlist action",
        )),
    };
    match result {
        Ok(present) if wants_json => {
            let response = Response::from_string(
                serde_json::json!({ "ok": true, "watchlisted": present }).to_string(),
            )
            .with_status_code(StatusCode(200))
            .with_header(local::html::header(
                "Content-Type",
                "application/json; charset=utf-8",
            ))
            .with_header(local::html::header("Cache-Control", "no-store"))
            .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
            request.respond(response)?;
            Ok(())
        }
        Ok(_) => {
            let return_value = value("return");
            let destination =
                local::streaming_library::safe_watchlist_return(return_value.as_deref());
            let response = Response::empty(StatusCode(303))
                .with_header(local::html::header("Location", &destination))
                .with_header(local::html::header("Cache-Control", "no-store"));
            request.respond(response)?;
            Ok(())
        }
        Err(error) if wants_json => {
            let response = Response::from_string(
                serde_json::json!({ "ok": false, "error": error.to_string() }).to_string(),
            )
            .with_status_code(StatusCode(422))
            .with_header(local::html::header(
                "Content-Type",
                "application/json; charset=utf-8",
            ))
            .with_header(local::html::header("Cache-Control", "no-store"))
            .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
            request.respond(response)?;
            Ok(())
        }
        Err(error) => {
            local::html::respond_text(request, 422, &format!("Watchlist update failed: {error}"))
        }
    }
}

pub(in super::super) fn safe_watchlist_return(value: Option<&str>) -> String {
    value
        .filter(|value| {
            *value == "/streaming"
                || value.starts_with("/streaming?")
                || *value == "/streaming/watchlist"
                || *value == "/streaming/calendar"
        })
        .unwrap_or("/streaming/watchlist")
        .to_owned()
}

pub(in super::super) fn streaming_page_response(
    request: Request,
    html: String,
) -> Result<(), Box<dyn Error>> {
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

pub(in super::super) fn respond_streaming_watchlist_page(
    request: Request,
    state_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    let library = local::streaming_library::load(state_dir)?;
    local::streaming_library::streaming_page_response(
        request,
        local::streaming_library::render_watchlist(
            &library.entries,
            local::security::action_token(),
        ),
    )
}

pub(in super::super) const STATE_FILE: &str = ".streaming-library.json";

pub(in super::super) const STATE_PART_FILE: &str = ".streaming-library.json.part";

pub(in super::super) const MAX_STATE_BYTES: u64 = 512 * 1024;

pub(in super::super) const MAX_WATCHLIST_ENTRIES: usize = 200;

pub(in super::super) static STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WatchlistEntry {
    pub title: String,
    pub watch_url: String,
    pub poster_url: Option<String>,
    pub sub: Option<String>,
    pub dub: Option<String>,
    pub total: Option<String>,
    pub media_type: Option<String>,
    pub added_at: u64,
    #[serde(default)]
    pub last_seen_episode: Option<String>,
    #[serde(default)]
    pub last_seen_release: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct StreamingState {
    #[serde(default)]
    pub(in super::super) entries: Vec<WatchlistEntry>,
    #[serde(default)]
    pub(in super::super) calendar_visited_at: Option<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct StreamingLibrary {
    pub entries: Vec<WatchlistEntry>,
    pub calendar_visited_at: Option<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct WatchlistInput {
    pub title: String,
    pub watch_url: String,
    pub poster_url: Option<String>,
    pub sub: Option<String>,
    pub dub: Option<String>,
    pub total: Option<String>,
    pub media_type: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CalendarSeen {
    pub watch_url: String,
    pub episode: String,
    pub released_at: Option<u64>,
}

impl WatchlistInput {
    pub(crate) fn validated(
        title: &str,
        watch_url: &str,
        poster_url: Option<&str>,
        sub: Option<&str>,
        dub: Option<&str>,
        total: Option<&str>,
        media_type: Option<&str>,
    ) -> Result<Self, &'static str> {
        let title =
            local::streaming_library::visible_value(title, 300).ok_or("invalid streaming title")?;
        let watch_url = local::aniwaves::validate_watch_page_url(watch_url)
            .map_err(|_| "unsupported AniWaves watch URL")?;
        let poster_url = poster_url
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                local::aniwaves::validated_poster_url(value)
                    .ok_or("unsupported AniWaves poster URL")
            })
            .transpose()?;
        Ok(Self {
            title,
            watch_url,
            poster_url,
            sub: local::streaming_library::short_value(sub, 8),
            dub: local::streaming_library::short_value(dub, 8),
            total: local::streaming_library::short_value(total, 8),
            media_type: local::streaming_library::short_value(media_type, 24),
        })
    }
}

pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

pub(crate) fn load(state_dir: &Path) -> io::Result<StreamingLibrary> {
    let _guard = local::streaming_library::state_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let state = local::streaming_library::read_state(state_dir)?;
    Ok(StreamingLibrary {
        entries: state.entries,
        calendar_visited_at: state.calendar_visited_at,
    })
}

pub(crate) fn add(state_dir: &Path, input: WatchlistInput) -> io::Result<bool> {
    let _guard = local::streaming_library::state_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut state = local::streaming_library::read_state(state_dir)?;
    if let Some(entry) = state
        .entries
        .iter_mut()
        .find(|entry| entry.watch_url == input.watch_url)
    {
        entry.title = input.title;
        entry.poster_url = input.poster_url.or_else(|| entry.poster_url.clone());
        entry.sub = input.sub;
        entry.dub = input.dub;
        entry.total = input.total;
        entry.media_type = input.media_type;
    } else {
        if state.entries.len() >= MAX_WATCHLIST_ENTRIES {
            return Err(io::Error::other("the streaming watchlist is full"));
        }
        state.entries.push(WatchlistEntry {
            title: input.title,
            watch_url: input.watch_url,
            poster_url: input.poster_url,
            sub: input.sub,
            dub: input.dub,
            total: input.total,
            media_type: input.media_type,
            added_at: local::streaming_library::now_unix(),
            last_seen_episode: None,
            last_seen_release: None,
        });
    }
    state
        .entries
        .sort_by_key(|entry| std::cmp::Reverse(entry.added_at));
    local::streaming_library::persist_state(state_dir, &state)?;
    Ok(true)
}

pub(crate) fn remove(state_dir: &Path, watch_url: &str) -> io::Result<bool> {
    let watch_url = local::aniwaves::validate_watch_page_url(watch_url)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "unsupported watch URL"))?;
    let _guard = local::streaming_library::state_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut state = local::streaming_library::read_state(state_dir)?;
    let before = state.entries.len();
    state.entries.retain(|entry| entry.watch_url != watch_url);
    let present = state.entries.len() == before;
    if !present {
        local::streaming_library::persist_state(state_dir, &state)?;
    }
    Ok(false)
}

pub(crate) fn mark_calendar_seen(
    state_dir: &Path,
    seen: &[CalendarSeen],
    visited_at: u64,
) -> io::Result<()> {
    let _guard = local::streaming_library::state_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut state = local::streaming_library::read_state(state_dir)?;
    let seen = seen
        .iter()
        .map(|item| (item.watch_url.as_str(), item))
        .collect::<HashMap<_, _>>();
    for entry in &mut state.entries {
        if let Some(item) = seen.get(entry.watch_url.as_str()) {
            entry.last_seen_episode = Some(item.episode.clone());
            entry.last_seen_release = item.released_at;
        }
    }
    state.calendar_visited_at = Some(visited_at);
    local::streaming_library::persist_state(state_dir, &state)
}

pub(crate) fn urls(entries: &[WatchlistEntry]) -> HashSet<String> {
    entries
        .iter()
        .map(|entry| entry.watch_url.clone())
        .collect()
}

pub(in super::super) fn state_lock() -> &'static Mutex<()> {
    STATE_LOCK.get_or_init(|| Mutex::new(()))
}

pub(in super::super) fn read_state(state_dir: &Path) -> io::Result<StreamingState> {
    let path = state_dir.join(STATE_FILE);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(StreamingState::default());
        }
        Err(error) => return Err(error),
    };
    if bytes.len() as u64 > MAX_STATE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "streaming library state is unexpectedly large",
        ));
    }
    let mut state = serde_json::from_slice::<StreamingState>(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    state.entries.retain(valid_stored_entry);
    state.entries.truncate(MAX_WATCHLIST_ENTRIES);
    Ok(state)
}

pub(in super::super) fn persist_state(state_dir: &Path, state: &StreamingState) -> io::Result<()> {
    fs::create_dir_all(state_dir)?;
    let bytes = serde_json::to_vec(state).map_err(io::Error::other)?;
    if bytes.len() as u64 > MAX_STATE_BYTES {
        return Err(io::Error::other(
            "streaming library state is unexpectedly large",
        ));
    }
    let temporary = state_dir.join(STATE_PART_FILE);
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, state_dir.join(STATE_FILE))
}

pub(in super::super) fn valid_stored_entry(entry: &WatchlistEntry) -> bool {
    local::streaming_library::visible_value(&entry.title, 300).is_some()
        && local::aniwaves::validate_watch_page_url(&entry.watch_url).is_ok()
        && entry
            .poster_url
            .as_deref()
            .is_none_or(|value| local::aniwaves::validated_poster_url(value).is_some())
}

pub(in super::super) fn visible_value(value: &str, maximum: usize) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value.chars().count() <= maximum && !value.chars().any(char::is_control))
        .then(|| value.to_owned())
}

pub(in super::super) fn short_value(value: Option<&str>, maximum: usize) -> Option<String> {
    value.and_then(|value| local::streaming_library::visible_value(value, maximum))
}

pub(in super::super) fn launch_url(watch_url: &str) -> String {
    let mut url = Url::parse("rustdl://stream").expect("static streaming URL");
    url.query_pairs_mut().append_pair("url", watch_url);
    url.to_string()
}

pub(in super::super) fn poster(entry: &WatchlistEntry) -> String {
    entry.poster_url.as_ref().map_or_else(
        || r#"<div class="library-poster fallback" aria-hidden="true">R</div>"#.to_owned(),
        |poster| {
            format!(
                r#"<div class="library-poster"><img src="{}" alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer"></div>"#,
                local::html::escape_html(poster)
            )
        },
    )
}

pub(in super::super) fn badges(entry: &WatchlistEntry) -> String {
    [
        entry.sub.as_ref().map(|value| format!("SUB {value}")),
        entry.dub.as_ref().map(|value| format!("DUB {value}")),
        entry.total.as_ref().map(|value| format!("{value} EPS")),
    ]
    .into_iter()
    .flatten()
    .map(|value| format!("<span>{}</span>", local::html::escape_html(&value)))
    .collect()
}

pub(in super::super) const LIBRARY_CSS: &str =
    include_str!("../../assets/css/streaming-library.css");

pub(crate) fn render_watchlist(entries: &[WatchlistEntry], token: &str) -> String {
    let cards = if entries.is_empty() {
        r#"<p class="empty"><strong>Your watchlist is empty.</strong><a href="/streaming">Browse the catalog</a> and tap ＋ on any show.</p>"#.to_owned()
    } else {
        entries
            .iter()
            .map(|entry| {
                let watch_href = &(local::html::escape_html(
                    &local::streaming_library::launch_url(&entry.watch_url),
                ));
                let poster_html = &(local::streaming_library::poster(entry));
                let display_title = &(local::html::escape_html(&entry.title));
                let badges_html = &(local::streaming_library::badges(entry));
                let media_type_label =
                    &(local::html::escape_html(entry.media_type.as_deref().unwrap_or("Series")));
                let action_token_value = &(local::html::escape_html(token));
                let watch_url = &(local::html::escape_html(&entry.watch_url));
                let remove_title = &(local::html::escape_html(&entry.title));
                format!(
                    include_str!("../../assets/html/watchlist-card.html"),
                    watch_href = watch_href,
                    poster_html = poster_html,
                    display_title = display_title,
                    badges_html = badges_html,
                    media_type_label = media_type_label,
                    action_token_value = action_token_value,
                    watch_url = watch_url,
                    remove_title = remove_title
                )
            })
            .collect()
    };
    {
        let entry_count = &(entries.len());
        format!(
            include_str!("../../assets/html/watchlist.html"),
            LIBRARY_CSS = LIBRARY_CSS,
            entry_count = entry_count,
            page_script = include_str!("../../assets/js/watchlist.js"),
            cards = cards
        )
    }
}

pub(in super::super) struct CalendarCard<'a> {
    pub(in super::super) entry: &'a WatchlistEntry,
    pub(in super::super) title: &'a str,
    pub(in super::super) poster_url: Option<&'a str>,
    pub(in super::super) latest_episode: &'a str,
    pub(in super::super) display_release: Option<u64>,
    pub(in super::super) is_new: bool,
    pub(in super::super) unavailable: bool,
}

pub(crate) fn render_calendar(
    entries: &[WatchlistEntry],
    schedules: &HashMap<String, Result<StreamSchedule, String>>,
    now: u64,
    previous_visit: Option<u64>,
) -> (String, Vec<CalendarSeen>) {
    let mut groups: [Vec<CalendarCard<'_>>; 8] = array::from_fn(|_| Vec::new());
    let mut seen = Vec::new();
    let mut new_count = 0usize;
    let today = local::streaming_library::weekday(now);
    for entry in entries {
        match schedules.get(&entry.watch_url) {
            Some(Ok(schedule)) => {
                let Some(latest) = schedule.episodes.last() else {
                    continue;
                };
                let latest_dated = schedule
                    .episodes
                    .iter()
                    .filter_map(|episode| episode.released_at.map(|released| (episode, released)))
                    .filter(|(_, released)| *released <= now)
                    .max_by_key(|(_, released)| *released);
                let next_dated = schedule
                    .episodes
                    .iter()
                    .filter_map(|episode| episode.released_at.map(|released| (episode, released)))
                    .filter(|(_, released)| *released > now && *released <= now + 14 * 86_400)
                    .min_by_key(|(_, released)| *released);
                let display_release = next_dated.or(latest_dated).map(|(_, released)| released);
                let group = display_release.map_or(7, weekday);
                let is_new = entry.last_seen_episode.as_ref().is_some_and(|episode| {
                    episode != &latest.number
                        || entry
                            .last_seen_release
                            .zip(latest.released_at)
                            .is_some_and(|(seen, current)| current > seen)
                });
                new_count += usize::from(is_new);
                seen.push(CalendarSeen {
                    watch_url: entry.watch_url.clone(),
                    episode: latest.number.clone(),
                    released_at: latest.released_at.or(latest_dated.map(|(_, value)| value)),
                });
                groups[group].push(CalendarCard {
                    entry,
                    title: &schedule.title,
                    poster_url: schedule
                        .poster_url
                        .as_deref()
                        .or(entry.poster_url.as_deref()),
                    latest_episode: &latest.number,
                    display_release,
                    is_new,
                    unavailable: false,
                });
            }
            _ => groups[7].push(CalendarCard {
                entry,
                title: &entry.title,
                poster_url: entry.poster_url.as_deref(),
                latest_episode: entry.last_seen_episode.as_deref().unwrap_or("—"),
                display_release: entry.last_seen_release,
                is_new: false,
                unavailable: true,
            }),
        }
    }
    for group in &mut groups {
        group.sort_by_key(|card| card.title.to_lowercase());
    }

    let names = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    let order = (0..7)
        .map(|offset| (today + offset) % 7)
        .collect::<Vec<_>>();
    let day_nav = order
        .iter()
        .filter(|index| !groups[**index].is_empty())
        .map(|index| format!(r##"<a href="#day-{index}">{}</a>"##, names[*index]))
        .collect::<String>();
    let mut sections = order
        .iter()
        .filter(|index| !groups[**index].is_empty())
        .map(|index| {
            let cards = groups[*index]
                .iter()
                .map(local::streaming_library::render_calendar_card)
                .collect::<String>();
            format!(
                r#"<section class="day" id="day-{index}"><h2>{}<span>{} show{}</span></h2><div class="calendar-grid">{cards}</div></section>"#,
                names[*index],
                groups[*index].len(),
                if groups[*index].len() == 1 { "" } else { "s" },
            )
        })
        .collect::<String>();
    if !groups[7].is_empty() {
        sections.push_str(&format!(
            r#"<section class="day" id="day-unscheduled"><h2>Schedule unavailable<span>{} show{}</span></h2><div class="calendar-grid">{}</div></section>"#,
            groups[7].len(),
            if groups[7].len() == 1 { "" } else { "s" },
            groups[7].iter().map(local::streaming_library::render_calendar_card).collect::<String>(),
        ));
    }
    if entries.is_empty() {
        sections = r#"<p class="empty"><strong>No saved shows to schedule.</strong><a href="/streaming">Add shows from the catalog</a> first.</p>"#.to_owned();
    }
    let visit = previous_visit.map_or_else(
        || "This is your first calendar check.".to_owned(),
        |_| "New badges compare against your previous calendar check.".to_owned(),
    );
    let html = {
        let new_status_suffix = &(if new_count == 1 { " is" } else { "s are" });
        format!(
            include_str!("../../assets/html/calendar.html"),
            LIBRARY_CSS = LIBRARY_CSS,
            new_status_suffix = new_status_suffix,
            page_script = include_str!("../../assets/js/calendar.js"),
            day_nav = day_nav,
            new_count = new_count,
            sections = sections,
            visit = visit
        )
    };
    (html, seen)
}

pub(in super::super) fn render_calendar_card(card: &CalendarCard<'_>) -> String {
    let poster = card.poster_url.map_or_else(
        || r#"<div class="library-poster fallback" aria-hidden="true">R</div>"#.to_owned(),
        |poster| format!(r#"<div class="library-poster"><img src="{}" alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer"></div>"#, local::html::escape_html(poster)),
    );
    let release = card.display_release.map_or_else(
        || "Release time unavailable".to_owned(),
        |released| format!(r#"<time data-unix="{released}">Scheduled release</time>"#),
    );
    format!(
        r#"<a class="calendar-card" href="{}">{poster}<div class="calendar-copy"><strong>{}</strong><span>{} EP {}</span>{release}</div>{}</a>"#,
        local::html::escape_html(&local::streaming_library::launch_url(&card.entry.watch_url)),
        local::html::escape_html(card.title),
        if card.unavailable {
            "Last seen"
        } else {
            "Latest"
        },
        local::html::escape_html(card.latest_episode),
        if card.is_new {
            r#"<b class="new-badge">NEW</b>"#
        } else {
            ""
        },
    )
}

pub(in super::super) fn weekday(timestamp: u64) -> usize {
    ((timestamp / 86_400 + 3) % 7) as usize
}

#[cfg(test)]
mod tests {
    use super::super::aniwaves::StreamEpisode;
    use super::*;

    fn input(url: &str) -> WatchlistInput {
        WatchlistInput::validated(
            "Test Show",
            url,
            Some("https://static.aniwaves.ru/resources/thumbnails/test.jpg"),
            Some("9"),
            None,
            Some("12"),
            Some("TV"),
        )
        .unwrap()
    }

    fn test_directory() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rustdl-streaming-library-{}-{}",
            std::process::id(),
            unique
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn native_watchlist_post_saves_without_poster_and_reports_errors() {
        let directory = test_directory();
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/__app/watchlist", server.server_addr());
        let server_directory = directory.clone();
        let worker = std::thread::spawn(move || {
            let client = super::super::super::external::http::build_client().unwrap();
            for _ in 0..4 {
                let request = server
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap()
                    .unwrap();
                super::super::super::workflows::server::handle_request(
                    request,
                    &client,
                    &server_directory,
                )
                .unwrap();
            }
        });
        let client = super::super::super::external::http::build_client().unwrap();
        let url = "https://aniwaves.ru/watch/test-show-123";
        let title = "Hero's Quest & Friends + 日本語";
        let post = |action, poster, token| {
            let mut form = Url::parse("http://localhost/").unwrap();
            form.query_pairs_mut().extend_pairs([
                ("token", token),
                ("action", action),
                ("url", url),
                ("title", title),
                ("poster", poster),
                ("response", "json"),
            ]);
            client
                .post(&endpoint)
                .header(
                    "Content-Type",
                    "application/x-www-form-urlencoded; charset=utf-8",
                )
                .body(form.query().unwrap().to_owned())
                .send()
                .unwrap()
        };
        let token = local::security::action_token();
        let response = post("add", "", token);
        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(
            response.json::<serde_json::Value>().unwrap()["watchlisted"],
            true
        );
        let library = load(&directory).unwrap();
        assert_eq!(library.entries.len(), 1);
        assert_eq!(library.entries[0].title, title);
        assert!(library.entries[0].poster_url.is_none());
        let response = post("add", "null", token);
        assert_eq!(response.status().as_u16(), 422);
        assert!(
            response.json::<serde_json::Value>().unwrap()["error"]
                .as_str()
                .unwrap()
                .contains("poster")
        );
        let response = post("remove", "", "expired-token");
        assert_eq!(response.status().as_u16(), 403);
        assert!(
            response.json::<serde_json::Value>().unwrap()["error"]
                .as_str()
                .unwrap()
                .contains("Reopen")
        );
        assert_eq!(load(&directory).unwrap().entries.len(), 1);
        let response = post("remove", "", token);
        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(
            response.json::<serde_json::Value>().unwrap()["watchlisted"],
            false
        );
        assert!(load(&directory).unwrap().entries.is_empty());
        worker.join().unwrap();
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn persists_deduplicates_and_removes_watchlist_entries() {
        let directory = test_directory();
        let url = "https://aniwaves.ru/watch/test-show-123";
        assert!(local::streaming_library::add(&directory, input(url)).unwrap());
        assert!(local::streaming_library::add(&directory, input(url)).unwrap());
        let library = local::streaming_library::load(&directory).unwrap();
        assert_eq!(library.entries.len(), 1);
        let html = local::streaming_library::render_watchlist(&library.entries, "test-token");
        assert!(html.contains("form.getAttribute('action')"));
        assert!(!html.contains("fetch(form.action"));
        assert!(!local::streaming_library::remove(&directory, url).unwrap());
        assert!(
            local::streaming_library::load(&directory)
                .unwrap()
                .entries
                .is_empty()
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn calendar_marks_only_changes_after_a_successful_seen_state() {
        let directory = test_directory();
        let url = "https://aniwaves.ru/watch/test-show-123";
        local::streaming_library::add(&directory, input(url)).unwrap();
        let first = CalendarSeen {
            watch_url: url.to_owned(),
            episode: "8".to_owned(),
            released_at: Some(1_787_414_400),
        };
        local::streaming_library::mark_calendar_seen(
            &directory,
            std::slice::from_ref(&first),
            1_787_414_400,
        )
        .unwrap();
        let library = local::streaming_library::load(&directory).unwrap();
        let schedule = StreamSchedule {
            title: "Test Show".to_owned(),
            poster_url: None,
            episodes: vec![StreamEpisode {
                number: "9".to_owned(),
                title: "New episode".to_owned(),
                released_at: Some(1_788_019_200),
            }],
            show_id: "123".to_owned(),
        };
        let schedules = HashMap::from([(url.to_owned(), Ok(schedule))]);
        let (html, seen) = local::streaming_library::render_calendar(
            &library.entries,
            &schedules,
            1_788_019_300,
            library.calendar_visited_at,
        );
        assert!(html.contains("new-badge\">NEW"));
        assert_eq!(seen[0].episode, "9");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejects_cross_origin_or_oversized_watchlist_metadata() {
        assert!(
            WatchlistInput::validated(
                "Show",
                "https://evil.test/watch/1",
                None,
                None,
                None,
                None,
                None
            )
            .is_err()
        );
        assert!(
            WatchlistInput::validated(
                &"x".repeat(301),
                "https://aniwaves.ru/watch/show-1",
                None,
                None,
                None,
                None,
                None
            )
            .is_err()
        );
        assert_eq!(local::streaming_library::weekday(0), 3);
    }
}
