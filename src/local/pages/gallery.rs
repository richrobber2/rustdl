//! Gallery page rendering and responses.

use super::super::super::local;
use super::super::super::local::gallery::{
    GALLERY_INITIAL_ITEMS, GALLERY_LAST_ITEMS, GALLERY_RENDER_COUNT, GALLERY_RENDER_MICROS,
};
use super::super::super::local::queue::DownloadPhase;
use super::super::super::local::runtime::PUBLISH_HOOK;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::Instant;

pub(in super::super::super) fn render(output_dir: &Path) -> io::Result<String> {
    local::pages::gallery::render_playlist(output_dir, None)
}

#[derive(Default)]
pub(in super::super::super) struct PlaylistGalleryGroup {
    pub(in super::super::super) title: String,
    pub(in super::super::super) members: Vec<String>,
    pub(in super::super::super) ready: Vec<String>,
    pub(in super::super::super) downloading: usize,
    pub(in super::super::super) total: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super::super) struct GalleryEntry {
    pub(in super::super::super) href: String,
    pub(in super::super::super) filename: Option<String>,
    pub(in super::super::super) state: String,
    pub(in super::super::super) title: String,
    pub(in super::super::super) subtitle: String,
    pub(in super::super::super) search_text: String,
    pub(in super::super::super) kind: String,
    pub(in super::super::super) thumbnail: Option<String>,
    pub(in super::super::super) transition_name: Option<String>,
}

pub(in super::super::super) fn render_entry(entry: &GalleryEntry, index: usize) -> String {
    let mut classes = "media-card".to_owned();
    match entry.kind.as_str() {
        "playlist" => classes.push_str(" collection-folder"),
        "audio" => classes.push_str(" audio"),
        "downloading" => classes.push_str(" downloading"),
        "downloading-audio" => classes.push_str(" downloading audio"),
        _ => {}
    }
    let thumbnail = entry.thumbnail.as_ref().map_or_else(String::new, |filename| {
        format!(
            r#"<img class="media-art" src="/thumbnail/{}.jpg" loading="lazy" decoding="async" data-thumbnail-retry="0" alt="">"#,
            local::html::escape_html(filename)
        )
    });
    let transition = entry
        .transition_name
        .as_ref()
        .map_or_else(String::new, |name| {
            format!(
                r#" data-view-transition-name="{}""#,
                local::html::escape_html(name)
            )
        });
    let card = format!(
        r#"<a class="{classes}" href="{}" data-gallery-index="{index}" data-gallery-kind="{}"><div class="media-thumb"{transition}>{thumbnail}</div><div class="media-info"><span class="media-state">{}</span><span class="media-title">{}</span><span class="media-file">{}</span></div></a>"#,
        local::html::escape_html(&entry.href),
        local::html::escape_html(&entry.kind),
        local::html::escape_html(&entry.state),
        local::html::escape_html(&entry.title),
        local::html::escape_html(&entry.subtitle),
    );
    if entry.filename.is_some() {
        format!(
            r#"<div class="media-card-shell">{card}<button class="card-menu-button" type="button" aria-label="Media actions">•••</button></div>"#
        )
    } else {
        card
    }
}

#[derive(Clone, Debug)]
pub(in super::super::super) struct GalleryModel {
    pub(in super::super::super) entries: Vec<GalleryEntry>,
    pub(in super::super::super) library_title: String,
    pub(in super::super::super) library_summary: String,
    pub(in super::super::super) collection_nav: String,
    pub(in super::super::super) item_count: usize,
}

/// Shared typed library data for native and web renderers; never parses HTML.
pub(in super::super::super) fn model(
    output_dir: &Path,
    selected_playlist: Option<&str>,
) -> io::Result<GalleryModel> {
    let mut filenames = if local::runtime::inspection_mode() {
        Vec::new()
    } else {
        local::gallery::load_gallery_filenames(output_dir)?
    };
    let ready_filenames = filenames.iter().cloned().collect::<HashSet<_>>();
    let mut active = if local::runtime::inspection_mode() {
        Vec::new()
    } else {
        local::queue::download_jobs()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .filter(|(_, job)| {
                matches!(
                    job.phase,
                    DownloadPhase::Queued
                        | DownloadPhase::Starting
                        | DownloadPhase::Downloading
                        | DownloadPhase::Paused
                )
            })
            .map(|(filename, _)| filename.clone())
            .filter(|filename| {
                local::media::valid_video_filename(filename) && !ready_filenames.contains(filename)
            })
            .collect::<Vec<_>>()
    };
    active.sort_unstable_by(|left, right| right.cmp(left));
    let item_count = active.len() + filenames.len();
    let memberships = if local::runtime::inspection_mode() {
        HashMap::new()
    } else {
        local::gallery::load_playlist_memberships(output_dir)?
    };
    let mut folder_entries = Vec::<GalleryEntry>::new();
    let mut collection_nav = String::new();
    let mut library_title = "Your gallery".to_owned();
    let mut library_summary = format!("{item_count} media items");
    if let Some(playlist_id) = selected_playlist {
        let title = memberships
            .values()
            .find(|membership| membership.playlist_id == playlist_id)
            .map(|membership| membership.title.clone())
            .unwrap_or_else(|| "Playlist folder".to_owned());
        filenames.retain(|filename| {
            memberships
                .get(filename)
                .is_some_and(|membership| membership.playlist_id == playlist_id)
        });
        active.retain(|filename| {
            memberships
                .get(filename)
                .is_some_and(|membership| membership.playlist_id == playlist_id)
        });
        filenames.sort_by_key(|filename| {
            memberships
                .get(filename)
                .map_or(usize::MAX, |membership| membership.position)
        });
        active.sort_by_key(|filename| {
            memberships
                .get(filename)
                .map_or(usize::MAX, |membership| membership.position)
        });
        library_title = title;
        library_summary = format!("{} playlist items", filenames.len() + active.len());
        collection_nav = r#"<div class="collection-nav"><a href="/">← All media</a><span class="media-state">Playlist folder</span></div>"#.to_owned();
    } else if !local::runtime::inspection_mode() {
        let mut groups = HashMap::<String, PlaylistGalleryGroup>::new();
        for filename in filenames.iter().chain(active.iter()) {
            let Some(membership) = memberships.get(filename) else {
                continue;
            };
            let group = groups.entry(membership.playlist_id.clone()).or_default();
            group.title = membership.title.clone();
            group.total = group.total.max(membership.total);
            group.members.push(filename.clone());
            if ready_filenames.contains(filename) {
                group.ready.push(filename.clone());
            } else {
                group.downloading += 1;
            }
        }
        let grouped = groups
            .values()
            .flat_map(|group| group.members.iter().cloned())
            .collect::<HashSet<_>>();
        filenames.retain(|filename| !grouped.contains(filename));
        active.retain(|filename| !grouped.contains(filename));
        let folder_count = groups.len();
        let mut groups = groups.into_iter().collect::<Vec<_>>();
        groups.sort_by(|left, right| {
            left.1
                .title
                .to_lowercase()
                .cmp(&right.1.title.to_lowercase())
        });
        folder_entries = groups
            .into_iter()
            .map(|(playlist_id, mut group)| {
                group.members.sort_by_key(|filename| {
                    memberships
                        .get(filename)
                        .map_or(usize::MAX, |value| value.position)
                });
                let thumbnail = group
                    .members
                    .iter()
                    .find(|filename| {
                        ready_filenames.contains(*filename)
                            && !local::media::is_audio_filename(filename)
                    })
                    .cloned();
                let saved = group.ready.len();
                let progress = if group.downloading == 0 {
                    format!("{saved} saved of {}", group.total.max(saved))
                } else {
                    format!("{saved} saved · {} downloading", group.downloading)
                };
                GalleryEntry {
                    href: format!("/gallery/playlist/{playlist_id}"),
                    filename: None,
                    state: "Playlist folder".to_owned(),
                    title: group.title.clone(),
                    subtitle: progress.clone(),
                    search_text: format!("{} {}", group.title, progress).to_lowercase(),
                    kind: "playlist".to_owned(),
                    thumbnail,
                    transition_name: None,
                }
            })
            .collect();
        library_summary = if folder_count == 0 {
            format!("{item_count} media items")
        } else {
            format!("{item_count} media · {folder_count} folders")
        };
    }
    let mut gallery_entries = folder_entries;
    gallery_entries.extend(active.iter().map(|filename| GalleryEntry {
        href: format!("/watch/{filename}"),
        filename: Some(filename.clone()),
        state: "Downloading".to_owned(),
        title: "Stream while saving".to_owned(),
        subtitle: filename.clone(),
        search_text: format!("stream while saving {filename}").to_lowercase(),
        kind: if local::media::is_audio_filename(filename) {
            "downloading-audio".to_owned()
        } else {
            "downloading".to_owned()
        },
        thumbnail: None,
        transition_name: Some(local::web_assets::view_transition_name(filename)),
    }));
    gallery_entries.extend(filenames.iter().map(|filename| {
        let audio = local::media::is_audio_filename(filename);
        GalleryEntry {
            href: format!("/watch/{filename}"),
            filename: Some(filename.clone()),
            state: "Ready".to_owned(),
            title: if audio {
                "Open audio player"
            } else {
                "Open player"
            }
            .to_owned(),
            subtitle: filename.clone(),
            search_text: format!(
                "{} {}",
                if audio {
                    "open audio player"
                } else {
                    "open player"
                },
                filename
            )
            .to_lowercase(),
            kind: if audio { "audio" } else { "video" }.to_owned(),
            thumbnail: (!audio).then(|| filename.clone()),
            transition_name: Some(local::web_assets::view_transition_name(filename)),
        }
    }));
    Ok(GalleryModel {
        entries: gallery_entries,
        library_title,
        library_summary,
        collection_nav,
        item_count,
    })
}

pub(in super::super::super) fn render_playlist(
    output_dir: &Path,
    selected_playlist: Option<&str>,
) -> io::Result<String> {
    let render_started = Instant::now();
    let GalleryModel {
        entries: gallery_entries,
        library_title,
        library_summary,
        collection_nav,
        item_count,
    } = model(output_dir, selected_playlist)?;
    let library = if local::runtime::inspection_mode() {
        format!(include_str!("../../../assets/html/gallery-inspection.html"),).to_owned()
    } else {
        let initial_cards = gallery_entries
            .iter()
            .take(GALLERY_INITIAL_ITEMS)
            .enumerate()
            .map(|(index, entry)| local::pages::gallery::render_entry(entry, index))
            .collect::<String>();
        let gallery_json = local::html::safe_script_json(&gallery_entries)?;
        {
            let library_heading = &(local::html::escape_html(&library_title));
            format!(
                include_str!("../../../assets/html/gallery.html"),
                library_heading = library_heading,
                collection_nav = collection_nav,
                gallery_json = gallery_json,
                initial_cards = initial_cards,
                library_summary = library_summary
            )
        }
    };
    let banner = if local::runtime::inspection_mode() {
        r#"<div class="inspection">Inspection mode · user data and network downloads are disabled</div>"#
    } else {
        ""
    };
    let mode_switch = if PUBLISH_HOOK.get().is_some() {
        if local::runtime::inspection_mode() {
            r#"<a class="mode-switch" href="rustdl://mode/normal">← Return to my gallery</a>"#
        } else {
            r#"<a class="mode-switch" href="rustdl://mode/inspection">◇ Preview safe UI</a>"#
        }
    } else {
        ""
    };
    let playback_script = local::web_assets::playback_script_tag();
    let view_transition_script = local::web_assets::view_transition_script_tag();
    let mut html = local::web_assets::index_html_template()
        .replace("<!--SAVED_VIDEOS-->", &library)
        .replace("<!--INSPECTION_BANNER-->", banner)
        .replace("<!--MODE_SWITCH-->", mode_switch)
        .replace(
            "<!--PLAYBACK_SCRIPT-->",
            if local::runtime::inspection_mode() {
                ""
            } else {
                &playback_script
            },
        )
        .replace("<!--VIEW_TRANSITIONS-->", &view_transition_script)
        .replace("<!--DEV_RELOAD-->", &local::dev::dev_reload_script());
    if local::runtime::inspection_mode() {
        html = html.replace(
            r#"<a class="queue-link" id="activity-link" href="/activity">Activity <span class="activity-count" id="activity-count" hidden></span> →</a>"#,
            "",
        );
        html = html.replace(
            r#"<a class="queue-link" href="/peers">Device transfer →</a>"#,
            "",
        );
        html = html.replace(
            r#"<a class="queue-link" href="/settings">Settings →</a>"#,
            "",
        );
        html = html.replace(" required autofocus", " required");
        html = html.replace(
            "</main>",
            r#"</main><aside class="queue-mini" aria-label="Synthetic download queue"><a href="/__inspect/result">↓</a><div class="queue-mini-info"><strong>synthetic-download.mp4</strong><span>downloading · 26.0 MB / 64.0 MB</span><div class="queue-mini-progress"><i style="width:41%"></i></div></div><button type="button">Pause</button></aside>"#,
        );
    }
    if !local::runtime::inspection_mode() {
        GALLERY_RENDER_COUNT.fetch_add(1, Ordering::Relaxed);
        GALLERY_RENDER_MICROS.store(
            render_started
                .elapsed()
                .as_micros()
                .min(u128::from(u64::MAX)) as u64,
            Ordering::Relaxed,
        );
        GALLERY_LAST_ITEMS.store(item_count as u64, Ordering::Relaxed);
    }
    Ok(html)
}
