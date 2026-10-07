//! Typed anime catalog and watchlist commands, invoked only by app controls.
use super::super::{external, workflows};
use super::aniwaves::{CatalogItem, CatalogView, StreamEpisode};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashSet, VecDeque};
use std::sync::Mutex;

#[derive(Default)]
struct Handles {
    next: u64,
    items: VecDeque<(String, CatalogItem)>,
}
static HANDLES: Mutex<Handles> = Mutex::new(Handles {
    next: 0,
    items: VecDeque::new(),
});
#[derive(Default)]
struct EpisodeHandles {
    next: u64,
    items: VecDeque<(String, String, String)>,
}
static EPISODES: Mutex<EpisodeHandles> = Mutex::new(EpisodeHandles {
    next: 0,
    items: VecDeque::new(),
});
fn episode_row(episode: &StreamEpisode, id: &str, privacy: bool) -> Value {
    json!({"id":id,"title":if privacy {"Episode"} else {episode.title.as_str()},
        "number":if privacy {""} else {episode.number.as_str()},
        "releasedAt":if privacy {None} else {episode.released_at}})
}
fn episode_rows(episodes: &[StreamEpisode], watch_url: &str, privacy: bool) -> Vec<Value> {
    let mut handles = EPISODES.lock().unwrap_or_else(|p| p.into_inner());
    episodes
        .iter()
        .map(|episode| {
            handles.next = handles.next.wrapping_add(1);
            let id = format!("episode-{:x}", handles.next);
            handles
                .items
                .push_back((id.clone(), watch_url.to_owned(), episode.number.clone()));
            while handles.items.len() > 4096 {
                handles.items.pop_front();
            }
            episode_row(episode, &id, privacy)
        })
        .collect()
}
#[derive(Clone)]
struct CalendarItem {
    item: CatalogItem,
    day: usize,
    latest: String,
    release: Option<u64>,
    is_new: bool,
    unavailable: bool,
}
#[derive(Clone)]
struct CalendarSession {
    token: String,
    items: Vec<CalendarItem>,
    new_count: usize,
    first_visit: bool,
}
static CALENDARS: Mutex<VecDeque<CalendarSession>> = Mutex::new(VecDeque::new());
static CALENDAR_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
fn calendar_row(item: &CalendarItem, id: &str, privacy: bool) -> Value {
    let mut data = row(&item.item, id, true, privacy);
    data["day"] = json!(
        [
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
            "Sunday",
            "Schedule unavailable"
        ][item.day.min(7)]
    );
    data["latestEpisode"] = json!(if privacy { "" } else { item.latest.as_str() });
    data["displayRelease"] = json!(if privacy { None } else { item.release });
    data["isNew"] = json!(item.is_new);
    data["unavailable"] = json!(item.unavailable);
    data
}
fn calendar_page(session: &CalendarSession, page: u16, privacy: bool) -> Value {
    let pages = session.items.len().div_ceil(25).max(1);
    let page = usize::from(page.max(1)).min(pages);
    let selected: Vec<_> = session
        .items
        .iter()
        .skip((page - 1) * 25)
        .take(25)
        .collect();
    let mut registry = HANDLES.lock().unwrap_or_else(|p| p.into_inner());
    let items: Vec<_> = selected
        .into_iter()
        .map(|item| {
            registry.next = registry.next.wrapping_add(1);
            let id = format!("anime-{:x}", registry.next);
            registry.items.push_back((id.clone(), item.item.clone()));
            while registry.items.len() > 4096 {
                registry.items.pop_front();
            }
            calendar_row(item, &id, privacy)
        })
        .collect();
    json!({"ok":true,"kind":"calendar","page":page,"pages":pages,"selection":session.token,
        "newCount":session.new_count,"firstVisit":session.first_visit,"items":items})
}
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Request {
    section: String,
    query: String,
    page: u16,
    refresh: bool,
    id: String,
}
fn row(item: &CatalogItem, id: &str, watched: bool, privacy: bool) -> Value {
    json!({"id":id,"title":if privacy {"Anime item"} else {&item.title},
        "poster":if privacy {""} else {&item.poster_url},"watchlisted":watched,
        "sub":if privacy {None} else {item.sub.as_deref()},
        "dub":if privacy {None} else {item.dub.as_deref()},
        "total":if privacy {None} else {item.total.as_deref()},
        "mediaType":if privacy {None} else {item.media_type.as_deref()}})
}
fn rows(items: &[CatalogItem], watchlisted: &HashSet<String>, privacy: bool) -> Vec<Value> {
    let mut registry = HANDLES.lock().unwrap_or_else(|p| p.into_inner());
    items
        .iter()
        .map(|item| {
            registry.next = registry.next.wrapping_add(1);
            let id = format!("anime-{:x}", registry.next);
            registry.items.push_back((id.clone(), item.clone()));
            while registry.items.len() > 4096 {
                registry.items.pop_front();
            }
            row(item, &id, watchlisted.contains(&item.watch_url), privacy)
        })
        .collect()
}
fn item(id: &str) -> Option<CatalogItem> {
    HANDLES
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .items
        .iter()
        .find(|(handle, _)| handle == id)
        .map(|(_, item)| item.clone())
}
pub(crate) fn command(action: &str, payload: &str, privacy: bool) -> String {
    let result = (|| -> Result<Value, &'static str> {
        if payload.len() > 8192 {
            return Err("Anime request is too large");
        }
        let request: Request =
            serde_json::from_str(payload).map_err(|_| "Invalid anime request")?;
        if request.query.len() > 256 || request.section.len() > 64 || request.id.len() > 64 {
            return Err("Anime request is too large");
        }
        if !matches!(
            action,
            "catalog"
                | "watchlist"
                | "add"
                | "remove"
                | "watch"
                | "episodes"
                | "play"
                | "calendar"
                | "calendar-page"
        ) {
            return Err("Unsupported anime action");
        }
        let output = super::queue::QUEUE_OUTPUT_DIR
            .get()
            .ok_or("Download engine is starting")?;
        if action == "calendar-page" {
            let session = CALENDARS
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .iter()
                .find(|s| s.token == request.id)
                .cloned()
                .ok_or("Calendar view expired. Refresh the calendar.")?;
            return Ok(calendar_page(&session, request.page, privacy));
        }
        if action == "calendar" {
            let library =
                super::streaming_library::load(output).map_err(|_| "Could not load watchlist")?;
            let client =
                external::http::build_client().map_err(|_| "Could not load release calendar")?;
            let schedules = workflows::streaming::load_watchlist_schedules(
                &client,
                &library.entries,
                request.refresh,
            );
            let now = super::streaming_library::now_unix();
            let model = super::streaming_library::calendar_model(&library.entries, &schedules, now);
            let mut items = Vec::new();
            for day in (0..7)
                .map(|offset| (model.today + offset) % 7)
                .chain(std::iter::once(7))
            {
                for card in &model.groups[day] {
                    items.push(CalendarItem {
                        item: CatalogItem {
                            title: card.title.to_owned(),
                            watch_url: card.entry.watch_url.clone(),
                            poster_url: card.poster_url.unwrap_or("").to_owned(),
                            sub: card.entry.sub.clone(),
                            dub: card.entry.dub.clone(),
                            total: card.entry.total.clone(),
                            media_type: card.entry.media_type.clone(),
                        },
                        day,
                        latest: card.latest_episode.to_owned(),
                        release: card.display_release,
                        is_new: card.is_new,
                        unavailable: card.unavailable,
                    });
                }
            }
            super::streaming_library::mark_calendar_seen(output, &model.seen, now)
                .map_err(|_| "Could not save release calendar state")?;
            let session = CalendarSession {
                token: format!(
                    "calendar-{:x}",
                    CALENDAR_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ),
                items,
                new_count: model.new_count,
                first_visit: library.calendar_visited_at.is_none(),
            };
            let result = calendar_page(&session, 1, privacy);
            let mut cache = CALENDARS.lock().unwrap_or_else(|p| p.into_inner());
            cache.push_back(session);
            while cache.len() > 8 {
                cache.pop_front();
            }
            return Ok(result);
        }
        if action == "play" {
            let selected = EPISODES
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .items
                .iter()
                .find(|(id, _, _)| id == &request.id)
                .cloned()
                .ok_or("This episode selection expired. Refresh episodes.")?;
            let url = super::aniwaves::validate_watch_page_url(&selected.1)
                .map_err(|_| "Unsupported anime watch link")?;
            return Ok(json!({"ok":true,"watchUrl":url,"episode":selected.2}));
        }
        if action == "episodes" {
            let selected =
                item(&request.id).ok_or("This anime selection expired. Refresh the page.")?;
            let client = external::http::build_client().map_err(|_| "Could not load episodes")?;
            let schedule = external::aniwaves::load_stream_schedule(
                &client,
                &selected.watch_url,
                request.refresh,
            )
            .map_err(|_| "Episodes unavailable. Try again.")?;
            let pages = schedule.episodes.len().div_ceil(25).max(1);
            let page = usize::from(request.page.max(1)).min(pages);
            let episodes: Vec<_> = schedule
                .episodes
                .into_iter()
                .skip((page - 1) * 25)
                .take(25)
                .collect();
            return Ok(
                json!({"ok":true,"kind":"episodes","page":page,"pages":pages,"selection":request.id,
                "title":if privacy {"Anime episodes"} else {schedule.title.as_str()},
                "episodes":episode_rows(&episodes,&selected.watch_url,privacy)}),
            );
        }
        if action == "watch" {
            let item =
                item(&request.id).ok_or("This anime selection expired. Refresh the page.")?;
            let url = super::aniwaves::validate_watch_page_url(&item.watch_url)
                .map_err(|_| "Unsupported anime watch link")?;
            return Ok(json!({"ok":true,"watchUrl":url}));
        }
        if matches!(action, "add" | "remove") {
            let item =
                item(&request.id).ok_or("This anime selection expired. Refresh the page.")?;
            let present = if action == "remove" {
                super::streaming_library::remove(output, &item.watch_url)
                    .map_err(|_| "Could not update watchlist")?
            } else {
                let input = super::streaming_library::WatchlistInput::validated(
                    &item.title,
                    &item.watch_url,
                    (!item.poster_url.is_empty()).then_some(item.poster_url.as_str()),
                    item.sub.as_deref(),
                    item.dub.as_deref(),
                    item.total.as_deref(),
                    item.media_type.as_deref(),
                )
                .map_err(|_| "Could not save this anime selection")?;
                super::streaming_library::add(output, input)
                    .map_err(|_| "Could not update watchlist")?
            };
            return Ok(json!({"ok":true,"kind":"action","id":request.id,"watchlisted":present}));
        }
        let library =
            super::streaming_library::load(output).map_err(|_| "Could not load watchlist")?;
        let watchlisted = super::streaming_library::urls(&library.entries);
        let page = request.page.clamp(1, 1000);
        if action == "watchlist" {
            let entries: Vec<_> = library
                .entries
                .iter()
                .map(|entry| CatalogItem {
                    title: entry.title.clone(),
                    watch_url: entry.watch_url.clone(),
                    poster_url: entry.poster_url.clone().unwrap_or_default(),
                    sub: entry.sub.clone(),
                    dub: entry.dub.clone(),
                    total: entry.total.clone(),
                    media_type: entry.media_type.clone(),
                })
                .collect();
            let pages = entries.len().div_ceil(25).max(1);
            let page = usize::from(page).min(pages);
            let selected: Vec<_> = entries.into_iter().skip((page - 1) * 25).take(25).collect();
            return Ok(
                json!({"ok":true,"kind":"watchlist","page":page,"pages":pages,
                "items":rows(&selected,&watchlisted,privacy)}),
            );
        }
        let section = (!request.section.is_empty()).then_some(request.section.as_str());
        let view = CatalogView::from_params(section, Some(&request.query))
            .map_err(|_| "Invalid anime category or search")?;
        let client = external::http::build_client().map_err(|_| "Could not load anime catalog")?;
        let catalog = external::aniwaves::load_catalog(&client, view, page, request.refresh)
            .map_err(|error| catalog_failure(error.as_ref()))?;
        Ok(
            json!({"ok":true,"kind":"catalog","page":catalog.page,"pages":catalog.pages,
            "items":rows(&catalog.items,&watchlisted,privacy)}),
        )
    })();
    result
        .unwrap_or_else(|detail| json!({"ok":false,"detail":detail}))
        .to_string()
}
fn catalog_failure(error: &(dyn std::error::Error + 'static)) -> &'static str {
    if let Some(error) = error.downcast_ref::<reqwest::Error>() {
        catalog_failure_kind(
            error.is_timeout(),
            error.is_connect(),
            error.status().map(|status| status.as_u16()),
        )
    } else {
        "Anime provider response could not be read. Refresh to retry."
    }
}
fn catalog_failure_kind(timeout: bool, connection: bool, status: Option<u16>) -> &'static str {
    if timeout {
        "Anime provider timed out. Refresh to retry."
    } else if connection {
        "Could not connect to anime provider. Check connectivity and refresh."
    } else if matches!(status, Some(403 | 429)) {
        "Anime provider rejected this request. Wait and refresh."
    } else if status.is_some() {
        "Anime provider is unavailable. Refresh to retry."
    } else {
        "Anime provider request failed. Refresh to retry."
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_anime_catalog_failures_are_actionable_without_provider_data() {
        assert!(catalog_failure_kind(true, false, None).contains("timed out"));
        assert!(catalog_failure_kind(false, true, None).contains("connect"));
        assert!(catalog_failure_kind(false, false, Some(403)).contains("rejected"));
        assert!(catalog_failure_kind(false, false, Some(429)).contains("rejected"));
        assert!(catalog_failure_kind(false, false, Some(503)).contains("unavailable"));
        let error = std::io::Error::other("https://private.invalid/private-title");
        assert!(!catalog_failure(&error).contains("private"));
    }
    #[test]
    fn native_anime_rows_hide_metadata_and_reject_untrusted_commands() {
        let item = CatalogItem {
            title: "Synthetic private title".into(),
            watch_url: "https://aniwaves.ru/watch/synthetic".into(),
            poster_url: "https://static.aniwaves.ru/synthetic.jpg".into(),
            sub: Some("12".into()),
            dub: None,
            total: Some("24".into()),
            media_type: Some("TV".into()),
        };
        let episode = StreamEpisode {
            number: "7".into(),
            title: "Synthetic episode title".into(),
            released_at: Some(123),
        };
        let hidden_episode = episode_row(&episode, "opaque-episode", true);
        assert_eq!(hidden_episode["title"], "Episode");
        assert_eq!(hidden_episode["number"], "");
        assert!(hidden_episode["releasedAt"].is_null());
        assert!(!hidden_episode.to_string().contains("Synthetic"));
        assert_eq!(
            episode_row(&episode, "opaque-episode", false)["number"],
            "7"
        );
        let calendar = CalendarItem {
            item: item.clone(),
            day: 2,
            latest: "7".into(),
            release: Some(123),
            is_new: true,
            unavailable: false,
        };
        let hidden_calendar = calendar_row(&calendar, "opaque-calendar-item", true);
        assert_eq!(hidden_calendar["day"], "Wednesday");
        assert_eq!(hidden_calendar["latestEpisode"], "");
        assert!(hidden_calendar["displayRelease"].is_null());
        assert_eq!(hidden_calendar["isNew"], true);
        assert!(!hidden_calendar.to_string().contains("Synthetic"));
        let redacted = row(&item, "opaque-id", true, true);
        for hidden in ["Synthetic", "aniwaves.ru", "synthetic.jpg", "watch_url"] {
            assert!(!redacted.to_string().contains(hidden));
        }
        assert_eq!(redacted["poster"], "");
        assert!(redacted["sub"].is_null());
        assert_eq!(redacted["watchlisted"], true);
        assert_eq!(row(&item, "opaque-id", false, false)["title"], item.title);
        assert!(!command("unsupported", "{}", true).contains("watch_url"));
        assert!(
            command("catalog", "{\"url\":\"https://example.com\"}", true)
                .contains("Invalid anime request")
        );
    }
}
