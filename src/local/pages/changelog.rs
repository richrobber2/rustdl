//! Changelog page rendering and responses.

use super::super::super::local;
use std::error::Error;
use tiny_http::Request;

pub(in super::super::super) const CHANGELOG: &[(&str, &[&str])] = &[
    (
        "0.1.45",
        &[
            "Enforced mobile-data preferences for queued and background episode downloads, with connection approval, waiting reasons, and partial-file resuming.",
            "Fixed episode watchlist saves for shows without posters and added clearer errors and duplicate-tap protection.",
            "Added paginated playlist selection and bounded background preparation for large playlists, plus fixed-size Rust download workers.",
            "Added Rainy City light and dark themes, sunrise transitions, parallax, a reduced-motion setting, and opt-in screenshots.",
            "Added AniWaves category browsing and improved mobile search layout and apostrophe formatting.",
            "Added in-page Next playback for compatible media and refreshed seekbar metadata on track changes.",
            "Expanded visual auditing across screen sizes and orientations, with missing detector coverage flagged for review.",
            "Removed obsolete Java compiler-option notices while retaining code warnings and Java 8 compatibility.",
        ],
    ),
    (
        "0.1.44",
        &[
            "Disabled deferred gallery painting, paint containment, and scroll-reveal animations to address blank cards that appear only after interaction.",
            "Reused loaded gallery cards during startup and filtering, and kept imports in place without resetting scroll position.",
            "Added batch-loading fallbacks and automatic recovery for delayed refreshes and thumbnails, with stale retries cancelled and background work paused.",
            "Added anime downloads and improved form sizing and window insets on smaller phones.",
            "Kept the activity indicator hidden when there is no activity.",
            "Organized local features, external providers, workflows, and shared page assets into separate modules.",
            "Added optional development gallery benchmarks, coverage reporting, and metrics-only measurements of the actual gallery.",
        ],
    ),
    (
        "0.1.43",
        &[
            "Fixed instant streaming bookmarks by preventing the action field from shadowing the form endpoint in JavaScript.",
            "Fixed watchlist removal counts and the transition into the empty-library state on older WebViews.",
            "Added regression coverage for both streaming bookmark form handlers.",
        ],
    ),
    (
        "0.1.42",
        &[
            "Added a persistent APK-local streaming watchlist with instant catalog controls and a native player save action.",
            "Added a release calendar that groups saved shows by their known episode day and flags changes since the previous successful check.",
            "Added precise episode timestamps, bounded four-worker schedule refreshes, five-minute caching, and fail-safe per-show seen markers.",
        ],
    ),
    (
        "0.1.41",
        &[
            "Added a native all-episodes selector to the protected streaming player header.",
            "Included episode titles in the dropdown while keeping a compact episode number during playback.",
            "Kept previous and next episode shortcuts and prevented overlapping manifest loads during direct selection.",
        ],
    ),
    (
        "0.1.40",
        &[
            "Expanded streaming discovery beyond Newest with Updated, Ongoing, and Added browse feeds.",
            "Replaced current-page title filtering with Rust-backed full-library AniWaves search and paginated results.",
            "Added direct handling for AniWaves home, browse, and search links while keeping playback isolated and redirect-safe.",
        ],
    ),
    (
        "0.1.39",
        &[
            "Made redirect permissions source-specific, so one provider can never expand another provider’s allowed hosts.",
            "Added All, SUB, DUB, Ready, and Issues filters above the protected player with live source counts.",
            "Made automatic failover respect the active source filter and exposed each provider’s health or checked-redirect state.",
        ],
    ),
    (
        "0.1.38",
        &[
            "Blocked every script-driven and HTTP redirect from local app pages to external sites while preserving deliberate user taps.",
            "Locked the isolated streaming WebView to the exact provider host selected by Rust, with no temporary cross-host redirect window.",
            "Moved legitimate provider redirect resolution into Rust health checks so known final player hosts open directly and safely.",
        ],
    ),
    (
        "0.1.37",
        &[
            "Replaced full AniWaves watch-page playback with an isolated RustDL player that opens only the selected provider embed.",
            "Added episode-scoped primary and backup discovery across Vidplay, BYFMS, DGHG, DatSaV, MyCloud, and newly returned server labels.",
            "Added concurrent Rust health checks, two-minute manifests, native SUB/DUB source controls, episode navigation, and automatic failover.",
        ],
    ),
    (
        "0.1.36",
        &[
            "Added a persistent Up Next playback queue with add/remove actions on every gallery media card.",
            "Added Next controls to the full player and mini-player, plus Android media-session and keyboard navigation.",
            "Added a Rust-backed gallery fallback order, queue count, stale-item cleanup, and one-tap queue clearing.",
        ],
    ),
    (
        "0.1.35",
        &[
            "Added persistent system, dark, and light appearance modes across every Rust-generated app page.",
            "Added a quick light/dark switch with smooth no-layout theme transitions and matching Android system bars.",
            "Added a drifting space background built from transform-only star fields that pauses while hidden and respects reduced motion.",
            "Added an APK-local space-effect switch and synchronized appearance through the gallery mini-player.",
            "Polished light mode across gallery controls, forms, queues, diagnostics, settings, changelog, and device-transfer surfaces.",
            "Centralized dark and light appearance into shared semantic root variables instead of separate component color patches.",
            "Added an original APK-bundled cosmic backdrop for a richer dark theme without network-loaded artwork.",
            "Added isolated AniWaves streaming with no download interception, privileged app bridges, popups, permissions, or external redirects.",
            "Added a Rust-rendered YouTube-style AniWaves newest catalog with lazy poster cards, instant search, pagination, caching, and one-tap protected playback.",
            "Made large galleries incremental with a bounded 32-card DOM window, visible-ahead loading, and global in-memory search.",
            "Replaced hundreds of per-card popovers with one reusable action sheet and moved shared-thumbnail transitions onto the selected card only.",
            "Added cached Rust gallery metadata, constant-time membership lookups, and a two-worker thumbnail queue with lightweight 480px previews.",
            "Added privacy-safe gallery timing, DOM, thumbnail-cache, and long-task counters to Diagnostics.",
        ],
    ),
    (
        "0.1.34",
        &[
            "Added a persistent mini-player that keeps the current media alive while browsing the gallery and app pages.",
            "Added compact play, Picture-in-Picture, expand, and close actions without covering the video.",
            "Upgraded Android Picture-in-Picture with the real video aspect ratio, automatic entry while playing, and seamless resizing.",
            "Made large galleries lighter to render and preserved search, filters, and scroll position across navigation.",
        ],
    ),
    (
        "0.1.33",
        &[
            "Added on-the-fly video-frame previews while dragging or tapping the seeker.",
            "Throttled preview decoding to the latest gesture position and used fast keyframe seeking when WebView supports it.",
            "Kept active-download previews inside real buffered ranges and showed a waiting state beyond available media.",
        ],
    ),
    (
        "0.1.32",
        &[
            "Made progressive playback seek against real browser-buffered time ranges instead of assuming byte percentage equals video time.",
            "Applied safe streaming seeks to touch, keyboard, double-tap, restored position, and Android media-session controls.",
            "Replaced growing-file polling with immediate Rust condition-variable wakeups when new download bytes arrive.",
        ],
    ),
    (
        "0.1.31",
        &[
            "Added a live Activity Center for downloads, device transfers, storage pressure, and app updates.",
            "Added active and attention badges plus immediate typed events for transfer and updater state changes.",
            "Added safe activity filters and direct actions without exposing peer addresses, thumbnails, or media contents.",
        ],
    ),
    (
        "0.1.30",
        &[
            "Added a typed Rust-to-Java-to-WebView event bridge for immediate app-state updates.",
            "Made queue progress, phases, errors, and actions update in place without two-second page reloads.",
            "Moved the player and gallery to event-driven refreshes with coalescing and a low-frequency recovery fallback.",
        ],
    ),
    (
        "0.1.29",
        &[
            "Added a persistent Settings page backed entirely by the installed APK.",
            "Added a safe custom Downloads subfolder for newly exported video and audio while preserving existing file locations.",
            "Added playback screen-awake and Diagnostics refresh controls with one-tap default restore.",
        ],
    ),
    (
        "0.1.28",
        &[
            "Rebuilt Diagnostics as a fail-soft dashboard where one unavailable Android source can no longer blank the entire page.",
            "Added explicit source coverage, safe value formatting, useful unavailable states, and clearer refresh feedback.",
            "Kept all telemetry APK-local with no companion process or external installation.",
        ],
    ),
    (
        "0.1.27",
        &[
            "Removed the ADB shell companion, Device control page, privileged receiver, and secure-settings permission.",
            "Moved diagnostics into the bundled APK using Android APIs and app-readable system totals.",
            "RustDL now exposes only features that work directly after installing the APK, with no external bootstrap.",
        ],
    ),
    (
        "0.1.26",
        &[
            "Made the Thermal diagnostics card show a real device-temperature reading instead of an apparently empty normal-status bar.",
            "Moved thermal collection onto lightweight battery sysfs and Android PowerManager sources so diagnostics responds immediately.",
            "Kept Android's throttling status visible beside the temperature.",
        ],
    ),
    (
        "0.1.25",
        &[
            "Added contextual Go to actions that open the screen where each changelog feature lives.",
            "Added precise deep links for downloader, device-control, queue, storage, transfer, diagnostics, gallery, and inspection features.",
            "Kept the sticky version picker for quickly moving through the full release history.",
        ],
    ),
    (
        "0.1.24",
        &[
            "Added sticky Go to version navigation to the in-app changelog.",
            "Every release now has a stable deep-link anchor and selected-release highlight.",
            "Added smooth one-tap jumps with reduced-motion and no-JavaScript anchor fallbacks.",
        ],
    ),
    (
        "0.1.23",
        &[
            "Added a live, privacy-scoped diagnostics dashboard backed by the authenticated shell companion.",
            "Added battery, temperature, thermal, CPU load, memory, storage, uptime, and RustDL process health.",
            "Kept diagnostics out of inspection mode and excluded logs, user media, Wi-Fi identity, and other-app enumeration.",
        ],
    ),
    (
        "0.1.22",
        &[
            "Added real allowlisted controls for system animation speed, global media transport, and music volume.",
            "Added live microphone and camera privacy state with explicit Block and Allow actions.",
            "Added a Motorola FM launcher while keeping protected tuning internals and arbitrary shell commands inaccessible.",
        ],
    ),
    (
        "0.1.21",
        &[
            "Added a token-authenticated ADB shell companion, shell-only bootstrap, and an explicit capability allowlist.",
            "Added an in-app device-control page with live companion and persistent-control status.",
            "Kept device controls and their JavaScript bridge completely out of no-user-data inspection mode.",
        ],
    ),
    (
        "0.1.20",
        &[
            "Replaced the browser's default pointer behavior on the seeker with RustDL-controlled scrubbing.",
            "Added pointer capture for stable mobile drags and prevented seeker gestures from leaking into page interaction.",
            "Kept keyboard seeking accessible while previewing and enforcing progressive-download boundaries.",
        ],
    ),
    (
        "0.1.19",
        &[
            "Moved custom playback controls into a dedicated dock below video and audio so they never obscure media during normal playback.",
            "Kept a compact auto-hiding control overlay only while fullscreen.",
            "Moved download status outside the picture and corrected desktop/mobile control grid sizing.",
        ],
    ),
    (
        "0.1.18",
        &[
            "Gallery thumbnails are now generated visible-first through lazy WebView requests, with two bounded Android decoders and direct scaled-frame extraction.",
            "RustDL now records BLAKE3 fingerprints while media downloads and caches validated fingerprints for fast duplicate storage scans.",
            "Index/player CSS and playback transitions now use content-hashed immutable assets for instant repeat navigation.",
            "Download concurrency and buffers now adapt to the phone's network, charging, power-save, thermal, free-storage, and CPU conditions.",
        ],
    ),
    (
        "0.1.17",
        &[
            "Pairing QR refreshes now use a compact Rust JSON endpoint instead of downloading and parsing the full page.",
            "The replacement SVG is applied directly while the current QR remains visible and the controls stay visually unchanged.",
            "Shortened the QR-only transition to about 100 ms for an effectively instant switch.",
        ],
    ),
    (
        "0.1.16",
        &[
            "Generating a new pairing QR now keeps the current page and code visible while Rust creates the replacement.",
            "Reserved QR and pairing-value geometry prevents layout movement during refresh.",
            "Added an in-place QR-only view transition with reduced-motion support and no changing button label.",
        ],
    ),
    (
        "0.1.15",
        &[
            "Added instant gallery search across media filenames and playlist folder titles.",
            "Added one-tap gallery filters for playlists, video, audio, and active downloads.",
            "Added live visible-result counts and an accessible empty-results state.",
        ],
    ),
    (
        "0.1.14",
        &[
            "Added Download selected as to apply one format choice across an entire playlist queue batch.",
            "Bulk presets cover best MP4, MP4 up to 1080p, 720p, or 480p, and audio-only M4A.",
            "Per-item format selectors remain available after applying a bulk preset for exceptions.",
        ],
    ),
    (
        "0.1.13",
        &[
            "Playlist downloads now stay together as named folders in the main gallery.",
            "Opening a playlist folder shows only its saved and downloading items in the original playlist order.",
            "Playlist grouping persists across restarts and additional queue batches without moving playable media out of Android Downloads.",
        ],
    ),
    (
        "0.1.12",
        &[
            "Added camera-scannable QR pairing between RustDL devices without adding camera permission to the app.",
            "Pairing deep links now hand the short-lived secret directly from Android to Rust instead of placing it in localhost URLs.",
            "Added a searchable completed-media picker after pairing, with one-tap encrypted sending and manual pairing as a fallback.",
        ],
    ),
    (
        "0.1.11",
        &[
            "Added direct encrypted transfers between RustDL devices on the same local network.",
            "Added short-lived manual pairing and a Send to device action for saved media.",
            "Added resumable 1 MiB chunks with XChaCha20-Poly1305 authentication and final BLAKE3 verification.",
        ],
    ),
    (
        "0.1.10",
        &[
            "Load complete public YouTube playlists across continuation pages.",
            "Search playlists and choose individual, visible, or first-10 entries before resolving formats.",
            "Added this in-app cumulative changelog.",
        ],
    ),
    (
        "0.1.9",
        &[
            "Added the first validated YouTube playlist discovery path.",
            "Confirmed ordered playlist deduplication with a temporary five-entry validation limit.",
        ],
    ),
    (
        "0.1.8",
        &[
            "Added Snapchat Spotlight support.",
            "Added Audio-only M4A choices for every supported video source.",
            "Upgraded playback with anchored controls, resume, speed, Picture-in-Picture, and media-session actions.",
        ],
    ),
    (
        "0.1.7",
        &[
            "Added YouTube videos and Shorts.",
            "Added the per-item quality wizard and safer multi-item queue controls.",
            "Added storage management for completed, partial, duplicate, watched, and thumbnail data.",
        ],
    ),
    (
        "0.1.6",
        &[
            "Added signed in-app APK update checks, downloads, verification, and install handoff.",
            "Repaired Android package versioning and install compatibility.",
        ],
    ),
    (
        "0.1.5",
        &[
            "Added an in-app switch between normal user mode and synthetic inspection mode.",
            "Kept screenshots blocked in user mode while preserving thumbnail generation for the gallery.",
        ],
    ),
    (
        "0.1.4",
        &[
            "Redesigned the video player and repaired fullscreen playback.",
            "Added shared-thumbnail View Transitions between the gallery and single-item player.",
        ],
    ),
    (
        "0.1.3",
        &[
            "Made active downloads streamable while bytes are still arriving.",
            "Added a gallery that opens an optimized single-media player instead of embedding every video.",
        ],
    ),
    (
        "0.1.2",
        &[
            "Added the Android APK and system share target.",
            "Added secure user mode and a separate no-user-data UI inspection process.",
        ],
    ),
    (
        "0.1.1",
        &[
            "Added the browser-based video player and gallery foundation.",
            "Added Rust development hot reload for faster UI iteration.",
        ],
    ),
    (
        "0.1.0",
        &[
            "Created the pure-Rust web app and X video downloader.",
            "Added the RustDL download folder and duplicate detection.",
        ],
    ),
];

pub(in super::super::super) fn destinations(
    version: &str,
) -> &'static [(&'static str, &'static str)] {
    match version {
        "0.1.44" => &[
            ("/#gallery-library", "Gallery"),
            ("/streaming", "Anime downloads"),
        ],
        "0.1.43" => &[
            ("/streaming", "Streaming bookmarks"),
            ("/streaming/watchlist", "Watchlist"),
        ],
        "0.1.42" => &[
            ("/streaming/watchlist", "Streaming watchlist"),
            ("/streaming/calendar", "Release calendar"),
        ],
        "0.1.41" => &[("/streaming", "Episode selector")],
        "0.1.40" => &[("/streaming", "Browse feeds & full-library search")],
        "0.1.39" => &[("/streaming", "Source filters & isolated redirects")],
        "0.1.38" => &[("/streaming", "Redirect-safe streaming")],
        "0.1.37" => &[("/streaming", "Streaming catalog & source player")],
        "0.1.36" => &[("/#gallery-library", "Up Next queue & player")],
        "0.1.35" => &[
            ("/settings", "Appearance & space effect"),
            ("/streaming", "AniWaves streaming catalog"),
        ],
        "0.1.34" => &[("/#gallery-library", "Mini-player & Picture-in-Picture")],
        "0.1.33" => &[("/#gallery-library", "Seek previews")],
        "0.1.32" => &[("/#gallery-library", "Streaming player")],
        "0.1.31" => &[("/activity", "Activity Center")],
        "0.1.30" => &[("/queue", "Live queue"), ("/#gallery-library", "Gallery")],
        "0.1.29" => &[("/settings", "Settings")],
        "0.1.28" | "0.1.27" | "0.1.26" | "0.1.23" => &[("/diagnostics", "Diagnostics")],
        "0.1.25" | "0.1.24" => &[("/changelog#version-jump", "version navigation")],
        "0.1.22" | "0.1.21" => &[],
        "0.1.20" | "0.1.19" | "0.1.4" | "0.1.3" => &[("/#gallery-library", "Gallery & player")],
        "0.1.18" => &[
            ("/#gallery-library", "Gallery"),
            ("/storage", "Storage"),
            ("/diagnostics", "Diagnostics"),
        ],
        "0.1.17" | "0.1.16" | "0.1.12" | "0.1.11" => &[("/peers", "Device transfer")],
        "0.1.15" => &[("/#gallery-library", "Gallery search")],
        "0.1.14" | "0.1.13" | "0.1.10" | "0.1.9" => &[("/#downloader", "Playlist downloader")],
        "0.1.8" => &[
            ("/#downloader", "Downloader"),
            ("/#gallery-library", "Gallery & player"),
        ],
        "0.1.7" => &[("/queue", "Download queue"), ("/storage", "Storage")],
        "0.1.6" => &[("/", "Update status")],
        "0.1.5" => &[("rustdl://mode/inspection", "Safe UI preview")],
        "0.1.2" => &[("/#downloader", "Share/download home")],
        "0.1.1" => &[("/#gallery-library", "Gallery & player")],
        "0.1.0" => &[("/#downloader", "Downloader")],
        _ => &[],
    }
}

pub(in super::super::super) fn render() -> String {
    let current = env!("CARGO_PKG_VERSION");
    let options = CHANGELOG
        .iter()
        .map(|(version, _)| {
            let id = format!("version-{}", version.replace('.', "-"));
            let selected = if *version == current { " selected" } else { "" };
            format!(
                r#"<option value="{id}"{selected}>Version {}</option>"#,
                local::html::escape_html(version)
            )
        })
        .collect::<String>();
    let releases = CHANGELOG
        .iter()
        .map(|(version, changes)| {
            let id = format!("version-{}", version.replace('.', "-"));
            let badge = if *version == current {
                r#"<span class="current">Current</span>"#
            } else {
                ""
            };
            let items = changes
                .iter()
                .map(|change| format!("<li>{}</li>", local::html::escape_html(change)))
                .collect::<String>();
            let destinations = local::pages::changelog::destinations(version)
                .iter()
                .map(|(href, label)| {
                    format!(
                        r#"<a class="goto" href="{}">Go to {} <span aria-hidden="true">→</span></a>"#,
                        local::html::escape_html(href),
                        local::html::escape_html(label)
                    )
                })
                .collect::<String>();
            let actions = if destinations.is_empty() {
                String::new()
            } else {
                format!(r#"<footer class="release-actions"><span>Find it in RustDL</span><div>{destinations}</div></footer>"#)
            };
            format!(
                r##"<article id="{id}"><header><h2><a class="version-link" href="#{id}">Version {version}</a></h2>{badge}</header><ul>{items}</ul>{actions}</article>"##
            )
        })
        .collect::<String>();
    {
        let dev_reload = &(local::dev::dev_reload_script());
        format!(
            include_str!("../../../assets/html/changelog.html"),
            dev_reload = dev_reload,
            page_css = include_str!("../../../assets/css/changelog.css"),
            page_script = include_str!("../../../assets/js/changelog.js"),
            options = options,
            releases = releases
        )
    }
}

pub(in super::super::super) fn respond(request: Request) -> Result<(), Box<dyn Error>> {
    local::html::respond_html(request, local::pages::changelog::render())
}
