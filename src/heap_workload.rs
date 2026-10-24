//! Offline workloads, with opt-in captured playlist HTML fixtures. No app
//! stores, media playback, server, or network requests.
use super::{local, workflows};
use std::sync::{Arc, Barrier, Mutex};

pub(crate) struct PlaylistPreviewFixture(local::discovery::PlaylistSession);

pub(crate) struct PlaylistFixture(Vec<local::playlist_resolution::Selection>);

pub(crate) struct GalleryFixture {
    path: std::path::PathBuf,
    count: usize,
}
impl GalleryFixture {
    fn new(count: usize) -> Result<Self, &'static str> {
        use std::os::unix::fs::DirBuilderExt;
        let base = std::path::Path::new("target/page-bench-fixtures");
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(base)
            .map_err(|_| "cannot create fixture parent")?;
        let path = base.join(format!(
            "gallery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|_| "cannot create private gallery fixture")?;
        let fixture = Self { path, count };
        for index in 0..count {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(fixture.path.join(format!("youtube-bench{index:06}.mp4")))
                .map_err(|_| "cannot create synthetic file")?;
        }
        Ok(fixture)
    }
}
impl Drop for GalleryFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

pub(crate) enum Workload {
    Activity,
    ActivityPage,
    SettingsPage,
    ChangelogPage,
    Gallery(GalleryFixture),
    QueuePage(usize),
    PlaylistPage(String, bool),
    PlaylistPreview(PlaylistPreviewFixture),
    Changelog,
    Catalog(String),
    Queue(usize),
    Quality(Vec<String>),
    Playlist(PlaylistFixture, bool),
}

impl Workload {
    pub(crate) fn new(name: &str) -> Result<Self, &'static str> {
        match name {
            "playlist-preview-100" | "playlist-preview-1000" | "playlist-preview-5000" => {
                let count: usize = name.rsplit('-').next().unwrap().parse().unwrap();
                Ok(Self::PlaylistPreview(PlaylistPreviewFixture(
                    local::discovery::PlaylistSession {
                        created: 0,
                        playlist_id: "PLsynthetic".to_owned(),
                        title: "Synthetic preview".to_owned(),
                        entries: (0..count)
                            .map(|i| local::youtube::YouTubePlaylistEntry {
                                video_id: format!("entry{i:06}"),
                                title: format!("Synthetic {i} </script>& 雪 {}", "x".repeat(96)),
                                author: format!("Creator {i}"),
                            })
                            .collect(),
                    },
                )))
            }
            "activity" => Ok(Self::Activity),
            "page-activity" => Ok(Self::ActivityPage),
            "page-settings" => Ok(Self::SettingsPage),
            "page-changelog" => Ok(Self::ChangelogPage),
            "gallery-500" | "gallery-5000" => Ok(Self::Gallery(GalleryFixture::new(
                if name.ends_with("5000") { 5000 } else { 500 },
            )?)),
            "playlist-page" | "playlist-page-baseline" => {
                let path = std::env::var("RUSTDL_PLAYLIST_HTML")
                    .map_err(|_| "set RUSTDL_PLAYLIST_HTML to a captured playlist page")?;
                if std::fs::metadata(&path)
                    .map_err(|_| "cannot stat playlist fixture")?
                    .len()
                    > 6 * 1024 * 1024
                {
                    return Err("playlist fixture exceeds 6 MiB");
                }
                let html =
                    std::fs::read_to_string(path).map_err(|_| "cannot read playlist fixture")?;
                Ok(Self::PlaylistPage(html, name.ends_with("baseline")))
            }
            "quality-500" => {
                use local::models::{DiscoveryCandidate, ResolvedVideo};
                let token = "synthetic-quality-session";
                let mut sessions = local::discovery::DISCOVERY_SESSIONS
                    .get_or_init(|| Mutex::new(Default::default()))
                    .lock()
                    .unwrap();
                if !sessions.is_empty() {
                    return Err("synthetic quality selection requires a fresh process");
                }
                let candidates = (0..500)
                    .map(|index| {
                        let qualities: Vec<_> = (0..12)
                            .map(|quality| {
                                let audio = quality == 11;
                                ResolvedVideo {
                                    filename: format!(
                                        "synthetic-{index}.{}",
                                        if audio { "m4a" } else { "mp4" }
                                    ),
                                    media_url: format!(
                                        "https://example.invalid/{index}/{quality}?synthetic={}",
                                        "x".repeat(2048)
                                    ),
                                    audio_url: (!audio).then(|| {
                                        format!(
                                            "https://example.invalid/audio/{index}?synthetic={}",
                                            "y".repeat(1024)
                                        )
                                    }),
                                    extract_audio: audio,
                                    quality_label: Some(if audio {
                                        "Audio only".to_owned()
                                    } else {
                                        format!("{}p", (quality + 1) * 144)
                                    }),
                                    quality_height: (!audio).then_some((quality + 1) * 144),
                                }
                            })
                            .collect();
                        DiscoveryCandidate {
                            resolved: qualities[0].clone(),
                            qualities,
                            source_url: format!("https://example.invalid/source/{index}"),
                            author: format!("Synthetic creator {index}"),
                            text: format!("Synthetic item {index}"),
                            playlist: None,
                        }
                    })
                    .collect();
                sessions.insert(
                    token.to_owned(),
                    Arc::new(local::discovery::DiscoverySession {
                        created: 0,
                        candidates,
                    }),
                );
                Ok(Self::Quality(
                    (0..500).map(|index| format!("{token}:{index}")).collect(),
                ))
            }
            "changelog" => Ok(Self::Changelog),
            "queue-500" | "queue-5000" | "queue-page-500" | "queue-page-5000" => {
                let count = if name.ends_with("5000") { 5000 } else { 500 };
                let mut jobs = local::queue::download_jobs().lock().unwrap();
                if !jobs.is_empty() {
                    return Err("synthetic queue requires a fresh process");
                }
                for i in 0..count {
                    use local::queue::{DownloadJob, DownloadPhase::*};
                    jobs.insert(
                        format!("{i}-1.mp4"),
                        DownloadJob {
                            phase: [
                                Queued,
                                Starting,
                                Downloading,
                                Paused,
                                Ready,
                                Failed,
                                Cancelled,
                            ][i % 7],
                            downloaded: 1024 * i as u64,
                            total: Some(8_000_000),
                            error: (i % 7 == 5).then(|| "Synthetic failure".to_owned()),
                            source_url: Some(format!("https://example.invalid/synthetic/{i}")),
                            media_url: None,
                            audio_url: None,
                            extract_audio: false,
                            quality_label: Some("720p".to_owned()),
                            quality_height: Some(720),
                        },
                    );
                }
                drop(jobs);
                // Exclude lazy global setup and pre-existing fixture storage.
                drop(local::activity_state::snapshot());
                Ok(if name.starts_with("queue-page-") {
                    Self::QueuePage(count)
                } else {
                    Self::Queue(count)
                })
            }
            "playlist-500" | "playlist-cancel-500" => {
                local::discovery::DISCOVERY_SESSIONS
                    .get_or_init(|| Mutex::new(Default::default()))
                    .lock()
                    .unwrap()
                    .reserve(8);
                let selections = (0..500)
                    .map(|i| {
                        (
                            local::youtube::YouTubePlaylistEntry {
                                video_id: format!("worker{i:05}"),
                                title: format!("Synthetic item {i}"),
                                author: "Synthetic".to_owned(),
                            },
                            local::gallery::PlaylistMembership {
                                playlist_id: "PLsynthetic00001".to_owned(),
                                title: "Synthetic playlist".to_owned(),
                                position: i + 1,
                                total: 500,
                            },
                        )
                    })
                    .collect();
                Ok(Self::Playlist(
                    PlaylistFixture(selections),
                    name == "playlist-cancel-500",
                ))
            }
            "catalog" => {
                let html = (0..60)
                    .map(|i| {
                        format!(
                            r#"<div class="item synthetic"><img src="https://static.aniwaves.ru/resources/thumbnails/synthetic-{i}.jpg"><a href="/watch/synthetic-{i}" class="name d-title">Synthetic &amp; Entry {i}</a><div class="ep-status sub"><span>12</span></div></div>"#
                        )
                    })
                    .collect();
                Ok(Self::Catalog(html))
            }
            _ => Err(
                "unknown workload; see PERFORMANCE.md for page, gallery, queue, quality, catalog and playlist cases",
            ),
        }
    }

    pub(crate) fn run(&self, iterations: usize) -> Result<usize, &'static str> {
        let mut checksum = 0usize;
        for _ in 0..iterations {
            let size = match self {
                Self::PlaylistPreview(PlaylistPreviewFixture(session)) => {
                    let body = local::discovery::render_playlist_selection("synthetic", session)
                        .map_err(|_| "preview render failed")?;
                    let marker = "<script id=\"playlist-data\" type=\"application/json\">";
                    let data = body
                        .split_once(marker)
                        .ok_or("missing preview data")?
                        .1
                        .split_once("</script>")
                        .ok_or("unterminated preview data")?
                        .0;
                    // Validate without allocating a second decoded playlist during profiling.
                    if data.matches("\"video_id\":").count() != session.entries.len()
                        || data.contains('<')
                        || data.contains('>')
                        || data.contains('&')
                    {
                        return Err("preview data lost entries or escaping");
                    }
                    let body = local::web_assets::decorate_app_html(body);
                    u64::from_le_bytes(
                        blake3::hash(body.as_bytes()).as_bytes()[..8]
                            .try_into()
                            .unwrap(),
                    ) as usize
                }
                Self::PlaylistPage(html, baseline) => {
                    let json = local::youtube::initial_data(html)
                        .map_err(|_| "initial data extraction failed")?;
                    let (entries, next) = if *baseline {
                        let data: serde_json::Value = serde_json::from_str(&json)
                            .map_err(|_| "baseline JSON parsing failed")?;
                        local::youtube::initial_playlist_entries(&data)
                            .map_err(|_| "baseline playlist extraction failed")?
                    } else {
                        let page = local::youtube_playlist::initial(&json)
                            .map_err(|_| "streaming playlist extraction failed")?;
                        (page.entries, page.continuation)
                    };
                    if entries.is_empty() {
                        return Err("playlist fixture contained no entries");
                    }
                    let mut hash = blake3::Hasher::new();
                    for entry in &entries {
                        for value in [&entry.video_id, &entry.title, &entry.author] {
                            hash.update(value.as_bytes());
                            hash.update(&[0]);
                        }
                    }
                    if let Some(next) = next {
                        hash.update(next.as_bytes());
                    }
                    u64::from_le_bytes(hash.finalize().as_bytes()[..8].try_into().unwrap()) as usize
                }
                Self::ActivityPage | Self::SettingsPage | Self::ChangelogPage => {
                    let body = match self {
                        Self::ActivityPage => local::activity::render(""),
                        Self::SettingsPage => local::settings::render(""),
                        _ => local::pages::changelog::render(),
                    };
                    hash_page(local::web_assets::decorate_app_html(body))
                }
                Self::Gallery(fixture) => {
                    let body = local::pages::gallery::render(&fixture.path)
                        .map_err(|_| "synthetic gallery rendering failed")?;
                    if local::gallery::GALLERY_LAST_ITEMS.load(std::sync::atomic::Ordering::Relaxed)
                        as usize
                        != fixture.count
                    {
                        return Err("gallery fixture lost items");
                    }
                    hash_page(local::web_assets::decorate_app_html(body))
                }
                Self::QueuePage(count) => {
                    let body = local::queue::render_queue_page(&[]);
                    if body.matches("<article data-filename=").count() != *count {
                        return Err("queue page lost rows");
                    }
                    hash_page(local::web_assets::decorate_app_html(body))
                }
                Self::Activity => std::hint::black_box(local::activity::render("")).len(),
                Self::Changelog => std::hint::black_box(local::pages::changelog::render()).len(),
                Self::Queue(count) => {
                    let snapshot = local::activity_state::snapshot();
                    let counts = &snapshot["counts"];
                    let total = counts["active"].as_u64().unwrap_or(0)
                        + counts["issues"].as_u64().unwrap_or(0)
                        + counts["completed"].as_u64().unwrap_or(0);
                    if total as usize != *count
                        || snapshot["downloads"].as_array().map(Vec::len) != Some(60)
                    {
                        return Err("queue snapshot lost counts or exceeded its visible-row bound");
                    }
                    std::hint::black_box(snapshot.to_string()).len()
                }
                Self::Quality(picks) => {
                    let body = local::discovery::render_quality_selection(picks)
                        .ok_or("synthetic quality selection was lost")?;
                    if body.matches("class=\"quality-card\"").count() != 500
                        || body
                            .matches("<option value=\"synthetic-quality-session:")
                            .count()
                            != 6000
                        || !body.contains("Add 500 to queue")
                    {
                        return Err("quality page lost candidates or formats");
                    }
                    let body = local::web_assets::decorate_app_html(body);
                    u64::from_le_bytes(
                        blake3::hash(body.as_bytes()).as_bytes()[..8]
                            .try_into()
                            .unwrap(),
                    ) as usize
                }
                Self::Playlist(fixture, cancel) => profile_playlist(&fixture.0, *cancel)?,
                Self::Catalog(html) => {
                    let catalog = local::aniwaves::parse_catalog_html(
                        local::aniwaves::CatalogView::Search("synthetic".to_owned()),
                        1,
                        std::hint::black_box(html),
                    )
                    .map_err(|_| "synthetic catalog parsing failed")?;
                    if catalog.items.len() != 60 {
                        return Err("synthetic catalog did not yield 60 entries");
                    }
                    std::hint::black_box(catalog.items.len())
                }
            };
            checksum = checksum.wrapping_add(size);
        }
        Ok(std::hint::black_box(checksum))
    }
}

fn profile_playlist(
    selections: &[local::playlist_resolution::Selection],
    cancel: bool,
) -> Result<usize, &'static str> {
    use local::playlist_resolution::{Resolution, WORKERS};
    let job = Arc::new(Mutex::new(Resolution::new(selections.to_vec())));
    let entered = Barrier::new(WORKERS + 1);
    let release = Barrier::new(WORKERS + 1);
    std::thread::scope(|scope| {
        for _ in 0..WORKERS {
            let job = &job;
            let entered = &entered;
            let release = &release;
            scope.spawn(move || {
                workflows::playlist_resolution::profile_worker(job, |url| {
                    if cancel {
                        entered.wait();
                        release.wait();
                    }
                    let resolved = local::models::ResolvedVideo {
                        filename: format!("youtube-{}.mp4", local::youtube::video_id(url).unwrap()),
                        media_url: "https://example.invalid/synthetic-stream".to_owned(),
                        audio_url: None,
                        extract_audio: false,
                        quality_label: Some("720p".to_owned()),
                        quality_height: Some(720),
                    };
                    Ok(local::models::DiscoveryCandidate {
                        qualities: vec![resolved.clone()],
                        resolved,
                        source_url: url.to_owned(),
                        author: "Synthetic".to_owned(),
                        text: "Synthetic entry".to_owned(),
                        playlist: None,
                    })
                })
            });
        }
        if cancel {
            entered.wait();
            job.lock().unwrap().cancelled = true;
            release.wait();
        }
    });
    let progress = local::playlist_resolution::snapshot(&job);
    let state = job.lock().unwrap();
    if state.workers != 0 || state.cancelled != cancel {
        return Err("playlist workers did not finish");
    }
    if cancel {
        if state.completed != 0 || state.next != WORKERS || state.quality_token.is_some() {
            return Err("cancelled playlist continued or published partial results");
        }
        Ok(std::hint::black_box(progress.to_string()).len())
    } else {
        if state.completed != selections.len() {
            return Err("playlist did not resolve every entry");
        }
        let token = state
            .quality_token
            .as_ref()
            .ok_or("playlist did not publish a session")?;
        let session = local::discovery::DISCOVERY_SESSIONS
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .remove(token)
            .ok_or("synthetic session missing")?;
        if session.candidates.len() != selections.len()
            || session.candidates.iter().enumerate().any(|(i, candidate)| {
                candidate.playlist.as_ref().map(|p| p.position) != Some(i + 1)
            })
        {
            return Err("playlist order or candidate count changed");
        }
        Ok(std::hint::black_box(session.candidates.len()))
    }
}

fn hash_page(body: String) -> usize {
    u64::from_le_bytes(
        blake3::hash(body.as_bytes()).as_bytes()[..8]
            .try_into()
            .unwrap(),
    ) as usize
}
