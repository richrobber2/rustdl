use super::{escape_html, html_attribute, html_entity_decode};
use reqwest::{
    Url,
    blocking::Client,
    header::{ACCEPT, REFERER, USER_AGENT},
};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io::Read;
use std::net::IpAddr;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const ORIGIN: &str = "https://aniwaves.ru";
const MAX_CATALOG_BYTES: u64 = 1_500_000;
const MAX_WATCH_BYTES: u64 = 1_000_000;
const MAX_AJAX_BYTES: u64 = 1_500_000;
const MAX_CATALOG_PAGE: u16 = 50;
const MAX_CATALOG_CACHE_ENTRIES: usize = 64;
const MAX_SCHEDULE_CACHE_ENTRIES: usize = 200;
const CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const MANIFEST_CACHE_TTL: Duration = Duration::from_secs(2 * 60);
const PLAYER_USER_AGENT: &str =
    "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 Chrome/140.0 Mobile Safari/537.36";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CatalogItem {
    pub title: String,
    pub watch_url: String,
    pub poster_url: String,
    pub sub: Option<String>,
    pub dub: Option<String>,
    pub total: Option<String>,
    pub media_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Catalog {
    pub view: CatalogView,
    pub page: u16,
    pub pages: u16,
    pub items: Vec<CatalogItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CatalogView {
    Newest,
    Updated,
    Ongoing,
    Added,
    Search(String),
}

impl CatalogView {
    pub(crate) fn from_params(
        section: Option<&str>,
        query: Option<&str>,
    ) -> Result<Self, &'static str> {
        if let Some(query) = query.map(str::trim).filter(|query| !query.is_empty()) {
            return normalize_search_query(query).map(Self::Search);
        }
        match section
            .unwrap_or("newest")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "newest" => Ok(Self::Newest),
            "updated" => Ok(Self::Updated),
            "ongoing" => Ok(Self::Ongoing),
            "added" => Ok(Self::Added),
            _ => Err("unsupported AniWaves catalog section"),
        }
    }

    fn section_slug(&self) -> Option<&'static str> {
        match self {
            Self::Newest => Some("newest"),
            Self::Updated => Some("updated"),
            Self::Ongoing => Some("ongoing"),
            Self::Added => Some("added"),
            Self::Search(_) => None,
        }
    }

    fn cache_key(&self) -> String {
        match self {
            Self::Search(query) => format!("search:{}", query.to_lowercase()),
            _ => self
                .section_slug()
                .expect("browse sections have slugs")
                .to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CatalogTarget {
    pub view: CatalogView,
    pub page: u16,
}

#[derive(Clone)]
struct CachedCatalog {
    fetched: Instant,
    catalog: Catalog,
}

static CATALOG_CACHE: OnceLock<Mutex<HashMap<String, CachedCatalog>>> = OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StreamEpisode {
    pub number: String,
    pub title: String,
    pub released_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StreamSchedule {
    pub title: String,
    pub poster_url: Option<String>,
    pub episodes: Vec<StreamEpisode>,
    pub(crate) show_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StreamSource {
    pub label: String,
    pub language: String,
    pub url: String,
    pub available: bool,
    pub redirected: bool,
    pub allowed_hosts: Vec<String>,
    pub issue: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StreamManifest {
    pub title: String,
    pub poster_url: Option<String>,
    pub episode: String,
    pub episodes: Vec<StreamEpisode>,
    pub sources: Vec<StreamSource>,
}

#[derive(Clone)]
struct CachedManifest {
    fetched: Instant,
    manifest: StreamManifest,
}

static MANIFEST_CACHE: OnceLock<Mutex<HashMap<String, CachedManifest>>> = OnceLock::new();

#[derive(Clone)]
struct CachedSchedule {
    fetched: Instant,
    schedule: StreamSchedule,
}

static SCHEDULE_CACHE: OnceLock<Mutex<HashMap<String, CachedSchedule>>> = OnceLock::new();

pub(crate) fn catalog_target_from_text(value: &str) -> Option<CatalogTarget> {
    value.split_whitespace().find_map(|candidate| {
        let candidate = candidate.trim_matches(|character: char| {
            matches!(
                character,
                '<' | '>' | '(' | ')' | '[' | ']' | '"' | '\'' | ','
            )
        });
        catalog_target_from_url(candidate)
    })
}

fn catalog_target_from_url(value: &str) -> Option<CatalogTarget> {
    let parsed = Url::parse(value).ok()?;
    if parsed.scheme() != "https"
        || !parsed
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("aniwaves.ru"))
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return None;
    }

    let page_from_query = parsed
        .query_pairs()
        .find(|(key, _)| key == "page")
        .and_then(|(_, value)| value.parse::<u16>().ok())
        .unwrap_or(1);
    if !(1..=MAX_CATALOG_PAGE).contains(&page_from_query) {
        return None;
    }
    let path = parsed.path().trim_end_matches('/');
    if matches!(path, "/filter" | "/search") {
        let query = parsed
            .query_pairs()
            .find(|(key, _)| key == "keyword")
            .map(|(_, value)| value.into_owned())?;
        return Some(CatalogTarget {
            view: CatalogView::Search(normalize_search_query(&query).ok()?),
            page: page_from_query,
        });
    }
    if matches!(path, "" | "/home") {
        return (page_from_query == 1).then_some(CatalogTarget {
            view: CatalogView::Newest,
            page: 1,
        });
    }
    for section in ["newest", "updated", "ongoing", "added"] {
        let root = format!("/{section}");
        if path == root {
            return Some(CatalogTarget {
                view: CatalogView::from_params(Some(section), None).ok()?,
                page: page_from_query,
            });
        }
        if let Some(page) = path
            .strip_prefix(&format!("/{section}/page/"))
            .and_then(|page| page.parse::<u16>().ok())
            && (1..=MAX_CATALOG_PAGE).contains(&page)
        {
            return Some(CatalogTarget {
                view: CatalogView::from_params(Some(section), None).ok()?,
                page,
            });
        }
    }
    None
}

fn normalize_search_query(query: &str) -> Result<String, &'static str> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > 100 || query.chars().any(char::is_control) {
        return Err("search must contain 1 to 100 visible characters");
    }
    Ok(query.to_owned())
}

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

    let url = upstream_catalog_url(&view, page)?;
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
    let catalog = parse_catalog_html(view, page, &html)?;
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

fn upstream_catalog_url(view: &CatalogView, page: u16) -> Result<Url, Box<dyn Error>> {
    let mut url = Url::parse(ORIGIN)?;
    match view {
        CatalogView::Search(query) => {
            url.set_path("/filter");
            url.query_pairs_mut()
                .append_pair("keyword", query)
                .append_pair("page", &page.to_string());
        }
        _ => {
            let section = view.section_slug().expect("browse sections have slugs");
            let path = if page == 1 {
                format!("/{section}")
            } else {
                format!("/{section}/page/{page}")
            };
            url.set_path(&path);
        }
    }
    Ok(url)
}

fn parse_catalog_html(view: CatalogView, page: u16, html: &str) -> Result<Catalog, Box<dyn Error>> {
    let items = html
        .split("<div class=\"item ")
        .skip(1)
        .take(60)
        .filter_map(parse_catalog_item)
        .collect::<Vec<_>>();
    if items.is_empty() && !matches!(view, CatalogView::Search(_)) {
        return Err("AniWaves returned no streaming catalog entries".into());
    }
    let pages = catalog_page_count(html, &view)
        .max(page)
        .clamp(1, MAX_CATALOG_PAGE);
    Ok(Catalog {
        view,
        page,
        pages,
        items,
    })
}

fn parse_catalog_item(fragment: &str) -> Option<CatalogItem> {
    let name_marker = fragment.find("class=\"name d-title\"")?;
    let name_tag_start = fragment[..name_marker].rfind("<a ")? + 3;
    let name_tag_end = fragment[name_marker..].find('>')? + name_marker;
    let name_tag = &fragment[name_tag_start..name_tag_end];
    let href = html_attribute(name_tag, "href")?;
    let watch_url = validated_watch_url(href)?;
    let title_end = fragment[name_tag_end + 1..].find('<')? + name_tag_end + 1;
    let title = html_entity_decode(fragment[name_tag_end + 1..title_end].trim());
    if title.is_empty() || title.len() > 300 {
        return None;
    }

    let image_start = fragment.find("<img ")? + 5;
    let image_end = fragment[image_start..].find('>')? + image_start;
    let poster_url =
        validated_poster_url(html_attribute(&fragment[image_start..image_end], "src")?)?;

    Some(CatalogItem {
        title,
        watch_url,
        poster_url,
        sub: status_value(fragment, "sub"),
        dub: status_value(fragment, "dub"),
        total: status_value(fragment, "total"),
        media_type: div_text(fragment, "right"),
    })
}

fn validated_watch_url(href: &str) -> Option<String> {
    if href.len() > 280
        || !href.starts_with("/watch/")
        || !href
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_'))
    {
        return None;
    }
    Some(format!("{ORIGIN}{href}"))
}

pub(crate) fn validated_poster_url(value: &str) -> Option<String> {
    let parsed = Url::parse(value).ok()?;
    (parsed.scheme() == "https"
        && parsed
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("static.aniwaves.ru"))
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.path().starts_with("/resources/thumbnails/")
        && value.len() <= 500)
        .then(|| parsed.to_string())
}

fn status_value(fragment: &str, class_name: &str) -> Option<String> {
    let marker = format!("class=\"ep-status {class_name}\"");
    let remainder = &fragment[fragment.find(&marker)? + marker.len()..];
    let start = remainder.find("<span>")? + "<span>".len();
    let end = remainder[start..].find('<')? + start;
    let value = remainder[start..end].trim();
    (!value.is_empty() && value.len() <= 8).then(|| html_entity_decode(value))
}

fn div_text(fragment: &str, class_name: &str) -> Option<String> {
    let marker = format!("<div class=\"{class_name}\">");
    let remainder = &fragment[fragment.find(&marker)? + marker.len()..];
    let end = remainder.find('<')?;
    let value = remainder[..end].trim();
    (!value.is_empty() && value.len() <= 24).then(|| html_entity_decode(value))
}

fn catalog_page_count(html: &str, view: &CatalogView) -> u16 {
    match view {
        CatalogView::Search(_) => ["&page=", "&amp;page="]
            .into_iter()
            .map(|marker| maximum_page_after(html, marker))
            .max()
            .unwrap_or(1),
        _ => maximum_page_after(
            html,
            &format!(
                "/{}/page/",
                view.section_slug().expect("browse sections have slugs")
            ),
        ),
    }
}

fn maximum_page_after(html: &str, marker: &str) -> u16 {
    let mut maximum = 1;
    let mut remainder = html;
    while let Some(index) = remainder.find(marker) {
        remainder = &remainder[index + marker.len()..];
        let digits = remainder.bytes().take_while(u8::is_ascii_digit).count();
        if let Ok(page) = remainder[..digits].parse::<u16>() {
            maximum = maximum.max(page.min(MAX_CATALOG_PAGE));
        }
        remainder = &remainder[digits..];
    }
    maximum
}

pub(crate) fn load_stream_manifest(
    client: &Client,
    watch_url: &str,
    requested_episode: Option<&str>,
    refresh: bool,
) -> Result<StreamManifest, Box<dyn Error>> {
    let watch_url = validate_watch_page_url(watch_url)?;
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

    let schedule = load_stream_schedule(client, &watch_url, refresh)?;
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

    let server_value = fetch_ajax_value(
        client,
        "/ajax/server/list",
        &[("servers", show_id.as_str()), ("eps", episode.as_str())],
    )?;
    let server_html = ajax_result_html(&server_value)?;
    let entries = parse_stream_server_entries(server_html);
    let mut sources = thread::scope(|scope| {
        let handles = entries
            .into_iter()
            .take(10)
            .map(|entry| {
                scope.spawn(move || resolve_stream_source(client, entry.0, entry.1, entry.2))
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .filter_map(|handle| handle.join().ok().flatten())
            .collect::<Vec<_>>()
    });
    let mut seen = HashSet::new();
    sources.retain(|source| seen.insert(source.url.clone()));
    sources.sort_by_key(|source| (!source.available, source_priority(&source.label)));
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
    let watch_url = validate_watch_page_url(watch_url)?;
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

    let watch_html = fetch_limited_html(client, &watch_url, MAX_WATCH_BYTES)?;
    let show_id = watch_show_id(&watch_html).ok_or("the watch page has no valid show id")?;
    let title = watch_page_title(&watch_html).unwrap_or_else(|| "AniWaves stream".to_owned());
    let poster_url = watch_page_poster(&watch_html);
    let episode_value = fetch_ajax_value(client, &format!("/ajax/episode/list/{show_id}"), &[])?;
    let episode_html = ajax_result_html(&episode_value)?;
    let episodes = parse_stream_episodes(episode_html);
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

pub(crate) fn validate_watch_page_url(value: &str) -> Result<String, Box<dyn Error>> {
    if value.len() > 500 {
        return Err("the watch URL is too long".into());
    }
    let parsed = Url::parse(value)?;
    if parsed.scheme() != "https"
        || !parsed
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("aniwaves.ru"))
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !parsed.path().starts_with("/watch/")
        || !parsed
            .path()
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_'))
    {
        return Err("unsupported AniWaves watch URL".into());
    }
    Ok(parsed.to_string())
}

fn fetch_limited_html(client: &Client, url: &str, limit: u64) -> Result<String, Box<dyn Error>> {
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

fn fetch_ajax_value(
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

fn ajax_result_html(value: &Value) -> Result<&str, Box<dyn Error>> {
    value
        .get("result")
        .and_then(Value::as_str)
        .ok_or_else(|| "AniWaves returned an incomplete streaming response".into())
}

fn watch_show_id(html: &str) -> Option<String> {
    let marker = html.find("id=\"watch-main\"")?;
    let start = html[..marker].rfind('<')? + 1;
    let end = html[marker..].find('>')? + marker;
    let value = html_attribute(&html[start..end], "data-id")?;
    (value.len() <= 12 && value.bytes().all(|byte| byte.is_ascii_digit())).then(|| value.to_owned())
}

fn watch_page_title(html: &str) -> Option<String> {
    let mut remainder = html;
    while let Some(index) = remainder.find("<meta ") {
        remainder = &remainder[index + 6..];
        let end = remainder.find('>')?;
        let tag = &remainder[..end];
        if html_attribute(tag, "property").is_some_and(|value| value == "og:title") {
            let title = html_entity_decode(html_attribute(tag, "content")?)
                .trim()
                .to_owned();
            return (!title.is_empty() && title.len() <= 300).then_some(title);
        }
        remainder = &remainder[end + 1..];
    }
    None
}

fn watch_page_poster(html: &str) -> Option<String> {
    let mut remainder = html;
    while let Some(index) = remainder.find("<meta ") {
        remainder = &remainder[index + 6..];
        let end = remainder.find('>')?;
        let tag = &remainder[..end];
        if html_attribute(tag, "property").is_some_and(|value| value == "og:image") {
            return validated_poster_url(html_attribute(tag, "content")?);
        }
        remainder = &remainder[end + 1..];
    }
    None
}

fn parse_stream_episodes(html: &str) -> Vec<StreamEpisode> {
    let mut episodes = Vec::new();
    let mut seen = HashSet::new();
    for fragment in html.split("<a ").skip(1).take(2_000) {
        let Some(tag_end) = fragment.find('>') else {
            continue;
        };
        let tag = &fragment[..tag_end];
        let Some(number) = html_attribute(tag, "data-num") else {
            continue;
        };
        if number.is_empty()
            || number.len() > 12
            || !number
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'-'))
            || !seen.insert(number.to_owned())
        {
            continue;
        }
        let title = stream_episode_title(&fragment[tag_end + 1..], number);
        let released_at = html_attribute(tag, "data-timestamp").and_then(parse_utc_timestamp);
        episodes.push(StreamEpisode {
            number: number.to_owned(),
            title,
            released_at,
        });
    }
    episodes
}

fn parse_utc_timestamp(value: &str) -> Option<u64> {
    let (date, time) = value.split_once(' ')?;
    let mut date = date.split('-').map(str::parse::<i64>);
    let year = date.next()?.ok()?;
    let month = date.next()?.ok()?;
    let day = date.next()?.ok()?;
    if date.next().is_some() || !(1970..=2200).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    let month_days = match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=month_days).contains(&day) {
        return None;
    }
    let mut time = time.split(':').map(str::parse::<i64>);
    let hour = time.next()?.ok()?;
    let minute = time.next()?.ok()?;
    let second = time.next()?.ok()?;
    if time.next().is_some()
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
    {
        return None;
    }

    // Howard Hinnant's civil-date transform gives days since 1970-01-01
    // with integer-only arithmetic and no timezone dependency.
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    (days >= 0).then_some((days * 86_400 + hour * 3_600 + minute * 60 + second) as u64)
}

fn stream_episode_title(fragment: &str, number: &str) -> String {
    let title = fragment
        .find("class=\"d-title\"")
        .and_then(|marker| fragment[marker..].find('>').map(|index| marker + index + 1))
        .and_then(|start| {
            fragment[start..]
                .find('<')
                .map(|end| html_entity_decode(fragment[start..start + end].trim()))
        })
        .filter(|title| !title.is_empty() && title.len() <= 300);
    title.unwrap_or_else(|| format!("Episode {number}"))
}

fn parse_stream_server_entries(html: &str) -> Vec<(String, String, String)> {
    let mut entries = Vec::new();
    for block in html
        .split("<div class=\"type\" data-type=\"")
        .skip(1)
        .take(8)
    {
        let Some(language_end) = block.find('"') else {
            continue;
        };
        let language = &block[..language_end];
        if !matches!(language, "sub" | "dub" | "raw") {
            continue;
        }
        for item in block.split("<li ").skip(1).take(12) {
            let Some(tag_end) = item.find('>') else {
                continue;
            };
            let Some(link_id) = html_attribute(&item[..tag_end], "data-link-id") else {
                continue;
            };
            let label_end = item[tag_end + 1..].find('<').unwrap_or(0);
            let label = html_entity_decode(item[tag_end + 1..tag_end + 1 + label_end].trim());
            if link_id.is_empty()
                || link_id.len() > 4_096
                || !link_id.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=' | b'-' | b'_')
                })
                || label.is_empty()
                || label.len() > 40
            {
                continue;
            }
            entries.push((label, language.to_owned(), link_id.to_owned()));
        }
    }
    entries
}

fn resolve_stream_source(
    client: &Client,
    label: String,
    language: String,
    link_id: String,
) -> Option<StreamSource> {
    let value = fetch_ajax_value(client, "/ajax/sources", &[("id", link_id.as_str())]).ok()?;
    let url = value
        .get("result")?
        .get("url")?
        .as_str()
        .and_then(validate_player_url)?;
    let probe = probe_player_source(client, &url);
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

fn validate_player_url(value: &str) -> Option<String> {
    if value.len() > 8_192 {
        return None;
    }
    let parsed = Url::parse(value).ok()?;
    let host = parsed.host_str()?;
    (parsed.scheme() == "https"
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.fragment().is_none()
        && !host.eq_ignore_ascii_case("localhost")
        && host.parse::<IpAddr>().is_err())
    .then(|| parsed.to_string())
}

#[derive(Debug, PartialEq, Eq)]
struct PlayerProbe {
    url: String,
    available: bool,
    redirected: bool,
    allowed_hosts: Vec<String>,
    issue: Option<String>,
}

fn player_url_host(value: &str) -> Option<String> {
    validate_player_url(value).and_then(|url| {
        Url::parse(&url)
            .ok()?
            .host_str()
            .map(|host| host.to_ascii_lowercase())
    })
}

fn player_allowed_hosts(original_url: &str, final_url: &str) -> Vec<String> {
    let mut hosts = Vec::with_capacity(2);
    for url in [original_url, final_url] {
        if let Some(host) = player_url_host(url)
            && !hosts.contains(&host)
        {
            hosts.push(host);
        }
    }
    hosts
}

fn probe_player_source(client: &Client, url: &str) -> PlayerProbe {
    match client
        .get(url)
        .header(USER_AGENT, PLAYER_USER_AGENT)
        .header(REFERER, format!("{ORIGIN}/"))
        .timeout(Duration::from_secs(7))
        .send()
    {
        Ok(response) => {
            let status = response.status();
            let Some(final_url) = validate_player_url(response.url().as_str()) else {
                return PlayerProbe {
                    url: url.to_owned(),
                    available: false,
                    redirected: false,
                    allowed_hosts: player_allowed_hosts(url, url),
                    issue: Some("Unsafe redirect blocked".to_owned()),
                };
            };
            PlayerProbe {
                redirected: final_url != url,
                allowed_hosts: player_allowed_hosts(url, &final_url),
                url: final_url,
                available: status.is_success(),
                issue: (!status.is_success()).then(|| format!("HTTP {}", status.as_u16())),
            }
        }
        Err(error) => PlayerProbe {
            url: url.to_owned(),
            available: false,
            redirected: false,
            allowed_hosts: player_allowed_hosts(url, url),
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

fn source_priority(label: &str) -> u8 {
    match label.to_ascii_lowercase().as_str() {
        "byfms" => 0,
        "dghg" => 1,
        "datsav" => 2,
        "mycloud" => 3,
        "vidplay" => 4,
        _ => 5,
    }
}

fn catalog_local_url(view: &CatalogView, page: u16, refresh: bool) -> String {
    let mut url = Url::parse("http://rustdl.local/streaming").expect("static local URL");
    match view {
        CatalogView::Search(query) => {
            url.query_pairs_mut().append_pair("q", query);
        }
        _ => {
            url.query_pairs_mut().append_pair(
                "section",
                view.section_slug().expect("browse sections have slugs"),
            );
        }
    }
    if page > 1 {
        url.query_pairs_mut().append_pair("page", &page.to_string());
    }
    if refresh {
        url.query_pairs_mut().append_pair("refresh", "1");
    }
    let mut local = url.path().to_owned();
    if let Some(query) = url.query() {
        local.push('?');
        local.push_str(query);
    }
    local
}

fn render_catalog_tabs(active: &CatalogView) -> String {
    let mut tabs = [
        (CatalogView::Newest, "Newest"),
        (CatalogView::Updated, "Updated"),
        (CatalogView::Ongoing, "Ongoing"),
        (CatalogView::Added, "Added"),
    ]
    .into_iter()
    .map(|(view, label)| {
        let selected = &view == active;
        format!(
            r#"<a class="{}" href="{}"{}>{label}</a>"#,
            if selected { "active" } else { "" },
            escape_html(&catalog_local_url(&view, 1, false)),
            if selected {
                r#" aria-current="page""#
            } else {
                ""
            }
        )
    })
    .collect::<String>();
    tabs.push_str(
        r#"<a href="/streaming/watchlist">Watchlist</a><a href="/streaming/calendar">Calendar</a>"#,
    );
    tabs
}

fn catalog_heading(view: &CatalogView) -> (String, &'static str) {
    match view {
        CatalogView::Newest => (
            "Newest releases.".to_owned(),
            "Fresh premieres and newly released anime.",
        ),
        CatalogView::Updated => (
            "Recently updated.".to_owned(),
            "Series with the latest episode activity.",
        ),
        CatalogView::Ongoing => (
            "Currently airing.".to_owned(),
            "Browse ongoing series without leaving RustDL.",
        ),
        CatalogView::Added => (
            "Recently added.".to_owned(),
            "New additions from across the full library.",
        ),
        CatalogView::Search(query) => (
            format!("Results for “{query}”."),
            "Full-library results from AniWaves, opened in RustDL’s protected player.",
        ),
    }
}

pub(crate) fn render_catalog(
    catalog: &Catalog,
    watchlisted: &HashSet<String>,
    action_token: &str,
) -> String {
    let return_url = catalog_local_url(&catalog.view, catalog.page, false);
    let cards = if catalog.items.is_empty() {
        r#"<p class="stream-empty">No anime matched that search. Try a shorter title or another spelling.</p>"#
            .to_owned()
    } else {
        catalog
            .items
            .iter()
            .map(|item| {
            let mut launch_url = Url::parse("rustdl://stream").expect("static streaming URL");
            launch_url
                .query_pairs_mut()
                .append_pair("url", &item.watch_url);
            let mut badges = Vec::new();
            if let Some(value) = &item.sub {
                badges.push(format!("<span>SUB {}</span>", escape_html(value)));
            }
            if let Some(value) = &item.dub {
                badges.push(format!("<span>DUB {}</span>", escape_html(value)));
            }
            if let Some(value) = &item.total {
                badges.push(format!("<span>{} EPS</span>", escape_html(value)));
            }
            let media_type = item
                .media_type
                .as_deref()
                .map(escape_html)
                .unwrap_or_else(|| "Series".to_owned());
            let saved = watchlisted.contains(&item.watch_url);
            format!(
                r#"<article class="stream-card-shell"><a class="stream-card" href="{}"><div class="stream-poster"><img src="{}" alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer"><span class="stream-play" aria-hidden="true">▶</span></div><div class="stream-copy"><strong>{}</strong><div class="stream-badges">{}</div><small>{media_type}</small></div></a><form class="stream-save-form" action="/__app/watchlist" method="post"><input type="hidden" name="token" value="{}"><input type="hidden" name="action" value="{}"><input type="hidden" name="url" value="{}"><input type="hidden" name="title" value="{}"><input type="hidden" name="poster" value="{}"><input type="hidden" name="sub" value="{}"><input type="hidden" name="dub" value="{}"><input type="hidden" name="total" value="{}"><input type="hidden" name="type" value="{}"><input type="hidden" name="return" value="{}"><button class="{}" type="submit" aria-label="{} {}">{}</button></form></article>"#,
                escape_html(launch_url.as_str()),
                escape_html(&item.poster_url),
                escape_html(&item.title),
                badges.join(""),
                escape_html(action_token),
                if saved { "remove" } else { "add" },
                escape_html(&item.watch_url),
                escape_html(&item.title),
                escape_html(&item.poster_url),
                escape_html(item.sub.as_deref().unwrap_or("")),
                escape_html(item.dub.as_deref().unwrap_or("")),
                escape_html(item.total.as_deref().unwrap_or("")),
                escape_html(item.media_type.as_deref().unwrap_or("")),
                escape_html(&return_url),
                if saved { "saved" } else { "" },
                if saved { "Remove" } else { "Add" },
                escape_html(&item.title),
                if saved { "✓" } else { "＋" },
            )
        })
        .collect::<String>()
    };
    let refresh = escape_html(&catalog_local_url(&catalog.view, catalog.page, true));
    let previous = (catalog.page > 1).then(|| {
        format!(
            r#"<a href="{}">← Previous</a>"#,
            escape_html(&catalog_local_url(&catalog.view, catalog.page - 1, false))
        )
    });
    let next = (catalog.page < catalog.pages).then(|| {
        format!(
            r#"<a href="{}">Next →</a>"#,
            escape_html(&catalog_local_url(&catalog.view, catalog.page + 1, false))
        )
    });
    let search_value = match &catalog.view {
        CatalogView::Search(query) => escape_html(query),
        _ => String::new(),
    };
    let section_value = catalog.view.section_slug().unwrap_or("newest");
    let tabs = render_catalog_tabs(&catalog.view);
    let (heading, description) = catalog_heading(&catalog.view);
    let eyebrow = if matches!(catalog.view, CatalogView::Search(_)) {
        "Library search"
    } else {
        "Browse anime"
    };
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>AniWaves streaming · RustDL</title><style>
:root{{color-scheme:dark;font-family:Inter,ui-sans-serif,system-ui,sans-serif}}*{{box-sizing:border-box}}body{{min-height:100vh;margin:0;padding:clamp(.8rem,3vw,2rem);color:var(--rustdl-text,#f7f7f8);background:var(--rustdl-page,#090a0f)}}.stream-shell{{width:min(100%,1180px);margin:auto}}.stream-top{{position:sticky;z-index:10;top:0;display:grid;grid-template-columns:auto minmax(0,1fr) auto;align-items:center;gap:.7rem;padding:.7rem;border:1px solid var(--rustdl-surface-border,#ffffff1c);border-radius:18px;background:var(--rustdl-surface,#0d0f16e8);box-shadow:0 14px 40px #0008;backdrop-filter:blur(18px)}}.stream-top>a,.stream-search-form button{{display:grid;place-items:center;min-height:2.8rem;padding:.65rem .8rem;border:1px solid var(--rustdl-surface-border,#ffffff20);border-radius:12px;color:var(--rustdl-control-text,#dfe5ef);background:var(--rustdl-control,#181b24);text-decoration:none;font:800 .75rem/1 system-ui}}.stream-search-form{{display:grid;grid-template-columns:minmax(0,1fr) auto;gap:.45rem;margin:0}}.stream-search{{width:100%;min-height:2.8rem;padding:.7rem .85rem;border:1px solid var(--rustdl-input-border,#ffffff24);border-radius:12px;color:var(--rustdl-text,#fff);background:var(--rustdl-input,#080a10);font:inherit;outline:none}}.stream-search:focus{{border-color:var(--rustdl-accent,#70dfc9);box-shadow:0 0 0 3px var(--rustdl-focus,#70dfc922)}}.stream-tabs{{display:flex;gap:.5rem;overflow-x:auto;margin:.7rem 0 1rem;padding:.2rem;scrollbar-width:none}}.stream-tabs a{{flex:none;padding:.65rem .8rem;border:1px solid var(--rustdl-surface-border,#ffffff20);border-radius:999px;color:var(--rustdl-control-text,#dfe5ef);background:var(--rustdl-control,#181b24);text-decoration:none;font-size:.72rem;font-weight:850}}.stream-tabs a.active{{border-color:var(--rustdl-accent,#70dfc9);color:var(--rustdl-accent-ink,#07110f);background:var(--rustdl-accent,#70dfc9)}}.stream-heading{{display:flex;align-items:end;justify-content:space-between;gap:1rem;margin:1.2rem .2rem}}.stream-heading h1{{margin:.35rem 0;font-size:clamp(2rem,7vw,4rem);letter-spacing:-.055em}}.stream-heading p{{margin:0;color:var(--rustdl-muted,#9ca3b3)}}.stream-heading span{{color:var(--rustdl-accent,#70dfc9);font-size:.72rem;font-weight:850;letter-spacing:.1em;text-transform:uppercase}}.stream-grid{{display:grid;grid-template-columns:repeat(5,minmax(0,1fr));gap:1rem}}.stream-card-shell{{position:relative;min-width:0}}.stream-card{{display:block;height:100%;min-width:0;overflow:hidden;border:1px solid var(--rustdl-surface-border,#ffffff16);border-radius:18px;color:var(--rustdl-text,#f7f7f8);background:var(--rustdl-surface,#10121a);text-decoration:none;transition:transform .18s ease,border-color .18s ease}}.stream-card:hover,.stream-card:focus-visible{{transform:translateY(-3px);border-color:var(--rustdl-accent-border,#70dfc966);outline:none}}.stream-save-form{{position:absolute;z-index:4;right:.6rem;top:.6rem;margin:0}}.stream-save-form button{{display:grid;place-items:center;width:2.45rem;height:2.45rem;padding:0;border:1px solid var(--rustdl-surface-border,#ffffff2a);border-radius:50%;color:var(--rustdl-control-text,#e7ebf2);background:var(--rustdl-surface,#11131bea);box-shadow:0 6px 20px #0007;font:900 1rem system-ui;cursor:pointer}}.stream-save-form button:disabled{{opacity:.5}}.stream-save-form button.saved{{color:var(--rustdl-accent-ink,#07110f);border-color:var(--rustdl-accent,#70dfc9);background:var(--rustdl-accent,#70dfc9)}}.stream-poster{{position:relative;aspect-ratio:5/7;overflow:hidden;background:linear-gradient(145deg,#1b2140,#0b1514)}}.stream-poster img{{width:100%;height:100%;object-fit:cover;transition:transform .28s ease}}.stream-card:hover img{{transform:scale(1.035)}}.stream-poster::after{{content:"";position:absolute;inset:45% 0 0;background:linear-gradient(transparent,#08090ed9)}}.stream-play{{position:absolute;z-index:2;right:.7rem;bottom:.7rem;display:grid;place-items:center;width:2.5rem;height:2.5rem;border-radius:50%;color:var(--rustdl-accent-ink,#07110f);background:var(--rustdl-accent,#70dfc9);box-shadow:0 8px 24px #0008;font-size:.8rem}}.stream-copy{{display:grid;gap:.45rem;padding:.75rem}}.stream-copy strong{{display:-webkit-box;overflow:hidden;min-height:2.6em;-webkit-box-orient:vertical;-webkit-line-clamp:2;font-size:.82rem;line-height:1.3}}.stream-copy small{{color:var(--rustdl-muted,#8f98aa);font-size:.68rem}}.stream-badges{{display:flex;flex-wrap:wrap;gap:.3rem}}.stream-badges span{{padding:.26rem .38rem;border-radius:6px;color:var(--rustdl-accent,#9ce9db);background:var(--rustdl-accent-soft,#70dfc914);font-size:.58rem;font-weight:900}}.stream-empty{{grid-column:1/-1;padding:2rem;border:1px dashed var(--rustdl-surface-border,#ffffff24);border-radius:18px;color:var(--rustdl-muted,#8f98aa);text-align:center}}.stream-pages{{display:flex;align-items:center;justify-content:center;gap:.65rem;margin:1.2rem 0}}.stream-pages a,.stream-pages span{{padding:.7rem .85rem;border:1px solid var(--rustdl-surface-border,#ffffff1f);border-radius:11px;color:var(--rustdl-control-text,#dfe5ef);background:var(--rustdl-control,#11131b);text-decoration:none;font-size:.72rem;font-weight:850}}.stream-pages span{{color:var(--rustdl-accent,#70dfc9)}}@media(max-width:900px){{.stream-grid{{grid-template-columns:repeat(4,minmax(0,1fr))}}}}@media(max-width:680px){{.stream-grid{{grid-template-columns:repeat(3,minmax(0,1fr));gap:.7rem}}}}@media(max-width:470px){{body{{padding:.65rem}}.stream-grid{{grid-template-columns:repeat(2,minmax(0,1fr))}}.stream-top{{grid-template-columns:auto minmax(0,1fr)}}.stream-top .refresh{{display:none}}.stream-heading{{align-items:start;flex-direction:column}}.stream-search-form button{{width:2.8rem;overflow:hidden;color:transparent}}.stream-search-form button::after{{content:"⌕";color:var(--rustdl-control-text,#dfe5ef);font-size:1rem}}}}@media(prefers-reduced-motion:reduce){{.stream-card,.stream-poster img{{transition:none}}}}
</style></head><body><main class="stream-shell"><nav class="stream-top"><a href="/">← RustDL</a><form class="stream-search-form" action="/streaming" method="get"><input type="hidden" name="section" value="{section_value}"><input class="stream-search" type="search" name="q" value="{search_value}" maxlength="100" placeholder="Search all anime…" aria-label="Search all anime" autocomplete="off"><button type="submit">Search</button></form><a class="refresh" href="{refresh}">Refresh</a></nav><nav class="stream-tabs" aria-label="Browse anime">{tabs}</nav><header class="stream-heading"><div><span>{eyebrow} · page {}/{}</span><h1>{}</h1><p>{description}</p></div><p>{} titles</p></header><section class="stream-grid">{cards}</section><nav class="stream-pages">{}<span>Page {} of {}</span>{}</nav></main><script>(()=>{{document.addEventListener('submit',async event=>{{const form=event.target;if(!form.matches('.stream-save-form'))return;event.preventDefault();const button=form.querySelector('button'),endpoint=form.getAttribute('action'),params=new URLSearchParams(new FormData(form));params.set('response','json');button.disabled=true;try{{const response=await fetch(endpoint,{{method:'POST',body:params,headers:{{Accept:'application/json'}}}});if(!response.ok)throw new Error();const value=await response.json(),saved=Boolean(value.watchlisted);form.elements.action.value=saved?'remove':'add';button.textContent=saved?'✓':'＋';button.classList.toggle('saved',saved);button.setAttribute('aria-label',(saved?'Remove ':'Add ')+form.elements.title.value)}}catch(_error){{}}finally{{button.disabled=false}}}})}})();</script></body></html>"#,
        catalog.page,
        catalog.pages,
        escape_html(&heading),
        catalog.items.len(),
        previous.unwrap_or_default(),
        catalog.page,
        catalog.pages,
        next.unwrap_or_default(),
    )
}

pub(crate) fn render_error(detail: &str, view: &CatalogView) -> String {
    let retry = escape_html(&catalog_local_url(view, 1, true));
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>AniWaves unavailable</title><style>:root{{color-scheme:dark;font-family:system-ui}}body{{min-height:100vh;margin:0;display:grid;place-items:center;padding:1rem;color:var(--rustdl-text,#f7f7f8);background:var(--rustdl-page,#090a0f)}}main{{width:min(100%,560px);padding:1.3rem;border:1px solid var(--rustdl-surface-border,#ffffff20);border-radius:20px;background:var(--rustdl-surface,#11131b)}}h1{{margin:.3rem 0}}p{{color:var(--rustdl-muted,#aeb5c4);line-height:1.5}}a{{display:inline-block;margin-top:.7rem;padding:.7rem .85rem;border-radius:10px;color:var(--rustdl-accent-ink,#07110f);background:var(--rustdl-accent,#70dfc9);text-decoration:none;font-weight:850}}</style></head><body><main><span>Streaming catalog</span><h1>Couldn’t load AniWaves.</h1><p>{}</p><a href="{retry}">Try again</a> <a href="/streaming">Browse newest</a> <a href="/">RustDL home</a></main></body></html>"#,
        escape_html(detail),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
      <div class="film_list grid">
        <div class="item "><div class="inner"><div class="ani poster"><a href="/watch/test-show-123"><img src="https://static.aniwaves.ru/resources/thumbnails/200x280/100/test.jpg" alt="Test"></a></div><div class="info"><div class="b1"><a class="name d-title" href="/watch/test-show-123">Test &amp; Show</a></div><div class="meta"><span class="ep-status sub"><span> 9 </span></span><span class="ep-status dub"><span> 4 </span></span><span class="ep-status total"><span>12</span></span><div class="right">TV</div></div></div></div></div>
        <div class="item "><div class="inner"><div class="ani poster"><a href="/watch/movie-456"><img src="https://static.aniwaves.ru/resources/thumbnails/200x280/100/movie.jpg" alt="Movie"></a></div><div class="info"><div class="b1"><a class="name d-title" href="/watch/movie-456">Movie</a></div><div class="meta"><span class="ep-status sub"><span> 1 </span></span><span class="ep-status total"><span>1</span></span><div class="right">Movie</div></div></div></div></div>
      </div><a href="/newest/page/2">2</a><a href="/newest/page/11">11</a>
    "#;

    #[test]
    fn parses_newest_catalog_without_media_extraction() {
        let catalog = parse_catalog_html(CatalogView::Newest, 1, FIXTURE).unwrap();
        assert_eq!(catalog.pages, 11);
        assert_eq!(catalog.items.len(), 2);
        assert_eq!(catalog.items[0].title, "Test & Show");
        assert_eq!(catalog.items[0].sub.as_deref(), Some("9"));
        assert_eq!(catalog.items[0].dub.as_deref(), Some("4"));
        assert_eq!(
            catalog.items[0].watch_url,
            "https://aniwaves.ru/watch/test-show-123"
        );
        let html = render_catalog(&catalog, &HashSet::new(), "test-token");
        assert!(html.contains("rustdl://stream?url="));
        assert!(!html.contains("download"));
        assert!(html.contains("Search all anime"));
        assert!(html.contains("method=\"get\""));
        assert!(html.contains("section=updated"));
        assert!(html.contains("/streaming/watchlist"));
        assert!(html.contains("/__app/watchlist"));
        assert!(html.contains("form.getAttribute('action')"));
        assert!(!html.contains("fetch(form.action"));
    }

    #[test]
    fn accepts_supported_aniwaves_catalog_and_search_urls() {
        assert_eq!(
            catalog_target_from_text("https://aniwaves.ru/newest"),
            Some(CatalogTarget {
                view: CatalogView::Newest,
                page: 1,
            })
        );
        assert_eq!(
            catalog_target_from_text("open https://aniwaves.ru/updated/page/11"),
            Some(CatalogTarget {
                view: CatalogView::Updated,
                page: 11,
            })
        );
        assert_eq!(
            catalog_target_from_text("https://aniwaves.ru/filter?keyword=one%20piece&page=2"),
            Some(CatalogTarget {
                view: CatalogView::Search("one piece".to_owned()),
                page: 2,
            })
        );
        assert_eq!(catalog_target_from_text("http://aniwaves.ru/newest"), None);
        assert_eq!(catalog_target_from_text("https://evil.test/newest"), None);
        assert_eq!(
            catalog_target_from_text("https://aniwaves.ru/watch/show-1"),
            None
        );
    }

    #[test]
    fn search_is_full_library_paginated_and_safe() {
        assert_eq!(
            CatalogView::from_params(Some("updated"), None),
            Ok(CatalogView::Updated)
        );
        assert_eq!(
            CatalogView::from_params(Some("updated"), Some("  One Piece  ")),
            Ok(CatalogView::Search("One Piece".to_owned()))
        );
        assert!(CatalogView::from_params(Some("unknown"), None).is_err());
        assert!(CatalogView::from_params(None, Some("\u{0000}")).is_err());

        let search_html =
            format!(r#"{FIXTURE}<a href="/filter?keyword=one+piece&amp;page=2">2</a>"#);
        let catalog =
            parse_catalog_html(CatalogView::Search("One Piece".to_owned()), 1, &search_html)
                .unwrap();
        assert_eq!(catalog.pages, 2);
        let html = render_catalog(&catalog, &HashSet::new(), "test-token");
        assert!(html.contains("value=\"One Piece\""));
        assert!(html.contains("q=One+Piece&amp;page=2"));

        let empty = parse_catalog_html(
            CatalogView::Search("Nothing here".to_owned()),
            1,
            "<html></html>",
        )
        .unwrap();
        assert!(empty.items.is_empty());
        assert!(
            render_catalog(&empty, &HashSet::new(), "test-token")
                .contains("No anime matched that search")
        );
    }

    #[test]
    fn parses_episode_and_server_manifests_without_player_page_html() {
        let watch = r#"<meta property="og:title" content="Test &amp; Show"><div id="watch-main" data-id="82697" data-url="/watch/test-show-82697">"#;
        assert_eq!(watch_show_id(watch).as_deref(), Some("82697"));
        assert_eq!(watch_page_title(watch).as_deref(), Some("Test & Show"));

        let episode_html = r#"<li><a data-num="1" data-slug="1" data-timestamp="2026-08-29 15:30:00"><span class="d-title">First &amp; Fast</span></a></li><li><a data-num="2" data-slug="2" data-timestamp=""><span class="d-title">Backup Time</span></a></li>"#;
        assert_eq!(
            parse_stream_episodes(episode_html),
            vec![
                StreamEpisode {
                    number: "1".to_owned(),
                    title: "First & Fast".to_owned(),
                    released_at: Some(1_788_017_400),
                },
                StreamEpisode {
                    number: "2".to_owned(),
                    title: "Backup Time".to_owned(),
                    released_at: None,
                },
            ]
        );
        assert_eq!(parse_utc_timestamp("1970-01-01 00:00:00"), Some(0));
        assert_eq!(
            parse_utc_timestamp("2024-02-29 23:59:59"),
            Some(1_709_251_199)
        );
        assert_eq!(parse_utc_timestamp("2025-02-29 00:00:00"), None);

        let servers = r#"<div class="type" data-type="sub"><ul><li data-link-id="abcDEF123+/=">Vidplay</li><li data-link-id="backup_456-">BYFMS</li></ul></div><div class="type" data-type="dub"><ul><li data-link-id="dub789==">DGHG</li></ul></div>"#;
        assert_eq!(
            parse_stream_server_entries(servers),
            vec![
                (
                    "Vidplay".to_owned(),
                    "sub".to_owned(),
                    "abcDEF123+/=".to_owned()
                ),
                (
                    "BYFMS".to_owned(),
                    "sub".to_owned(),
                    "backup_456-".to_owned()
                ),
                ("DGHG".to_owned(), "dub".to_owned(), "dub789==".to_owned()),
            ]
        );
    }

    #[test]
    fn streaming_manifest_rejects_unsafe_origins_and_player_urls() {
        assert!(validate_watch_page_url("https://aniwaves.ru/watch/test-show-82697").is_ok());
        assert!(validate_watch_page_url("https://evil.test/watch/test-show-82697").is_err());
        assert!(validate_watch_page_url("https://aniwaves.ru/watch/test?token=secret").is_err());
        assert_eq!(
            validate_player_url("https://player.example/e/abc").as_deref(),
            Some("https://player.example/e/abc")
        );
        assert!(validate_player_url("http://player.example/e/abc").is_none());
        assert!(validate_player_url("https://127.0.0.1/e/abc").is_none());
        assert!(validate_player_url("https://user:pass@player.example/e/abc").is_none());
        assert_eq!(
            player_allowed_hosts(
                "https://Embed.Example/e/abc",
                "https://media.example/player/abc"
            ),
            vec!["embed.example".to_owned(), "media.example".to_owned()]
        );
        assert_eq!(
            player_allowed_hosts(
                "https://embed.example/e/abc",
                "https://embed.example/player/abc"
            ),
            vec!["embed.example".to_owned()]
        );
    }

    #[test]
    #[ignore = "requires the live AniWaves catalog and player hosts"]
    fn resolves_multiple_live_backup_sources() {
        let client = Client::builder()
            .user_agent("rustdl-manifest-test")
            .build()
            .expect("build live manifest client");
        let manifest = load_stream_manifest(
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
        let updated =
            load_catalog(&client, CatalogView::Updated, 1, true).expect("load updated catalog");
        assert!(!updated.items.is_empty());
        let search = load_catalog(
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
