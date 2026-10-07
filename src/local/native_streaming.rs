//! Presentation-only streaming data; decoder URLs remain in app-owned state.
use super::aniwaves::StreamManifest;
use serde_json::{Value, json};

pub(crate) fn manifest_page(manifest: &StreamManifest, privacy: bool) -> Value {
    json!({"title":if privacy {"Anime playback"} else {&manifest.title},
        "episode":if privacy {""} else {&manifest.episode},
        "sources":manifest.sources.iter().enumerate().take(64).map(|(index,source)|json!({
            "index":index,"label":if privacy {format!("Server {}",index+1)} else {source.label.clone()},
            "language":if privacy {""} else {&source.language},"available":source.available,
            "issue":source.issue.is_some()
        })).collect::<Vec<_>>()})
}

#[cfg(test)]
#[test]
fn native_streaming_projection_hides_decoder_sources_and_private_metadata() {
    use super::aniwaves::StreamSource;
    assert!(command("invalid", "{}", true).contains("Unsupported streaming action"));
    assert!(
        command("open", r#"{"unknown":"private"}"#, true).contains("Invalid streaming request")
    );
    let manifest = StreamManifest {
        title: "Private synthetic anime".into(),
        poster_url: Some("https://example.invalid/private-poster".into()),
        episode: "private-episode".into(),
        episodes: vec![super::aniwaves::StreamEpisode {
            number: "private-episode".into(),
            title: "Private episode title".into(),
            released_at: None,
        }],
        sources: vec![StreamSource {
            label: "Private server".into(),
            language: "Private language".into(),
            url: "https://example.invalid/private-video".into(),
            available: true,
            redirected: false,
            allowed_hosts: vec!["private-host".into()],
            issue: Some("private-provider-error".into()),
        }],
    };
    let mut page = manifest_page(&manifest, true);
    append_episodes(&mut page, &manifest, 0, true);
    assert_eq!(page["episodeTotal"], 1);
    assert_eq!(page["episodes"][0]["selected"], true);
    let mut beyond = manifest_page(&manifest, true);
    append_episodes(&mut beyond, &manifest, 100, true);
    assert_eq!(beyond["episodes"], json!([]));
    assert_eq!(page["sources"][0]["index"], 0);
    assert_eq!(page["sources"][0]["available"], true);
    assert_eq!(page["sources"][0]["issue"], true);
    let text = page.to_string();
    for hidden in [
        "Private",
        "private-",
        "https://",
        "allowedHosts",
        "posterUrl",
    ] {
        assert!(!text.contains(hidden));
    }
    let mut many = manifest.clone();
    many.sources = vec![many.sources[0].clone(); 65];
    let mut last = manifest_page(&many, true);
    append_sources(&mut last, &many, 64, true);
    assert_eq!(last["sourceTotal"], 65);
    assert_eq!(last["sources"][0]["index"], 64);
    assert_eq!(last["sources"].as_array().unwrap().len(), 1);
    many.sources[64].available = false;
    many.sources[64].language = "dub".into();
    append_filtered_sources(&mut last, &many, 0, true, "issues");
    assert_eq!(last["sourceTotal"], 1);
    assert_eq!(last["sources"][0]["index"], 64);
    append_filtered_sources(&mut last, &many, 0, true, "dub");
    assert_eq!(last["sourceTotal"], 1);
    assert_eq!(last["sources"][0]["language"], "");
    assert!(
        command("page", r#"{"sourceFilter":"invalid"}"#, true).contains("Invalid server filter")
    );
    let public = manifest_page(&manifest, false).to_string();
    assert!(public.contains("Private synthetic anime"));
    assert!(!public.contains("private-video"));
    assert!(!public.contains("private-provider-error"));
}

#[derive(Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
struct Request {
    watch_url: String,
    episode: String,
    token: String,
    refresh: bool,
    offset: usize,
    index: usize,
    source_offset: usize,
    source_filter: String,
}
static SESSIONS: std::sync::Mutex<std::collections::VecDeque<(String, String, StreamManifest)>> =
    std::sync::Mutex::new(std::collections::VecDeque::new());

pub(crate) fn command(action: &str, payload: &str, privacy: bool) -> String {
    let result = (|| -> Result<Value, &'static str> {
        if payload.len() > 8192 {
            return Err("Streaming request is too large");
        }
        let request: Request =
            serde_json::from_str(payload).map_err(|_| "Invalid streaming request")?;
        if request.watch_url.len() > 4096 || request.episode.len() > 64 || request.token.len() > 64
        {
            return Err("Streaming request is too large");
        }
        if !matches!(
            action,
            "open" | "page" | "episode" | "watchlist-add" | "watchlist-remove"
        ) {
            return Err("Unsupported streaming action");
        }
        if request.offset > 100000 || request.index > 100000 || request.source_offset > 100000 {
            return Err("Invalid episode selection");
        }
        let filter = if request.source_filter.is_empty() {
            "all"
        } else {
            request.source_filter.as_str()
        };
        if !matches!(filter, "all" | "sub" | "dub" | "ready" | "issues") {
            return Err("Invalid server filter");
        }
        if matches!(action, "page" | "watchlist-add" | "watchlist-remove") {
            let sessions = SESSIONS.lock().unwrap_or_else(|p| p.into_inner());
            let (_, watch, manifest) = sessions
                .iter()
                .find(|(id, _, _)| id == &request.token)
                .ok_or("Streaming selection expired")?;
            let watch = watch.clone();
            let manifest = manifest.clone();
            drop(sessions);
            let output = super::queue::QUEUE_OUTPUT_DIR
                .get()
                .ok_or("Download engine is starting")?;
            if action == "watchlist-remove" {
                super::streaming_library::remove(output, &watch)
                    .map_err(|_| "Could not update watchlist")?;
            } else if action == "watchlist-add" {
                let input = watchlist_input(&manifest, &watch)?;
                super::streaming_library::add(output, input)
                    .map_err(|_| "Could not update watchlist")?;
            }
            let mut page = manifest_page(&manifest, privacy);
            append_episodes(&mut page, &manifest, request.offset, privacy);
            append_filtered_sources(&mut page, &manifest, request.source_offset, privacy, filter);
            append_watchlist(&mut page, &watch)?;
            page["token"] = request.token.into();
            page["ok"] = true.into();
            return Ok(page);
        }
        let (watch_url, selected_episode) = if action == "episode" {
            let sessions = SESSIONS.lock().unwrap_or_else(|p| p.into_inner());
            let (_, watch, manifest) = sessions
                .iter()
                .find(|(id, _, _)| id == &request.token)
                .ok_or("Streaming selection expired")?;
            let episode = manifest
                .episodes
                .get(request.index)
                .ok_or("Invalid episode selection")?;
            (watch.clone(), episode.number.clone())
        } else {
            (request.watch_url.clone(), request.episode.clone())
        };
        let watch = super::aniwaves::validate_watch_page_url(&watch_url)
            .map_err(|_| "Unsupported anime link")?;
        let client = super::super::external::http::build_client()
            .map_err(|_| "Could not prepare streaming")?;
        let episode = (!selected_episode.is_empty()).then_some(selected_episode.as_str());
        let manifest = super::super::external::aniwaves::load_stream_manifest(
            &client,
            &watch,
            episode,
            request.refresh,
        )
        .map_err(|_| "Could not resolve streaming servers")?;
        let token = super::security::random_token();
        let mut page = manifest_page(&manifest, privacy);
        append_episodes(&mut page, &manifest, 0, privacy);
        append_filtered_sources(&mut page, &manifest, 0, privacy, filter);
        append_watchlist(&mut page, &watch)?;
        let mut sessions = SESSIONS.lock().unwrap_or_else(|p| p.into_inner());
        if sessions.len() >= 8 {
            sessions.pop_front();
        }
        sessions.push_back((token.clone(), watch, manifest));
        page["token"] = token.into();
        page["ok"] = true.into();
        Ok(page)
    })();
    result
        .unwrap_or_else(|detail| json!({"ok":false,"detail":detail}))
        .to_string()
}

fn decoder_source(source: &super::aniwaves::StreamSource) -> Option<Value> {
    let url = reqwest::Url::parse(&source.url).ok()?;
    if url.scheme() != "https"
        || url
            .host_str()
            .is_none_or(|host| host.is_empty() || host.eq_ignore_ascii_case("localhost"))
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    Some(
        json!({"url":source.url,"allowedHosts":source.allowed_hosts,"redirected":source.redirected}),
    )
}
/// Decoder-only bridge. This result must never be sent to a GPUI presentation snapshot.
pub(crate) fn source_for_decoder(token: &str, index: usize) -> String {
    if token.len() > 64 || index > 100000 {
        return "{}".into();
    }
    let source = {
        let sessions = SESSIONS.lock().unwrap_or_else(|p| p.into_inner());
        sessions
            .iter()
            .find(|(id, _, _)| id == token)
            .and_then(|(_, _, manifest)| {
                manifest
                    .sources
                    .get(index)
                    .map(|source| (source.clone(), manifest.episode.clone()))
            })
    };
    source
        .as_ref()
        .and_then(|(source, episode)| {
            let mut data = decoder_source(source)?;
            data["episode"] = episode.clone().into();
            Some(data)
        })
        .unwrap_or_else(|| json!({}))
        .to_string()
}
#[cfg(test)]
#[test]
fn native_streaming_decoder_source_rejects_unsafe_urls() {
    use super::aniwaves::StreamSource;
    let mut source = StreamSource {
        label: String::new(),
        language: String::new(),
        url: String::new(),
        available: true,
        redirected: false,
        allowed_hosts: vec![],
        issue: None,
    };
    for url in [
        "file:///private/video",
        "javascript:alert(1)",
        "http://example.invalid/video",
        "https://localhost/video",
        "https://user:secret@example.invalid/video",
    ] {
        source.url = url.into();
        assert!(decoder_source(&source).is_none());
    }
    source.url = "https://example.invalid/synthetic-player".into();
    assert!(decoder_source(&source).is_some());
    assert_eq!(source_for_decoder("missing-synthetic", 64), "{}");
}

fn append_episodes(page: &mut Value, manifest: &StreamManifest, offset: usize, privacy: bool) {
    let offset = offset.min(manifest.episodes.len());
    page["episodeOffset"] = offset.into();
    page["episodeTotal"] = manifest.episodes.len().into();
    page["episodes"]=manifest.episodes.iter().enumerate().skip(offset).take(25).map(|(index,episode)|json!({"index":index,"number":if privacy {""}else{&episode.number},"title":if privacy {"Episode"}else{&episode.title},"selected":episode.number==manifest.episode})).collect::<Vec<_>>().into();
}

fn append_sources(page: &mut Value, manifest: &StreamManifest, offset: usize, privacy: bool) {
    append_filtered_sources(page, manifest, offset, privacy, "all");
}
fn append_filtered_sources(
    page: &mut Value,
    manifest: &StreamManifest,
    offset: usize,
    privacy: bool,
    filter: &str,
) {
    let sources: Vec<_> = manifest
        .sources
        .iter()
        .enumerate()
        .filter(|(_, source)| match filter {
            "sub" | "dub" => source.language.eq_ignore_ascii_case(filter),
            "ready" => source.available,
            "issues" => !source.available,
            _ => true,
        })
        .collect();
    let offset = offset.min(sources.len());
    page["sourceFilter"] = filter.into();
    page["sourceOffset"] = offset.into();
    page["sourceTotal"] = sources.len().into();
    page["sources"]=sources.into_iter().skip(offset).take(64).map(|(index,source)|json!({"index":index,"label":if privacy{format!("Server {}",index+1)}else{source.label.clone()},"language":if privacy{""}else{&source.language},"available":source.available,"issue":source.issue.is_some()})).collect::<Vec<_>>().into();
}

fn append_watchlist(page: &mut Value, watch: &str) -> Result<(), &'static str> {
    let output = super::queue::QUEUE_OUTPUT_DIR
        .get()
        .ok_or("Download engine is starting")?;
    let library = super::streaming_library::load(output).map_err(|_| "Could not load watchlist")?;
    page["watchlisted"] = super::streaming_library::urls(&library.entries)
        .contains(watch)
        .into();
    Ok(())
}

fn watchlist_input(
    manifest: &StreamManifest,
    watch: &str,
) -> Result<super::streaming_library::WatchlistInput, &'static str> {
    super::streaming_library::WatchlistInput::validated(
        &manifest.title,
        watch,
        manifest.poster_url.as_deref(),
        None,
        None,
        None,
        None,
    )
    .map_err(|_| "Could not save anime selection")
}
#[cfg(test)]
#[test]
fn native_streaming_watchlist_uses_validated_cached_selection() {
    let mut manifest = StreamManifest {
        title: "Synthetic anime".into(),
        poster_url: None,
        episode: "1".into(),
        episodes: vec![],
        sources: vec![],
    };
    let watch = "https://aniwaves.ru/watch/test-show-123";
    let input = watchlist_input(&manifest, watch).unwrap();
    assert_eq!(input.title, "Synthetic anime");
    assert_eq!(input.watch_url, watch);
    assert!(watchlist_input(&manifest, "https://evil.invalid/watch/1").is_err());
    manifest.title.clear();
    assert!(watchlist_input(&manifest, watch).is_err());
    for action in ["watchlist-add", "watchlist-remove"] {
        let result = command(action, r#"{"token":"synthetic-missing-session"}"#, true);
        assert!(result.contains("Streaming selection expired"));
        assert!(!result.contains(watch));
    }
}

/// Cached selection handoff avoids resolving rotating provider addresses twice.
pub(crate) fn manifest_for_decoder(
    token: &str,
    watch: &str,
    episode: Option<&str>,
) -> Option<StreamManifest> {
    if token.is_empty() || token.len() > 64 {
        return None;
    }
    let sessions = SESSIONS.lock().unwrap_or_else(|p| p.into_inner());
    cached_manifest(&sessions, token, watch, episode)
}
fn cached_manifest(
    sessions: &std::collections::VecDeque<(String, String, StreamManifest)>,
    token: &str,
    watch: &str,
    episode: Option<&str>,
) -> Option<StreamManifest> {
    sessions
        .iter()
        .find(|(id, url, manifest)| {
            id == token && url == watch && episode.is_none_or(|value| value == manifest.episode)
        })
        .map(|(_, _, manifest)| manifest.clone())
}
#[cfg(test)]
#[test]
fn native_streaming_cached_handoff_rejects_wrong_selection() {
    let manifest = StreamManifest {
        title: "Synthetic".into(),
        poster_url: None,
        episode: "2".into(),
        episodes: vec![],
        sources: vec![],
    };
    let sessions = std::collections::VecDeque::from([(
        "token".into(),
        "https://example.invalid/watch".into(),
        manifest,
    )]);
    assert!(
        cached_manifest(
            &sessions,
            "token",
            "https://example.invalid/watch",
            Some("2")
        )
        .is_some()
    );
    assert!(
        cached_manifest(
            &sessions,
            "expired",
            "https://example.invalid/watch",
            Some("2")
        )
        .is_none()
    );
    assert!(
        cached_manifest(
            &sessions,
            "token",
            "https://example.invalid/other",
            Some("2")
        )
        .is_none()
    );
    assert!(
        cached_manifest(
            &sessions,
            "token",
            "https://example.invalid/watch",
            Some("3")
        )
        .is_none()
    );
}
