//! AniWaves URL validation, metadata parsing, and catalog rendering.

use super::super::local;
use reqwest::Url;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::error::Error;
use std::net::IpAddr;

pub(in super::super) const ORIGIN: &str = "https://aniwaves.ru";

pub(in super::super) const MAX_CATALOG_BYTES: u64 = 1_500_000;

pub(in super::super) const MAX_WATCH_BYTES: u64 = 1_000_000;

pub(in super::super) const MAX_AJAX_BYTES: u64 = 1_500_000;

pub(in super::super) const MAX_CATALOG_PAGE: u16 = 1000;

pub(in super::super) const PLAYER_USER_AGENT: &str =
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

// Verified AniWaves category routes; accept only these paths from local requests.
pub(in super::super) const CATALOG_CATEGORIES: &[(&str, &str)] = &[
    ("type/tv-series", "TV Series"),
    ("type/movies", "Movies"),
    ("type/ova", "OVAs"),
    ("type/ona", "ONAs"),
    ("type/special", "Specials"),
    ("type/music", "Music Videos"),
    ("genre/action", "Action"),
    ("genre/adventure", "Adventure"),
    ("genre/comedy", "Comedy"),
    ("genre/drama", "Drama"),
    ("genre/fantasy", "Fantasy"),
    ("genre/historical", "Historical"),
    ("genre/horror", "Horror"),
    ("genre/isekai", "Isekai"),
    ("genre/martial-arts", "Martial Arts"),
    ("genre/mecha", "Mecha"),
    ("genre/music", "Music"),
    ("genre/mystery", "Mystery"),
    ("genre/psychological", "Psychological"),
    ("genre/romance", "Romance"),
    ("genre/school", "School"),
    ("genre/sci-fi", "Sci-Fi"),
    ("genre/sports", "Sports"),
    ("genre/supernatural", "Supernatural"),
    ("genre/suspense", "Suspense"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CatalogView {
    Newest,
    Updated,
    Ongoing,
    Added,
    Search(String),
    Category(&'static str),
}

impl CatalogView {
    pub(crate) fn from_params(
        section: Option<&str>,
        query: Option<&str>,
    ) -> Result<Self, &'static str> {
        if let Some(query) = query.map(str::trim).filter(|query| !query.is_empty()) {
            return local::aniwaves::normalize_search_query(query).map(Self::Search);
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
            category => CATALOG_CATEGORIES
                .iter()
                .find(|(slug, _)| *slug == category)
                .map(|(slug, _)| Self::Category(slug))
                .ok_or("unsupported AniWaves catalog section"),
        }
    }

    pub(in super::super) fn section_slug(&self) -> Option<&'static str> {
        match self {
            Self::Newest => Some("newest"),
            Self::Updated => Some("updated"),
            Self::Ongoing => Some("ongoing"),
            Self::Added => Some("added"),
            Self::Search(_) => None,
            Self::Category(slug) => Some(slug),
        }
    }

    pub(in super::super) fn cache_key(&self) -> String {
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

pub(crate) fn catalog_target_from_text(value: &str) -> Option<CatalogTarget> {
    value.split_whitespace().find_map(|candidate| {
        let candidate = candidate.trim_matches(|character: char| {
            matches!(
                character,
                '<' | '>' | '(' | ')' | '[' | ']' | '"' | '\'' | ','
            )
        });
        local::aniwaves::catalog_target_from_url(candidate)
    })
}

pub(in super::super) fn catalog_target_from_url(value: &str) -> Option<CatalogTarget> {
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
            view: CatalogView::Search(local::aniwaves::normalize_search_query(&query).ok()?),
            page: page_from_query,
        });
    }
    if matches!(path, "" | "/home") {
        return (page_from_query == 1).then_some(CatalogTarget {
            view: CatalogView::Newest,
            page: 1,
        });
    }
    for section in ["newest", "updated", "ongoing", "added"]
        .into_iter()
        .chain(CATALOG_CATEGORIES.iter().map(|(slug, _)| *slug))
    {
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

pub(in super::super) fn normalize_search_query(query: &str) -> Result<String, &'static str> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > 100 || query.chars().any(char::is_control) {
        return Err("search must contain 1 to 100 visible characters");
    }
    Ok(query.to_owned())
}

pub(in super::super) fn upstream_catalog_url(
    view: &CatalogView,
    page: u16,
) -> Result<Url, Box<dyn Error>> {
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

pub(in super::super) fn parse_catalog_html(
    view: CatalogView,
    page: u16,
    html: &str,
) -> Result<Catalog, Box<dyn Error>> {
    let items = html
        .split("<div class=\"item ")
        .skip(1)
        .take(60)
        .filter_map(local::aniwaves::parse_catalog_item)
        .collect::<Vec<_>>();
    if items.is_empty() && !matches!(view, CatalogView::Search(_)) {
        return Err("AniWaves returned no streaming catalog entries".into());
    }
    let pages = local::aniwaves::catalog_page_count(html, &view)
        .max(page)
        .clamp(1, MAX_CATALOG_PAGE);
    Ok(Catalog {
        view,
        page,
        pages,
        items,
    })
}

pub(in super::super) fn parse_catalog_item(fragment: &str) -> Option<CatalogItem> {
    let name_marker = fragment.find("class=\"name d-title\"")?;
    let name_tag_start = fragment[..name_marker].rfind("<a ")? + 3;
    let name_tag_end = fragment[name_marker..].find('>')? + name_marker;
    let name_tag = &fragment[name_tag_start..name_tag_end];
    let href = local::html::html_attribute(name_tag, "href")?;
    let watch_url = local::aniwaves::validated_watch_url(href)?;
    let title_end = fragment[name_tag_end + 1..].find('<')? + name_tag_end + 1;
    let title = local::html::html_entity_decode(fragment[name_tag_end + 1..title_end].trim());
    if title.is_empty() || title.len() > 300 {
        return None;
    }

    let image_start = fragment.find("<img ")? + 5;
    let image_end = fragment[image_start..].find('>')? + image_start;
    let poster_url = local::aniwaves::validated_poster_url(local::html::html_attribute(
        &fragment[image_start..image_end],
        "src",
    )?)?;

    Some(CatalogItem {
        title,
        watch_url,
        poster_url,
        sub: local::aniwaves::status_value(fragment, "sub"),
        dub: local::aniwaves::status_value(fragment, "dub"),
        total: local::aniwaves::status_value(fragment, "total"),
        media_type: local::aniwaves::div_text(fragment, "right"),
    })
}

pub(in super::super) fn validated_watch_url(href: &str) -> Option<String> {
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

pub(in super::super) fn status_value(fragment: &str, class_name: &str) -> Option<String> {
    let marker = format!("class=\"ep-status {class_name}\"");
    let remainder = &fragment[fragment.find(&marker)? + marker.len()..];
    let start = remainder.find("<span>")? + "<span>".len();
    let end = remainder[start..].find('<')? + start;
    let value = remainder[start..end].trim();
    (!value.is_empty() && value.len() <= 8).then(|| local::html::html_entity_decode(value))
}

pub(in super::super) fn div_text(fragment: &str, class_name: &str) -> Option<String> {
    let marker = format!("<div class=\"{class_name}\">");
    let remainder = &fragment[fragment.find(&marker)? + marker.len()..];
    let end = remainder.find('<')?;
    let value = remainder[..end].trim();
    (!value.is_empty() && value.len() <= 24).then(|| local::html::html_entity_decode(value))
}

pub(in super::super) fn catalog_page_count(html: &str, view: &CatalogView) -> u16 {
    match view {
        CatalogView::Search(_) => ["&page=", "&amp;page="]
            .into_iter()
            .map(|marker| local::aniwaves::maximum_page_after(html, marker))
            .max()
            .unwrap_or(1),
        _ => local::aniwaves::maximum_page_after(
            html,
            &format!(
                "/{}/page/",
                view.section_slug().expect("browse sections have slugs")
            ),
        ),
    }
}

pub(in super::super) fn maximum_page_after(html: &str, marker: &str) -> u16 {
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

pub(in super::super) fn ajax_result_html(value: &Value) -> Result<&str, Box<dyn Error>> {
    value
        .get("result")
        .and_then(Value::as_str)
        .ok_or_else(|| "AniWaves returned an incomplete streaming response".into())
}

pub(in super::super) fn watch_show_id(html: &str) -> Option<String> {
    let marker = html.find("id=\"watch-main\"")?;
    let start = html[..marker].rfind('<')? + 1;
    let end = html[marker..].find('>')? + marker;
    let value = local::html::html_attribute(&html[start..end], "data-id")?;
    (value.len() <= 12 && value.bytes().all(|byte| byte.is_ascii_digit())).then(|| value.to_owned())
}

pub(in super::super) fn watch_page_title(html: &str) -> Option<String> {
    let mut remainder = html;
    while let Some(index) = remainder.find("<meta ") {
        remainder = &remainder[index + 6..];
        let end = remainder.find('>')?;
        let tag = &remainder[..end];
        if local::html::html_attribute(tag, "property").is_some_and(|value| value == "og:title") {
            let title =
                local::html::html_entity_decode(local::html::html_attribute(tag, "content")?)
                    .trim()
                    .to_owned();
            return (!title.is_empty() && title.len() <= 300).then_some(title);
        }
        remainder = &remainder[end + 1..];
    }
    None
}

pub(in super::super) fn watch_page_poster(html: &str) -> Option<String> {
    let mut remainder = html;
    while let Some(index) = remainder.find("<meta ") {
        remainder = &remainder[index + 6..];
        let end = remainder.find('>')?;
        let tag = &remainder[..end];
        if local::html::html_attribute(tag, "property").is_some_and(|value| value == "og:image") {
            return local::aniwaves::validated_poster_url(local::html::html_attribute(
                tag, "content",
            )?);
        }
        remainder = &remainder[end + 1..];
    }
    None
}

pub(in super::super) fn parse_stream_episodes(html: &str) -> Vec<StreamEpisode> {
    let mut episodes = Vec::new();
    let mut seen = HashSet::new();
    for fragment in html.split("<a ").skip(1).take(2_000) {
        let Some(tag_end) = fragment.find('>') else {
            continue;
        };
        let tag = &fragment[..tag_end];
        let Some(number) = local::html::html_attribute(tag, "data-num") else {
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
        let title = local::aniwaves::stream_episode_title(&fragment[tag_end + 1..], number);
        let released_at = local::html::html_attribute(tag, "data-timestamp")
            .and_then(local::aniwaves::parse_utc_timestamp);
        episodes.push(StreamEpisode {
            number: number.to_owned(),
            title,
            released_at,
        });
    }
    episodes
}

pub(in super::super) fn parse_utc_timestamp(value: &str) -> Option<u64> {
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

pub(in super::super) fn stream_episode_title(fragment: &str, number: &str) -> String {
    let title = fragment
        .find("class=\"d-title\"")
        .and_then(|marker| fragment[marker..].find('>').map(|index| marker + index + 1))
        .and_then(|start| {
            fragment[start..]
                .find('<')
                .map(|end| local::html::html_entity_decode(fragment[start..start + end].trim()))
        })
        .filter(|title| !title.is_empty() && title.len() <= 300);
    title.unwrap_or_else(|| format!("Episode {number}"))
}

pub(in super::super) fn parse_stream_server_entries(html: &str) -> Vec<(String, String, String)> {
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
            let Some(link_id) = local::html::html_attribute(&item[..tag_end], "data-link-id")
            else {
                continue;
            };
            let label_end = item[tag_end + 1..].find('<').unwrap_or(0);
            let label =
                local::html::html_entity_decode(item[tag_end + 1..tag_end + 1 + label_end].trim());
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

pub(in super::super) fn validate_player_url(value: &str) -> Option<String> {
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
pub(in super::super) struct PlayerProbe {
    pub(in super::super) url: String,
    pub(in super::super) available: bool,
    pub(in super::super) redirected: bool,
    pub(in super::super) allowed_hosts: Vec<String>,
    pub(in super::super) issue: Option<String>,
}

pub(in super::super) fn player_url_host(value: &str) -> Option<String> {
    local::aniwaves::validate_player_url(value).and_then(|url| {
        Url::parse(&url)
            .ok()?
            .host_str()
            .map(|host| host.to_ascii_lowercase())
    })
}

pub(in super::super) fn player_allowed_hosts(original_url: &str, final_url: &str) -> Vec<String> {
    let mut hosts = Vec::with_capacity(2);
    for url in [original_url, final_url] {
        if let Some(host) = local::aniwaves::player_url_host(url)
            && !hosts.contains(&host)
        {
            hosts.push(host);
        }
    }
    hosts
}

pub(in super::super) fn source_priority(label: &str) -> u8 {
    match label.to_ascii_lowercase().as_str() {
        "byfms" => 0,
        "dghg" => 1,
        "datsav" => 2,
        "mycloud" => 3,
        "vidplay" => 4,
        _ => 5,
    }
}

pub(in super::super) fn catalog_local_url(view: &CatalogView, page: u16, refresh: bool) -> String {
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

pub(in super::super) fn render_catalog_tabs(active: &CatalogView) -> String {
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
            local::html::escape_html(&local::aniwaves::catalog_local_url(&view, 1, false)),
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

fn render_catalog_categories(active: &CatalogView) -> String {
    let mut options = String::from(r#"<option value="newest">All categories</option>"#);
    for (prefix, label) in [("type/", "Types"), ("genre/", "Genres")] {
        options.push_str(&format!(r#"<optgroup label="{label}">"#));
        for (slug, label) in CATALOG_CATEGORIES
            .iter()
            .filter(|(slug, _)| slug.starts_with(prefix))
        {
            let selected = if active.section_slug() == Some(*slug) {
                " selected"
            } else {
                ""
            };
            options.push_str(&format!(
                r#"<option value="{slug}"{selected}>{label}</option>"#
            ));
        }
        options.push_str("</optgroup>");
    }
    format!(
        r#"<form class="stream-categories" action="/streaming" method="get"><label for="stream-category">Categories</label><select id="stream-category" name="section">{options}</select><button type="submit">Browse</button></form>"#
    )
}

pub(in super::super) fn catalog_heading(view: &CatalogView) -> (String, &'static str) {
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
        CatalogView::Category(slug) => (
            format!(
                "{} anime.",
                CATALOG_CATEGORIES
                    .iter()
                    .find(|(value, _)| value == slug)
                    .map(|(_, label)| *label)
                    .unwrap_or("Category")
            ),
            "Browse this category across the AniWaves library.",
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
    let return_url = local::aniwaves::catalog_local_url(&catalog.view, catalog.page, false);
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
                    badges.push(format!(
                        "<span>SUB {}</span>",
                        local::html::escape_html(value)
                    ));
                }
                if let Some(value) = &item.dub {
                    badges.push(format!(
                        "<span>DUB {}</span>",
                        local::html::escape_html(value)
                    ));
                }
                if let Some(value) = &item.total {
                    badges.push(format!(
                        "<span>{} EPS</span>",
                        local::html::escape_html(value)
                    ));
                }
                let media_type = item
                    .media_type
                    .as_deref()
                    .map(local::html::escape_html)
                    .unwrap_or_else(|| "Series".to_owned());
                let saved = watchlisted.contains(&item.watch_url);
                {
                    let launch_href = &(local::html::escape_html(launch_url.as_str()));
                    let poster_src = &(local::html::escape_html(&item.poster_url));
                    let display_title = &(local::html::escape_html(&item.title));
                    let badges_html = &(badges.join(""));
                    let action_token_value = &(local::html::escape_html(action_token));
                    let watchlist_action = &(if saved { "remove" } else { "add" });
                    let watch_url = &(local::html::escape_html(&item.watch_url));
                    let saved_title = &(local::html::escape_html(&item.title));
                    let saved_poster = &(local::html::escape_html(&item.poster_url));
                    let sub_count = &(local::html::escape_html(item.sub.as_deref().unwrap_or("")));
                    let dub_count = &(local::html::escape_html(item.dub.as_deref().unwrap_or("")));
                    let episode_count =
                        &(local::html::escape_html(item.total.as_deref().unwrap_or("")));
                    let saved_media_type =
                        &(local::html::escape_html(item.media_type.as_deref().unwrap_or("")));
                    let return_path = &(local::html::escape_html(&return_url));
                    let saved_class = &(if saved { "saved" } else { "" });
                    let action_label = &(if saved { "Remove" } else { "Add" });
                    let action_title = &(local::html::escape_html(&item.title));
                    let action_icon = &(if saved { "✓" } else { "＋" });
                    format!(
                        include_str!("../../assets/html/streaming-card.html"),
                        launch_href = launch_href,
                        poster_src = poster_src,
                        dub_count = dub_count,
                        episode_count = episode_count,
                        saved_media_type = saved_media_type,
                        return_path = return_path,
                        saved_class = saved_class,
                        action_label = action_label,
                        action_title = action_title,
                        action_icon = action_icon,
                        display_title = display_title,
                        badges_html = badges_html,
                        action_token_value = action_token_value,
                        watchlist_action = watchlist_action,
                        watch_url = watch_url,
                        saved_title = saved_title,
                        saved_poster = saved_poster,
                        sub_count = sub_count,
                        media_type = media_type
                    )
                }
            })
            .collect::<String>()
    };
    let refresh = local::html::escape_html(&local::aniwaves::catalog_local_url(
        &catalog.view,
        catalog.page,
        true,
    ));
    let previous = (catalog.page > 1).then(|| {
        format!(
            r#"<a href="{}">← Previous</a>"#,
            local::html::escape_html(&local::aniwaves::catalog_local_url(
                &catalog.view,
                catalog.page - 1,
                false
            ))
        )
    });
    let next = (catalog.page < catalog.pages).then(|| {
        format!(
            r#"<a href="{}">Next →</a>"#,
            local::html::escape_html(&local::aniwaves::catalog_local_url(
                &catalog.view,
                catalog.page + 1,
                false
            ))
        )
    });
    let search_value = match &catalog.view {
        CatalogView::Search(query) => local::html::escape_html(query),
        _ => String::new(),
    };
    let section_value = catalog.view.section_slug().unwrap_or("newest");
    let tabs = local::aniwaves::render_catalog_tabs(&catalog.view);
    let (heading, description) = local::aniwaves::catalog_heading(&catalog.view);
    let eyebrow = if matches!(catalog.view, CatalogView::Search(_)) {
        "Library search"
    } else {
        "Browse anime"
    };
    {
        let page_number = &(catalog.page);
        let page_count = &(catalog.pages);
        let title_text = &(local::html::escape_html(&heading));
        let item_count = &(catalog.items.len());
        let previous_link = &(previous.unwrap_or_default());
        let current_page = &(catalog.page);
        let total_pages = &(catalog.pages);
        let next_link = &(next.unwrap_or_default());
        format!(
            include_str!("../../assets/html/streaming-catalog.html"),
            page_number = page_number,
            page_count = page_count,
            title_text = title_text,
            item_count = item_count,
            previous_link = previous_link,
            current_page = current_page,
            total_pages = total_pages,
            next_link = next_link,
            page_css = include_str!("../../assets/css/streaming-catalog.css"),
            page_script = include_str!("../../assets/js/streaming-catalog.js"),
            cards = cards,
            description = description,
            eyebrow = eyebrow,
            refresh = refresh,
            search_value = search_value,
            section_value = section_value,
            tabs = tabs,
            categories = render_catalog_categories(&catalog.view)
        )
    }
}

pub(crate) fn render_error(detail: &str, view: &CatalogView) -> String {
    let retry = local::html::escape_html(&local::aniwaves::catalog_local_url(view, 1, true));
    {
        let detail_text = &(local::html::escape_html(detail));
        format!(
            include_str!("../../assets/html/streaming-error.html"),
            detail_text = detail_text,
            page_css = include_str!("../../assets/css/streaming-error.css"),
            retry = retry
        )
    }
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
        let catalog = local::aniwaves::parse_catalog_html(CatalogView::Newest, 1, FIXTURE).unwrap();
        assert_eq!(catalog.pages, 11);
        assert_eq!(catalog.items.len(), 2);
        assert_eq!(catalog.items[0].title, "Test & Show");
        assert_eq!(catalog.items[0].sub.as_deref(), Some("9"));
        assert_eq!(catalog.items[0].dub.as_deref(), Some("4"));
        assert_eq!(
            catalog.items[0].watch_url,
            "https://aniwaves.ru/watch/test-show-123"
        );
        let html = local::aniwaves::render_catalog(&catalog, &HashSet::new(), "test-token");
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
    fn categories_preserve_routes_selection_and_pagination() {
        for (slug, label) in CATALOG_CATEGORIES {
            let view = CatalogView::from_params(Some(slug), None).unwrap();
            let upstream = upstream_catalog_url(&view, 155).unwrap();
            assert_eq!(upstream.path(), format!("/{slug}/page/155"));
            assert_eq!(
                catalog_target_from_url(upstream.as_str()),
                Some(CatalogTarget {
                    view: view.clone(),
                    page: 155,
                })
            );
            let url = Url::parse(&format!(
                "http://rustdl.local{}",
                catalog_local_url(&view, 155, true)
            ))
            .unwrap();
            let params = url
                .query_pairs()
                .collect::<std::collections::HashMap<_, _>>();
            assert_eq!(params.get("section").map(|v| v.as_ref()), Some(*slug));
            assert_eq!(params.get("page").map(|v| v.as_ref()), Some("155"));
            assert_eq!(params.get("refresh").map(|v| v.as_ref()), Some("1"));
            let fixture = format!(r#"{FIXTURE}<a href="/{slug}/page/155">Last</a>"#);
            let catalog = parse_catalog_html(view, 2, &fixture).unwrap();
            assert_eq!(catalog.pages, 155);
            let html = render_catalog(&catalog, &HashSet::new(), "test-token");
            assert!(html.contains(&format!(r#"value="{slug}" selected>{label}</option>"#)));
            assert!(html.contains(&format!("{label} anime.")));
            assert!(html.contains(&local::html::escape_html(&catalog_local_url(
                &catalog.view,
                3,
                false
            ))));
        }
        assert_ne!(
            CatalogView::Category("type/music").cache_key(),
            CatalogView::Category("genre/music").cache_key()
        );
        for invalid in [
            "genre/unknown",
            "genre/../watch",
            "https://other.test",
            "type/movies?page=2",
        ] {
            assert!(CatalogView::from_params(Some(invalid), None).is_err());
        }
        assert!(catalog_target_from_url("https://aniwaves.ru/genre/action/page/1001").is_none());
    }

    #[test]
    fn accepts_supported_aniwaves_catalog_and_search_urls() {
        assert_eq!(
            local::aniwaves::catalog_target_from_text("https://aniwaves.ru/newest"),
            Some(CatalogTarget {
                view: CatalogView::Newest,
                page: 1,
            })
        );
        assert_eq!(
            local::aniwaves::catalog_target_from_text("open https://aniwaves.ru/updated/page/11"),
            Some(CatalogTarget {
                view: CatalogView::Updated,
                page: 11,
            })
        );
        assert_eq!(
            local::aniwaves::catalog_target_from_text(
                "https://aniwaves.ru/filter?keyword=one%20piece&page=2"
            ),
            Some(CatalogTarget {
                view: CatalogView::Search("one piece".to_owned()),
                page: 2,
            })
        );
        assert_eq!(
            local::aniwaves::catalog_target_from_text("http://aniwaves.ru/newest"),
            None
        );
        assert_eq!(
            local::aniwaves::catalog_target_from_text("https://evil.test/newest"),
            None
        );
        assert_eq!(
            local::aniwaves::catalog_target_from_text("https://aniwaves.ru/watch/show-1"),
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
        let catalog = local::aniwaves::parse_catalog_html(
            CatalogView::Search("One Piece".to_owned()),
            1,
            &search_html,
        )
        .unwrap();
        assert_eq!(catalog.pages, 2);
        let html = local::aniwaves::render_catalog(&catalog, &HashSet::new(), "test-token");
        assert!(html.contains("value=\"One Piece\""));
        assert!(html.contains("q=One+Piece&amp;page=2"));

        let empty = local::aniwaves::parse_catalog_html(
            CatalogView::Search("Nothing here".to_owned()),
            1,
            "<html></html>",
        )
        .unwrap();
        assert!(empty.items.is_empty());
        assert!(
            local::aniwaves::render_catalog(&empty, &HashSet::new(), "test-token")
                .contains("No anime matched that search")
        );
    }

    #[test]
    fn parses_episode_and_server_manifests_without_player_page_html() {
        let watch = r#"<meta property="og:title" content="Test &amp; Show"><div id="watch-main" data-id="82697" data-url="/watch/test-show-82697">"#;
        assert_eq!(
            local::aniwaves::watch_show_id(watch).as_deref(),
            Some("82697")
        );
        assert_eq!(
            local::aniwaves::watch_page_title(watch).as_deref(),
            Some("Test & Show")
        );

        let episode_html = r#"<li><a data-num="1" data-slug="1" data-timestamp="2026-08-29 15:30:00"><span class="d-title">First &amp; Fast</span></a></li><li><a data-num="2" data-slug="2" data-timestamp=""><span class="d-title">Backup Time</span></a></li>"#;
        assert_eq!(
            local::aniwaves::parse_stream_episodes(episode_html),
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
        assert_eq!(
            local::aniwaves::parse_utc_timestamp("1970-01-01 00:00:00"),
            Some(0)
        );
        assert_eq!(
            local::aniwaves::parse_utc_timestamp("2024-02-29 23:59:59"),
            Some(1_709_251_199)
        );
        assert_eq!(
            local::aniwaves::parse_utc_timestamp("2025-02-29 00:00:00"),
            None
        );

        let servers = r#"<div class="type" data-type="sub"><ul><li data-link-id="abcDEF123+/=">Vidplay</li><li data-link-id="backup_456-">BYFMS</li></ul></div><div class="type" data-type="dub"><ul><li data-link-id="dub789==">DGHG</li></ul></div>"#;
        assert_eq!(
            local::aniwaves::parse_stream_server_entries(servers),
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
        assert!(
            local::aniwaves::validate_watch_page_url("https://aniwaves.ru/watch/test-show-82697")
                .is_ok()
        );
        assert!(
            local::aniwaves::validate_watch_page_url("https://evil.test/watch/test-show-82697")
                .is_err()
        );
        assert!(
            local::aniwaves::validate_watch_page_url("https://aniwaves.ru/watch/test?token=secret")
                .is_err()
        );
        assert_eq!(
            local::aniwaves::validate_player_url("https://player.example/e/abc").as_deref(),
            Some("https://player.example/e/abc")
        );
        assert!(local::aniwaves::validate_player_url("http://player.example/e/abc").is_none());
        assert!(local::aniwaves::validate_player_url("https://127.0.0.1/e/abc").is_none());
        assert!(
            local::aniwaves::validate_player_url("https://user:pass@player.example/e/abc")
                .is_none()
        );
        assert_eq!(
            local::aniwaves::player_allowed_hosts(
                "https://Embed.Example/e/abc",
                "https://media.example/player/abc"
            ),
            vec!["embed.example".to_owned(), "media.example".to_owned()]
        );
        assert_eq!(
            local::aniwaves::player_allowed_hosts(
                "https://embed.example/e/abc",
                "https://embed.example/player/abc"
            ),
            vec!["embed.example".to_owned()]
        );
    }
}
