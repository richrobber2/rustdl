//! AniWaves catalog, schedule, and player requests with bounded caches.

use super::super::local::aniwaves::{
    Catalog, CatalogView, MAX_AJAX_BYTES, MAX_CATALOG_BYTES, MAX_CATALOG_PAGE, MAX_WATCH_BYTES,
    ORIGIN, PLAYER_USER_AGENT, PlayerProbe, StreamManifest, StreamSchedule, StreamSource,
};
use super::super::{external, local};
use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, REFERER, USER_AGENT};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io::Read;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

pub(in super::super) const MAX_CATALOG_CACHE_ENTRIES: usize = 64;

pub(in super::super) const MAX_SCHEDULE_CACHE_ENTRIES: usize = 200;

pub(in super::super) const CACHE_TTL: Duration = Duration::from_secs(5 * 60);

pub(in super::super) const MANIFEST_CACHE_TTL: Duration = Duration::from_secs(2 * 60);

#[derive(Clone)]
pub(in super::super) struct CachedCatalog {
    pub(in super::super) fetched: Instant,
    pub(in super::super) catalog: Catalog,
}

pub(in super::super) static CATALOG_CACHE: OnceLock<Mutex<HashMap<String, CachedCatalog>>> =
    OnceLock::new();

#[derive(Clone)]
pub(in super::super) struct CachedManifest {
    pub(in super::super) fetched: Instant,
    pub(in super::super) manifest: StreamManifest,
}

pub(in super::super) static MANIFEST_CACHE: OnceLock<Mutex<HashMap<String, CachedManifest>>> =
    OnceLock::new();

#[derive(Clone)]
pub(in super::super) struct CachedSchedule {
    pub(in super::super) fetched: Instant,
    pub(in super::super) schedule: StreamSchedule,
}

pub(in super::super) static SCHEDULE_CACHE: OnceLock<Mutex<HashMap<String, CachedSchedule>>> =
    OnceLock::new();

pub(crate) fn load_catalog(
    client: &Client,
    view: CatalogView,
    page: u16,
    refresh: bool,
) -> Result<Catalog, Box<dyn Error>> {
    if !(1..=MAX_CATALOG_PAGE).contains(&page) {
        return Err("AniWaves catalog page is out of range".into());
    }
    let cache_key = format!("{}:{page}", view.cache_key());
    let cache = CATALOG_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if !refresh {
        let cached = cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&cache_key)
            .filter(|cached| cached.fetched.elapsed() < CACHE_TTL)
            .cloned();
        if let Some(cached) = cached {
            return Ok(cached.catalog);
        }
    }

    let url = local::aniwaves::upstream_catalog_url(&view, page)?;
    let response = client
        .get(url)
        .timeout(Duration::from_secs(20))
        .send()?
        .error_for_status()?;
    if response.url().scheme() != "https"
        || !response
            .url()
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("aniwaves.ru"))
    {
        return Err("AniWaves redirected the catalog to an unsupported host".into());
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_CATALOG_BYTES)
    {
        return Err("AniWaves catalog response is unexpectedly large".into());
    }
    let mut html = String::new();
    response
        .take(MAX_CATALOG_BYTES + 1)
        .read_to_string(&mut html)?;
    if html.len() as u64 > MAX_CATALOG_BYTES {
        return Err("AniWaves catalog response is unexpectedly large".into());
    }
    let catalog = local::aniwaves::parse_catalog_html(view, page, &html)?;
    let mut cache = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.retain(|_, cached| cached.fetched.elapsed() < CACHE_TTL);
    if cache.len() >= MAX_CATALOG_CACHE_ENTRIES && !cache.contains_key(&cache_key) {
        let oldest = cache
            .iter()
            .min_by_key(|(_, cached)| cached.fetched)
            .map(|(key, _)| key.clone());
        if let Some(oldest) = oldest {
            cache.remove(&oldest);
        }
    }
    cache.insert(
        cache_key,
        CachedCatalog {
            fetched: Instant::now(),
            catalog: catalog.clone(),
        },
    );
    Ok(catalog)
}

pub(crate) fn load_stream_manifest(
    client: &Client,
    watch_url: &str,
    requested_episode: Option<&str>,
    refresh: bool,
) -> Result<StreamManifest, Box<dyn Error>> {
    let watch_url = local::aniwaves::validate_watch_page_url(watch_url)?;
    let cache_key = format!("{}#{}", watch_url, requested_episode.unwrap_or("latest"));
    let cache = MANIFEST_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if !refresh {
        let cached = cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&cache_key)
            .filter(|cached| cached.fetched.elapsed() < MANIFEST_CACHE_TTL)
            .cloned();
        if let Some(cached) = cached {
            return Ok(cached.manifest);
        }
    }

    let schedule = external::aniwaves::load_stream_schedule(client, &watch_url, refresh)?;
    let show_id = &schedule.show_id;
    let title = schedule.title;
    let poster_url = schedule.poster_url;
    let episodes = schedule.episodes;
    let episode = requested_episode
        .filter(|requested| episodes.iter().any(|item| item.number == *requested))
        .map(str::to_owned)
        .unwrap_or_else(|| {
            episodes
                .last()
                .expect("episodes is not empty")
                .number
                .clone()
        });

    let server_value = external::aniwaves::fetch_ajax_value(
        client,
        "/ajax/server/list",
        &[("servers", show_id.as_str()), ("eps", episode.as_str())],
    )?;
    let server_html = local::aniwaves::ajax_result_html(&server_value)?;
    let entries = local::aniwaves::parse_stream_server_entries(server_html);
    let mut sources = thread::scope(|scope| {
        let handles = entries
            .into_iter()
            .take(10)
            .map(|entry| {
                scope.spawn(move || {
                    external::aniwaves::resolve_stream_source(client, entry.0, entry.1, entry.2)
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .filter_map(|handle| handle.join().ok().flatten())
            .collect::<Vec<_>>()
    });
    let mut seen = HashSet::new();
    sources.retain(|source| seen.insert(source.url.clone()));
    sources.sort_by_key(|source| {
        (
            !source.available,
            local::aniwaves::source_priority(&source.label),
        )
    });
    if sources.is_empty() {
        return Err("AniWaves returned no supported player sources for this episode".into());
    }

    let manifest = StreamManifest {
        title,
        poster_url,
        episode,
        episodes,
        sources,
    };
    cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(
            cache_key,
            CachedManifest {
                fetched: Instant::now(),
                manifest: manifest.clone(),
            },
        );
    Ok(manifest)
}

pub(crate) fn load_stream_schedule(
    client: &Client,
    watch_url: &str,
    refresh: bool,
) -> Result<StreamSchedule, Box<dyn Error>> {
    let watch_url = local::aniwaves::validate_watch_page_url(watch_url)?;
    let cache = SCHEDULE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if !refresh {
        let cached = cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&watch_url)
            .filter(|cached| cached.fetched.elapsed() < CACHE_TTL)
            .cloned();
        if let Some(cached) = cached {
            return Ok(cached.schedule);
        }
    }

    let watch_html = external::aniwaves::fetch_limited_html(client, &watch_url, MAX_WATCH_BYTES)?;
    let show_id =
        local::aniwaves::watch_show_id(&watch_html).ok_or("the watch page has no valid show id")?;
    let title = local::aniwaves::watch_page_title(&watch_html)
        .unwrap_or_else(|| "AniWaves stream".to_owned());
    let poster_url = local::aniwaves::watch_page_poster(&watch_html);
    let episode_value = external::aniwaves::fetch_ajax_value(
        client,
        &format!("/ajax/episode/list/{show_id}"),
        &[],
    )?;
    let episode_html = local::aniwaves::ajax_result_html(&episode_value)?;
    let episodes = local::aniwaves::parse_stream_episodes(episode_html);
    if episodes.is_empty() {
        return Err("AniWaves returned no playable episodes".into());
    }
    let schedule = StreamSchedule {
        title,
        poster_url,
        episodes,
        show_id,
    };
    let mut cache = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.retain(|_, cached| cached.fetched.elapsed() < CACHE_TTL);
    if cache.len() >= MAX_SCHEDULE_CACHE_ENTRIES && !cache.contains_key(&watch_url) {
        let oldest = cache
            .iter()
            .min_by_key(|(_, cached)| cached.fetched)
            .map(|(key, _)| key.clone());
        if let Some(oldest) = oldest {
            cache.remove(&oldest);
        }
    }
    cache.insert(
        watch_url,
        CachedSchedule {
            fetched: Instant::now(),
            schedule: schedule.clone(),
        },
    );
    Ok(schedule)
}

pub(in super::super) fn fetch_limited_html(
    client: &Client,
    url: &str,
    limit: u64,
) -> Result<String, Box<dyn Error>> {
    let response = client
        .get(url)
        .timeout(Duration::from_secs(20))
        .send()?
        .error_for_status()?;
    if response
        .content_length()
        .is_some_and(|length| length > limit)
    {
        return Err("the streaming response is unexpectedly large".into());
    }
    if response.url().scheme() != "https"
        || !response
            .url()
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("aniwaves.ru"))
    {
        return Err("AniWaves redirected outside its supported origin".into());
    }
    let mut body = String::new();
    response.take(limit + 1).read_to_string(&mut body)?;
    if body.len() as u64 > limit {
        return Err("the streaming response is unexpectedly large".into());
    }
    Ok(body)
}

pub(in super::super) fn fetch_ajax_value(
    client: &Client,
    path: &str,
    query: &[(&str, &str)],
) -> Result<Value, Box<dyn Error>> {
    let mut url = Url::parse(ORIGIN)?;
    url.set_path(path);
    url.query_pairs_mut().extend_pairs(query.iter().copied());
    let response = client
        .get(url)
        .header(ACCEPT, "application/json")
        .header("X-Requested-With", "XMLHttpRequest")
        .timeout(Duration::from_secs(20))
        .send()?
        .error_for_status()?;
    if response.url().scheme() != "https"
        || !response
            .url()
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("aniwaves.ru"))
    {
        return Err("AniWaves redirected an API request outside its origin".into());
    }
    let mut body = String::new();
    response
        .take(MAX_AJAX_BYTES + 1)
        .read_to_string(&mut body)?;
    if body.len() as u64 > MAX_AJAX_BYTES {
        return Err("the streaming API response is unexpectedly large".into());
    }
    let value = serde_json::from_str::<Value>(&body)?;
    if value.get("status").and_then(Value::as_u64) != Some(200) {
        return Err("AniWaves could not resolve that streaming selection".into());
    }
    Ok(value)
}

pub(in super::super) fn resolve_stream_source(
    client: &Client,
    label: String,
    language: String,
    link_id: String,
) -> Option<StreamSource> {
    let value =
        external::aniwaves::fetch_ajax_value(client, "/ajax/sources", &[("id", link_id.as_str())])
            .ok()?;
    let url = value
        .get("result")?
        .get("url")?
        .as_str()
        .and_then(local::aniwaves::validate_player_url)?;
    let probe = external::aniwaves::probe_player_source(client, &url);
    Some(StreamSource {
        label,
        language,
        url: probe.url,
        available: probe.available,
        redirected: probe.redirected,
        allowed_hosts: probe.allowed_hosts,
        issue: probe.issue,
    })
}

pub(in super::super) fn probe_player_source(client: &Client, url: &str) -> PlayerProbe {
    match client
        .get(url)
        .header(USER_AGENT, PLAYER_USER_AGENT)
        .header(REFERER, format!("{ORIGIN}/"))
        .timeout(Duration::from_secs(7))
        .send()
    {
        Ok(response) => {
            let status = response.status();
            let Some(final_url) = local::aniwaves::validate_player_url(response.url().as_str())
            else {
                return PlayerProbe {
                    url: url.to_owned(),
                    available: false,
                    redirected: false,
                    allowed_hosts: local::aniwaves::player_allowed_hosts(url, url),
                    issue: Some("Unsafe redirect blocked".to_owned()),
                };
            };
            PlayerProbe {
                redirected: final_url != url,
                allowed_hosts: local::aniwaves::player_allowed_hosts(url, &final_url),
                url: final_url,
                available: status.is_success(),
                issue: (!status.is_success()).then(|| format!("HTTP {}", status.as_u16())),
            }
        }
        Err(error) => PlayerProbe {
            url: url.to_owned(),
            available: false,
            redirected: false,
            allowed_hosts: local::aniwaves::player_allowed_hosts(url, url),
            issue: Some(if error.is_timeout() {
                "Timed out".to_owned()
            } else if error.is_connect() {
                "Connection failed".to_owned()
            } else {
                "Health check failed".to_owned()
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the live AniWaves catalog and player hosts"]
    fn resolves_multiple_live_backup_sources() {
        let client = Client::builder()
            .user_agent("rustdl-manifest-test")
            .build()
            .expect("build live manifest client");
        let manifest = external::aniwaves::load_stream_manifest(
            &client,
            "https://aniwaves.ru/watch/grow-up-show-himawari-no-circus-dan-82697",
            Some("1"),
            true,
        )
        .expect("resolve live stream manifest");
        assert!(manifest.sources.len() >= 2);
        assert!(
            manifest
                .sources
                .iter()
                .all(|source| source.url.starts_with("https://"))
        );
        assert!(
            manifest
                .sources
                .iter()
                .all(|source| !source.url.contains("aniwaves.ru"))
        );
    }

    #[test]
    #[ignore = "requires the live AniWaves browse and search pages"]
    fn loads_live_browse_and_full_library_search() {
        let client = Client::builder()
            .user_agent("rustdl-catalog-test")
            .build()
            .expect("build live catalog client");
        let updated = external::aniwaves::load_catalog(&client, CatalogView::Updated, 1, true)
            .expect("load updated catalog");
        assert!(!updated.items.is_empty());
        let search = external::aniwaves::load_catalog(
            &client,
            CatalogView::Search("One Piece".to_owned()),
            1,
            true,
        )
        .expect("search full catalog");
        assert!(
            search
                .items
                .iter()
                .any(|item| item.title.eq_ignore_ascii_case("One Piece"))
        );
    }
}
