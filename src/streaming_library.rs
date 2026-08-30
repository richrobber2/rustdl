use super::{
    aniwaves::{self, StreamSchedule},
    escape_html,
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::array;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const STATE_FILE: &str = ".streaming-library.json";
const STATE_PART_FILE: &str = ".streaming-library.json.part";
const MAX_STATE_BYTES: u64 = 512 * 1024;
const MAX_WATCHLIST_ENTRIES: usize = 200;
static STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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
struct StreamingState {
    #[serde(default)]
    entries: Vec<WatchlistEntry>,
    #[serde(default)]
    calendar_visited_at: Option<u64>,
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
        let title = visible_value(title, 300).ok_or("invalid streaming title")?;
        let watch_url = aniwaves::validate_watch_page_url(watch_url)
            .map_err(|_| "unsupported AniWaves watch URL")?;
        let poster_url = poster_url
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                aniwaves::validated_poster_url(value).ok_or("unsupported AniWaves poster URL")
            })
            .transpose()?;
        Ok(Self {
            title,
            watch_url,
            poster_url,
            sub: short_value(sub, 8),
            dub: short_value(dub, 8),
            total: short_value(total, 8),
            media_type: short_value(media_type, 24),
        })
    }
}

pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

pub(crate) fn load(state_dir: &Path) -> io::Result<StreamingLibrary> {
    let _guard = state_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let state = read_state(state_dir)?;
    Ok(StreamingLibrary {
        entries: state.entries,
        calendar_visited_at: state.calendar_visited_at,
    })
}

pub(crate) fn add(state_dir: &Path, input: WatchlistInput) -> io::Result<bool> {
    let _guard = state_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut state = read_state(state_dir)?;
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
            added_at: now_unix(),
            last_seen_episode: None,
            last_seen_release: None,
        });
    }
    state
        .entries
        .sort_by_key(|entry| std::cmp::Reverse(entry.added_at));
    persist_state(state_dir, &state)?;
    Ok(true)
}

pub(crate) fn remove(state_dir: &Path, watch_url: &str) -> io::Result<bool> {
    let watch_url = aniwaves::validate_watch_page_url(watch_url)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "unsupported watch URL"))?;
    let _guard = state_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut state = read_state(state_dir)?;
    let before = state.entries.len();
    state.entries.retain(|entry| entry.watch_url != watch_url);
    let present = state.entries.len() == before;
    if !present {
        persist_state(state_dir, &state)?;
    }
    Ok(false)
}

pub(crate) fn mark_calendar_seen(
    state_dir: &Path,
    seen: &[CalendarSeen],
    visited_at: u64,
) -> io::Result<()> {
    let _guard = state_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut state = read_state(state_dir)?;
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
    persist_state(state_dir, &state)
}

pub(crate) fn urls(entries: &[WatchlistEntry]) -> HashSet<String> {
    entries
        .iter()
        .map(|entry| entry.watch_url.clone())
        .collect()
}

fn state_lock() -> &'static Mutex<()> {
    STATE_LOCK.get_or_init(|| Mutex::new(()))
}

fn read_state(state_dir: &Path) -> io::Result<StreamingState> {
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

fn persist_state(state_dir: &Path, state: &StreamingState) -> io::Result<()> {
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

fn valid_stored_entry(entry: &WatchlistEntry) -> bool {
    visible_value(&entry.title, 300).is_some()
        && aniwaves::validate_watch_page_url(&entry.watch_url).is_ok()
        && entry
            .poster_url
            .as_deref()
            .is_none_or(|value| aniwaves::validated_poster_url(value).is_some())
}

fn visible_value(value: &str, maximum: usize) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value.chars().count() <= maximum && !value.chars().any(char::is_control))
        .then(|| value.to_owned())
}

fn short_value(value: Option<&str>, maximum: usize) -> Option<String> {
    value.and_then(|value| visible_value(value, maximum))
}

fn launch_url(watch_url: &str) -> String {
    let mut url = Url::parse("rustdl://stream").expect("static streaming URL");
    url.query_pairs_mut().append_pair("url", watch_url);
    url.to_string()
}

fn poster(entry: &WatchlistEntry) -> String {
    entry.poster_url.as_ref().map_or_else(
        || r#"<div class="library-poster fallback" aria-hidden="true">R</div>"#.to_owned(),
        |poster| {
            format!(
                r#"<div class="library-poster"><img src="{}" alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer"></div>"#,
                escape_html(poster)
            )
        },
    )
}

fn badges(entry: &WatchlistEntry) -> String {
    [
        entry.sub.as_ref().map(|value| format!("SUB {value}")),
        entry.dub.as_ref().map(|value| format!("DUB {value}")),
        entry.total.as_ref().map(|value| format!("{value} EPS")),
    ]
    .into_iter()
    .flatten()
    .map(|value| format!("<span>{}</span>", escape_html(&value)))
    .collect()
}

const LIBRARY_CSS: &str = r#"
@view-transition{navigation:auto}:root{color-scheme:dark;font-family:Inter,ui-sans-serif,system-ui,sans-serif}*{box-sizing:border-box}body{min-height:100vh;margin:0;padding:clamp(.8rem,3vw,2rem);color:var(--rustdl-text,#f7f7f8);background:var(--rustdl-page,#090a0f)}main{width:min(100%,1120px);margin:auto}.library-top{position:sticky;z-index:20;top:0;display:flex;align-items:center;gap:.55rem;padding:.65rem;border:1px solid var(--rustdl-surface-border,#ffffff1c);border-radius:17px;background:var(--rustdl-surface,#0d0f16e8);box-shadow:0 14px 40px #0007;backdrop-filter:blur(18px)}.library-top a,.library-actions a,.remove-watch{display:inline-grid;place-items:center;min-height:2.65rem;padding:.65rem .8rem;border:1px solid var(--rustdl-surface-border,#ffffff20);border-radius:11px;color:var(--rustdl-control-text,#dfe5ef);background:var(--rustdl-control,#181b24);text-decoration:none;font:800 .75rem/1 system-ui}.library-top a:first-child{margin-right:auto}.library-top .active{color:var(--rustdl-accent-ink,#07110f);border-color:var(--rustdl-accent,#70dfc9);background:var(--rustdl-accent,#70dfc9)}header{display:flex;align-items:end;justify-content:space-between;gap:1rem;margin:1.4rem .2rem}header span{color:var(--rustdl-accent,#70dfc9);font-size:.7rem;font-weight:850;letter-spacing:.1em;text-transform:uppercase}h1{margin:.35rem 0;font-size:clamp(2.15rem,7vw,4.2rem);letter-spacing:-.055em}p{margin:0;color:var(--rustdl-muted,#9ca3b3);line-height:1.45}.library-actions{display:flex;gap:.5rem}.watch-grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:.8rem}.watch-card{position:relative;display:grid;grid-template-columns:7.4rem minmax(0,1fr);min-height:10.4rem;overflow:hidden;border:1px solid var(--rustdl-surface-border,#ffffff16);border-radius:18px;background:var(--rustdl-surface,#10121a);transition:transform .18s ease,border-color .18s ease}.watch-card:hover{transform:translateY(-2px);border-color:var(--rustdl-accent-border,#70dfc966)}.watch-open{display:contents;color:inherit;text-decoration:none}.library-poster{min-height:100%;overflow:hidden;background:linear-gradient(145deg,#1b2140,#0b1514)}.library-poster img{width:100%;height:100%;object-fit:cover}.library-poster.fallback{display:grid;place-items:center;color:#70dfc9;font-size:2.5rem;font-weight:950}.watch-copy{min-width:0;display:flex;flex-direction:column;gap:.5rem;padding:.85rem .8rem}.watch-copy strong{display:-webkit-box;overflow:hidden;padding-right:2.2rem;-webkit-box-orient:vertical;-webkit-line-clamp:3;font-size:.86rem;line-height:1.3}.watch-copy small{margin-top:auto;color:var(--rustdl-muted,#8f98aa);font-size:.68rem}.badges{display:flex;flex-wrap:wrap;gap:.3rem}.badges span,.new-badge{padding:.26rem .38rem;border-radius:6px;color:var(--rustdl-accent,#9ce9db);background:var(--rustdl-accent-soft,#70dfc914);font-size:.58rem;font-weight:900}.remove-form{position:absolute;z-index:3;right:.55rem;top:.55rem}.remove-watch{min-width:2.25rem;min-height:2.25rem;padding:.45rem;color:var(--rustdl-control-text,#dfe5ef);background:var(--rustdl-surface,#11131b);cursor:pointer}.empty{grid-column:1/-1;padding:2.2rem;border:1px dashed var(--rustdl-surface-border,#ffffff24);border-radius:18px;text-align:center}.empty strong{display:block;margin-bottom:.45rem}.empty a{color:var(--rustdl-accent,#70dfc9)}.day-nav{display:flex;gap:.45rem;overflow-x:auto;margin:0 0 1rem;padding:.15rem;scrollbar-width:none}.day-nav a{flex:none;padding:.58rem .72rem;border:1px solid var(--rustdl-surface-border,#ffffff1f);border-radius:999px;color:var(--rustdl-control-text,#dfe5ef);background:var(--rustdl-control,#181b24);text-decoration:none;font-size:.68rem;font-weight:850}.day{scroll-margin-top:5rem;margin:0 0 1rem;padding:1rem;border:1px solid var(--rustdl-surface-border,#ffffff16);border-radius:20px;background:color-mix(in srgb,var(--rustdl-surface,#10121a) 82%,transparent)}.day h2{display:flex;align-items:center;justify-content:space-between;gap:.7rem;margin:0 0 .8rem;font-size:1rem}.day h2 span{color:var(--rustdl-muted,#8f98aa);font-size:.68rem}.calendar-grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:.7rem}.calendar-card{position:relative;display:grid;grid-template-columns:5rem minmax(0,1fr);min-height:7.2rem;overflow:hidden;border:1px solid var(--rustdl-surface-border,#ffffff14);border-radius:14px;color:inherit;background:var(--rustdl-control,#0c0e15);text-decoration:none}.calendar-card .library-poster{aspect-ratio:5/7}.calendar-copy{min-width:0;display:flex;flex-direction:column;gap:.35rem;padding:.68rem}.calendar-copy strong{display:-webkit-box;overflow:hidden;padding-right:2.8rem;-webkit-box-orient:vertical;-webkit-line-clamp:2;font-size:.78rem;line-height:1.3}.calendar-copy span,.calendar-copy time{color:var(--rustdl-muted,#8f98aa);font-size:.65rem}.new-badge{position:absolute;right:.5rem;top:.5rem;color:var(--rustdl-accent-ink,#07110f);background:var(--rustdl-accent,#70dfc9)}.notice{margin:0 0 1rem;padding:.8rem 1rem;border:1px solid var(--rustdl-accent-border,#70dfc944);border-radius:13px;color:var(--rustdl-muted,#aeb5c4);background:var(--rustdl-accent-soft,#70dfc90d);font-size:.75rem}@media(max-width:920px){.watch-grid,.calendar-grid{grid-template-columns:repeat(2,minmax(0,1fr))}}@media(max-width:580px){body{padding:.65rem}.library-top{overflow-x:auto}.library-top a:first-child{margin-right:0}.library-top a{flex:none}.library-top .catalog{margin-left:auto}header{align-items:start;flex-direction:column}.watch-grid,.calendar-grid{grid-template-columns:1fr}.watch-card{grid-template-columns:6.8rem minmax(0,1fr)}.library-actions{width:100%}.library-actions a{flex:1}.day{padding:.7rem}}@media(prefers-reduced-motion:reduce){.watch-card{transition:none}}
"#;

pub(crate) fn render_watchlist(entries: &[WatchlistEntry], token: &str) -> String {
    let cards = if entries.is_empty() {
        r#"<p class="empty"><strong>Your watchlist is empty.</strong><a href="/streaming">Browse the catalog</a> and tap ＋ on any show.</p>"#.to_owned()
    } else {
        entries
            .iter()
            .map(|entry| {
                format!(
                    r#"<article class="watch-card"><a class="watch-open" href="{}">{}<div class="watch-copy"><strong>{}</strong><div class="badges">{}</div><small>{}</small></div></a><form class="remove-form" action="/__app/watchlist" method="post"><input type="hidden" name="token" value="{}"><input type="hidden" name="action" value="remove"><input type="hidden" name="url" value="{}"><input type="hidden" name="return" value="/streaming/watchlist"><button class="remove-watch" type="submit" aria-label="Remove {} from watchlist">✓</button></form></article>"#,
                    escape_html(&launch_url(&entry.watch_url)),
                    poster(entry),
                    escape_html(&entry.title),
                    badges(entry),
                    escape_html(entry.media_type.as_deref().unwrap_or("Series")),
                    escape_html(token),
                    escape_html(&entry.watch_url),
                    escape_html(&entry.title),
                )
            })
            .collect()
    };
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Streaming watchlist · RustDL</title><style>{LIBRARY_CSS}</style></head><body><main><nav class="library-top"><a href="/">← RustDL</a><a class="catalog" href="/streaming">Catalog</a><a class="active" href="/streaming/watchlist" aria-current="page">Watchlist</a><a href="/streaming/calendar">Calendar</a></nav><header><div><span>Saved locally</span><h1>Your watchlist.</h1><p>One-tap playback through RustDL’s protected player.</p></div><p id="watch-count">{} saved</p></header><section class="watch-grid">{cards}</section></main><script>(()=>{{const count=document.querySelector('#watch-count');document.addEventListener('submit',async event=>{{const form=event.target;if(!form.matches('.remove-form'))return;event.preventDefault();const card=form.closest('.watch-card'),endpoint=form.getAttribute('action'),remaining=document.querySelectorAll('.watch-card').length-1;form.querySelector('button').disabled=true;try{{const response=await fetch(endpoint,{{method:'POST',body:new URLSearchParams(new FormData(form)),headers:{{Accept:'application/json'}}}});if(!response.ok)throw new Error();const remove=()=>card.remove();if(document.startViewTransition)document.startViewTransition(remove);else remove();count.textContent=remaining+' saved';if(remaining===0)location.reload()}}catch(_error){{form.querySelector('button').disabled=false}}}})}})();</script></body></html>"#,
        entries.len(),
    )
}

struct CalendarCard<'a> {
    entry: &'a WatchlistEntry,
    title: &'a str,
    poster_url: Option<&'a str>,
    latest_episode: &'a str,
    display_release: Option<u64>,
    is_new: bool,
    unavailable: bool,
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
    let today = weekday(now);
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
                .map(render_calendar_card)
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
            groups[7].iter().map(render_calendar_card).collect::<String>(),
        ));
    }
    if entries.is_empty() {
        sections = r#"<p class="empty"><strong>No saved shows to schedule.</strong><a href="/streaming">Add shows from the catalog</a> first.</p>"#.to_owned();
    }
    let visit = previous_visit.map_or_else(
        || "This is your first calendar check.".to_owned(),
        |_| "New badges compare against your previous calendar check.".to_owned(),
    );
    let html = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Release calendar · RustDL</title><style>{LIBRARY_CSS}</style></head><body><main><nav class="library-top"><a href="/">← RustDL</a><a class="catalog" href="/streaming">Catalog</a><a href="/streaming/watchlist">Watchlist</a><a class="active" href="/streaming/calendar" aria-current="page">Calendar</a></nav><header><div><span>Release rhythm</span><h1>Your calendar.</h1><p>Saved shows grouped by their latest known release day.</p></div><div class="library-actions"><a href="/streaming/calendar?refresh=1">Refresh schedules</a></div></header><p class="notice">{visit} {new_count} show{} new right now. Times are converted to this phone’s timezone.</p><nav class="day-nav" aria-label="Release days">{day_nav}</nav>{sections}</main><script>document.querySelectorAll('time[data-unix]').forEach(time=>{{const date=new Date(Number(time.dataset.unix)*1000);time.textContent=date.toLocaleString([],{{month:'short',day:'numeric',hour:'numeric',minute:'2-digit'}})}});</script></body></html>"#,
        if new_count == 1 { " is" } else { "s are" },
    );
    (html, seen)
}

fn render_calendar_card(card: &CalendarCard<'_>) -> String {
    let poster = card.poster_url.map_or_else(
        || r#"<div class="library-poster fallback" aria-hidden="true">R</div>"#.to_owned(),
        |poster| format!(r#"<div class="library-poster"><img src="{}" alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer"></div>"#, escape_html(poster)),
    );
    let release = card.display_release.map_or_else(
        || "Release time unavailable".to_owned(),
        |released| format!(r#"<time data-unix="{released}">Scheduled release</time>"#),
    );
    format!(
        r#"<a class="calendar-card" href="{}">{poster}<div class="calendar-copy"><strong>{}</strong><span>{} EP {}</span>{release}</div>{}</a>"#,
        escape_html(&launch_url(&card.entry.watch_url)),
        escape_html(card.title),
        if card.unavailable {
            "Last seen"
        } else {
            "Latest"
        },
        escape_html(card.latest_episode),
        if card.is_new {
            r#"<b class="new-badge">NEW</b>"#
        } else {
            ""
        },
    )
}

fn weekday(timestamp: u64) -> usize {
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
    fn persists_deduplicates_and_removes_watchlist_entries() {
        let directory = test_directory();
        let url = "https://aniwaves.ru/watch/test-show-123";
        assert!(add(&directory, input(url)).unwrap());
        assert!(add(&directory, input(url)).unwrap());
        let library = load(&directory).unwrap();
        assert_eq!(library.entries.len(), 1);
        let html = render_watchlist(&library.entries, "test-token");
        assert!(html.contains("form.getAttribute('action')"));
        assert!(!html.contains("fetch(form.action"));
        assert!(!remove(&directory, url).unwrap());
        assert!(load(&directory).unwrap().entries.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn calendar_marks_only_changes_after_a_successful_seen_state() {
        let directory = test_directory();
        let url = "https://aniwaves.ru/watch/test-show-123";
        add(&directory, input(url)).unwrap();
        let first = CalendarSeen {
            watch_url: url.to_owned(),
            episode: "8".to_owned(),
            released_at: Some(1_787_414_400),
        };
        mark_calendar_seen(&directory, std::slice::from_ref(&first), 1_787_414_400).unwrap();
        let library = load(&directory).unwrap();
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
        let (html, seen) = render_calendar(
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
        assert_eq!(weekday(0), 3);
    }
}
