# rustdl

A Rust CLI, web UI, and Android app for downloading MP4 video from public X posts,
YouTube videos, and Snapchat Spotlight, with an AniWaves streaming catalog on Android.
The Rust binary serves the HTML, CSS, and JavaScript and handles downloads. The UI
runs JavaScript in the browser or Android WebView; no separately installed JavaScript
runtime, Node.js, Python, `yt-dlp`, or `ffmpeg` is required. YouTube extraction also
uses an embedded JavaScript engine through `rustypipe`.

## Quick start: Android APK

From a fresh clone in Termux, install the pinned build prerequisites once and build:

```sh
pkg install git make
git clone https://github.com/richrobber2/rustdl.git
cd rustdl
make setup
make apk
```

`make setup` installs the required Termux packages and downloads a pinned Android
API-35 `android.jar` only after verifying its SHA-256 digest. `make apk` checks every
prerequisite, builds the optimized ARM64 Rust library, compiles the Android wrapper,
creates a local debug signing key when needed, signs the APK, and verifies its
signature. The result is `target/android-termux-gpui/rustdl.apk`.

Install it with Android's package installer or with ADB:

```sh
adb install -r target/android-termux-gpui/rustdl.apk
```

If the original signing key was lost, build a separate installation with:

```sh
RUSTDL_VARIANT=alongside make apk
```

This produces `target/android-termux-alongside-gpui/rustdl.apk`, labeled **RustDL Next**
with package ID `app.rustdl.next`. It uses separate app data, local ports 37758/37759
(and peer port 37760), and `Downloads/RustDL Next`. The existing app remains installed;
its private library and settings are not migrated. Keep `android/debug.keystore`
backed up outside Termux so future builds can update this installation.

The app embeds the Rust server in a native library and renders its interface with
GPUI Kit. Restricted provider decoding and initialization/accessibility fallbacks
retain WebView support while validation is incomplete. Paste a link normally, or use **Share → Download with RustDL**
from X, YouTube, or Snapchat to begin a download immediately. Android's MediaStore publishes completed files to
`Downloads/RustDL` without broad storage permissions; an app-private copy backs the
built-in player and duplicate cache.

Downloads are progressive: once the upstream MP4 headers arrive, RustDL opens the
player and streams from the growing `.part` file while the background worker keeps
writing it. Requests that reach the write edge wait for the next chunk, then resume;
completed files still use atomic rename, duplicate detection, byte-range seeking, and
MediaStore publication. The persistent Smart Queue keeps partial files and resumes
them with HTTP byte ranges after a network interruption or app restart. It runs
one to three transfers concurrently, depending on device conditions, using a fixed
pool of three workers. Waiting items do not create extra threads. The queue exposes
pause, resume, retry, cancel, progress, and direct-play controls. Paste up to 50 links
at once or share text containing multiple X links; duplicate links and already-completed videos are not added twice.
On Android, active work is protected by a foreground data-sync service with a private
aggregate progress notification (after the standard one-time notification permission).

Paste a post or thread to preview every video in its unrolled thread, or paste an X
profile to preview up to 40 recent media posts. RustDL uses FxTwitter's v2 thread and
profile-media APIs and presents a checked selection list before anything is queued.
A second wizard step exposes every available quality per selected item, defaults to
the best stream, and carries that choice into the persistent queue. Every supported
video also offers **Audio only · M4A**. YouTube uses its native M4A audio stream;
for X and Snapchat, Android losslessly copies the existing audio track after the
progressive source finishes, without re-encoding. Audio files have independent
duplicate keys, resume with the queue, publish with the correct MediaStore MIME type,
and open in the gallery's audio player. Discovery
sessions are short-lived; post discovery is capped at 50 videos, while playlist selections can include all loaded entries.

YouTube Shorts, watch, embed, live, and `youtu.be` links resolve through the
`rustypipe` extractor. RustDL prefers progressive MP4 streams that already contain
both video and audio. When those are unavailable, Android can combine separate
H.264 MP4 video and M4A audio tracks using its native media APIs.
Signed YouTube stream links are refreshed automatically before a resumed transfer
when they are close to expiry while retaining the selected resolution. Private,
paid, age-restricted, and region-blocked videos are not bypassed.

YouTube `/playlist?list=…` links are also supported. RustDL follows playlist
continuations to load the full public entry list, then opens a searchable selection
screen without resolving hundreds of media streams up front. Choose individual
entries, the first 10, visible results, or **Select all** for the full playlist; nothing starts
until the selected entries are resolved and the user chooses video quality or
audio-only for each one. Preparation opens a live progress page immediately, reuses
three extractor workers, and supports cancellation. Failed entries are listed before
continuing with the available videos. Playlist and format screens show 32 entries per page; Select all and bulk format
changes still include every page. Selected entries retain their playlist title and original
position. The main gallery presents them as one folder, and opening it shows only
that playlist in order; later batches join the same folder automatically. The format
step also has **Download selected as** presets that apply best MP4, a shared MP4
resolution target, or audio-only M4A to the entire selected batch at once while
keeping individual overrides available.

Public Snapchat Spotlight links resolve from Snapchat's Open Graph metadata. RustDL
accepts both shared `/spotlight/…` links and attributed `/@creator/spotlight/…` links,
downloads only HTTPS media hosted on Snapchat's own `sc-cdn.net` infrastructure, and
uses the Spotlight ID as a stable duplicate key. The original progressive MP4 enters
the same quality wizard, resumable queue, streaming player, gallery, and MediaStore flow.

Saved media can also move directly between two RustDL devices on the same Wi-Fi or
hotspot. The receiver opens **Device transfer** to generate a short-lived QR code;
the sender scans it with the device's normal camera, RustDL opens a searchable list
of completed media, and one tap starts the transfer. No in-app camera permission is
needed, and manual address/key entry remains available as a fallback. Generating a
replacement code keeps the existing QR and page geometry on screen while a compact
Rust JSON endpoint prepares its replacement, then directly swaps the SVG with a
roughly 100 ms QR-only transition. One-megabyte
chunks are encrypted and authenticated with XChaCha20-Poly1305,
interrupted transfers resume from the receiver's saved offset, and the completed
file is published only after its BLAKE3 digest matches.

The **Storage manager** reports the exact RustDL footprint for videos, resumable
partials, thumbnails, and queue metadata. It marks completed playback and matching
file content, identifies stale partials, and can clear regenerable thumbnails. Video,
watched-item, and stale-partial removal require a dedicated confirmation followed by
a token-protected POST. Android deletion removes both the private player copy and the
published `Downloads/RustDL` entry; active transfers cannot be deleted.

The home screen presents ready and active downloads in a
responsive gallery. Completed cards use locally generated JPEG poster frames without
embedding playable video elements; Android now generates only requested visible
posters, with bounded scaled-frame decoding instead of scanning the entire library at
startup. Opening a card creates one dedicated player for that selection. Gallery
search matches media filenames and playlist titles instantly,
with filters for playlist folders, video, audio, and active downloads. Chromium 126+
uses CSS-only
cross-document View Transitions to morph the selected card into its player and back;
older engines retain normal navigation, and reduced-motion preferences are respected.

Player controls live in a dedicated dock below the media during normal playback, so
they never cover the picture. Fullscreen switches the same controls to a compact
auto-hiding overlay, while picture-in-picture remains video-only. Pointer scrubbing
uses a captured RustDL gesture instead of the browser's default range interaction,
while keyboard seeking remains available.

Stable gallery/player styles and scripts use content-hashed immutable URLs so WebView
can reuse parsed assets across navigation. Rust records BLAKE3 fingerprints during
downloads for fast duplicate scans, while active transfer count and I/O buffers adapt to the
phone's network, charging, power-save, thermal, storage, and CPU conditions.

Run `make help` for the short command list. `make check` verifies the Android toolchain
without modifying the device, `make test` runs the optimized Rust suite, `make run`
starts the web app, and `make dev` starts hot reload. Advanced builds can set
`ANDROID_JAR=/path/to/android.jar`. Generated platform files and signing keys remain
local and are ignored by Git.

Screenshots are blocked by default. Enable **Settings → Allow screenshots** and
save to permit screenshots, screen recording, and app previews. This applies to
the main app and anime player; restoring defaults blocks screenshots again.

**Inspection privacy** defaults to on. When screenshots are allowed, it conceals
thumbnails, titles/filenames, and video frames on the main app's screen before
capture, while retaining navigation, app controls, and progress. It also conceals
unaudited pages entirely. Fullscreen screenshots and entering picture-in-picture
are blocked in this mode. This is an on-screen privacy curtain, so the same fields
are hidden from the phone user while screenshots are allowed. Disable screenshots
for normal private viewing. Agents must keep Inspection privacy enabled and use
synthetic inspection pages whenever possible; see [AGENTS.md](AGENTS.md).

Choose **Settings → Background theme → Rainy City**, then **Save settings**, for
the optional city background with subtle parallax while scrolling. Reduced motion
keeps it still. Light mode uses the matching daylight artwork;
dark mode uses the neon night scene. Space remains the default; Restore defaults
returns to it. The background choice persists independently of light/dark mode.

### Streaming catalog, watchlist, and calendar

Open **AniWaves streaming** from the gallery to browse newest, updated, ongoing,
or recently added titles, or search the catalog. On Android, selecting a title opens
a dedicated streaming player with episode selection. Streaming mode disables
downloads and stays separate from the saved-media gallery.

AniWaves browsing includes a Categories picker with 19 genres and six media types.
Category selection is preserved across pagination and refresh; title search still
searches the full library.

Save titles from the catalog or player to the persistent **Watchlist**, then open
**Calendar** to see their latest known release information grouped by day. New badges
compare episode information with the previous successful calendar check. Use
**Refresh schedules** to request fresh information; unavailable schedules are shown
separately. Release timestamps display in the phone's timezone.

### Changelog and push guard

The changelog initially renders five releases and loads older notes in batches of
eight near the end of the page or through **Load older versions**. Version navigation
and bookmarked version links fetch the selected release directly.

Run `make hooks` after cloning to enable the changelog pre-push guard. Every pushed
range that changes tracked content must add concrete notes to `CHANGELOG` in
`src/local/pages/changelog.rs`; uncommitted notes do not count. CI repeats this check
for pushes and pull requests. The local guard is enabled in this checkout. Agents
must not bypass it; see [AGENTS.md](AGENTS.md).

### Activity and settings

Open **Activity** from the gallery for download and device-transfer progress,
errors, available storage, and Android update status. Filters show all operations,
active work, items needing attention, or completed work. Each row links to its
player, queue, or transfer screen. State events refresh the page promptly, with a
15-second recovery poll while it is visible. Activity is unavailable in inspection
mode.

Android's **Settings** page controls the completed-download folder, keeping the
screen awake during playback, appearance, the moving space background, and the
diagnostics refresh interval. Changing the folder affects future exports; existing
exports remain in place and the playback cache remains private to the app.

### Live diagnostics

The normal-mode **Diagnostics** page samples Android APIs from inside the installed
APK every five seconds by default while visible; Settings offers 3, 5, 10, or 30
seconds. It reports battery level, charging state, battery temperature, Android thermal severity, normalized CPU load, available/total
memory, available/total internal storage, device uptime, and the RustDL app process.
A manual refresh and copyable JSON snapshot are available for troubleshooting. No
ADB process, external executable, privileged receiver, or setup step is required.

Diagnostics intentionally exclude logcat, notifications, media filenames, Wi-Fi
identity, location, other-app enumeration, and screen content. The route and Java
bridge are unavailable in inspection mode. Sampling pauses whenever the page is
hidden and resumes when it becomes visible. Sources fail independently: if Android
restricts one reading, its card says so while every available metric continues to
refresh.

### No-user-data UI inspection

Use the dedicated inspection action when validating UI. It starts in a separate
Android task and process with its own WebView data directory, localhost port, and
cache directory. It ignores Android share data, disables network downloads and
MediaStore publication, does not enumerate saved videos, and displays only
RustDL-generated synthetic states:

```sh
sh android/inspect-ui.sh home 10.5.0.2:39271
sh android/inspect-ui.sh result 10.5.0.2:39271
sh android/inspect-ui.sh player 10.5.0.2:39271
```

The script only launches the selected screen; it never captures a screenshot. UI
structure can be inspected separately with Android's accessibility hierarchy tools.

To produce an image without capturing the current Android display, use the guarded
renderer:

```sh
sh android/capture-inspection.sh home target/inspection-home.png 10.5.0.2:39271
sh android/capture-inspection.sh result target/inspection-result.png 10.5.0.2:39271
sh android/capture-inspection.sh player target/inspection-player.png 10.5.0.2:39271
```

This action can only load the inspection server's generated fixtures. Android draws
that WebView directly to a private bitmap, the script retrieves it over localhost,
and only the isolated inspection process closes afterward. The normal app keeps
running. It never captures the device display, another app, saved videos, shared text,
or other user content. Launching RustDL normally or from the share sheet always enters
the normal process, even if inspection mode is active at the same time. Normal
user-mode windows use Android's secure-window protection by default. The optional
**Allow screenshots** setting permits screenshots and display capture in normal mode. Only the isolated synthetic inspection UI is renderable by the
guarded inspection workflow.

The home screen also exposes an explicit mode control. **Preview safe UI** launches
the isolated synthetic task, while **Return to my gallery** returns to the
normal task. User videos and generated thumbnails are never mounted into inspection
mode.

## Web page

```sh
cargo run -- serve
```

Open <http://127.0.0.1:8080>, paste supported X, YouTube, or Snapchat links, and select
**Find videos**. Choose the items and formats to add to the queue.
The Android app exports completed media through MediaStore to `Downloads/RustDL`
by default and keeps a private playback copy. Stable source IDs form the filenames,
so submitting the same item and format again detects the existing file and skips
the duplicate download.

The result page includes a native browser video player as soon as downloading starts.
The home-page gallery shows active and saved videos so they can be reopened later.
The Rust media routes support progressive reads and HTTP byte ranges without loading
the entire video into memory. On Android, fullscreen playback uses a dedicated WebView
custom-view container with immersive system bars; Back exits fullscreen before leaving
the player. Playback position and speed are remembered per video, completed items can
appear in a Continue Watching shelf, and the player provides double-tap ten-second
seeking, rotation lock, and secure Android picture-in-picture controls.

The default is accessible only from the same device. To expose it to other devices
on your local network, bind all interfaces explicitly:

```sh
cargo run -- serve --bind 0.0.0.0:8080
```

Override the destination when needed:

```sh
cargo run -- serve --output-dir ./my-videos
```

## Hot reload

For development, start the watcher instead of the regular server:

```sh
cargo run -- dev
```

Changes under `src/`, `assets/`, or to `Cargo.toml` trigger a rebuild. A successful build restarts
the Rust server and reloads connected browser pages automatically. If compilation
fails, the last working server stays online while the compiler error is shown in the
terminal. `--bind` and `--output-dir` work in development mode too.

For Android development, the APK watcher rebuilds, reinstalls, and relaunches a chosen
mode whenever Rust, web assets, Java, XML, or Android script sources change:

```sh
sh android/hot-reload.sh normal 10.5.0.2:39271
sh android/hot-reload.sh home 10.5.0.2:39271
sh android/hot-reload.sh result 10.5.0.2:39271
sh android/hot-reload.sh player 10.5.0.2:39271
```

`normal` is the default. The other choices launch only synthetic inspection states;
the watcher never captures a screenshot.

## Self-updates

Sideloaded Android builds can check and download signed RustDL updates in the
background. Configure the HTTPS release-manifest URL when building the APK:

```sh
RUSTDL_UPDATE_MANIFEST_URL=https://downloads.example.com/rustdl/latest.json \
RUSTDL_KEYSTORE=/secure/rustdl-release.jks \
RUSTDL_KEY_ALIAS=rustdl \
RUSTDL_KEYSTORE_PASSWORD='replace-me' \
RUSTDL_KEY_PASSWORD='replace-me' \
sh android/build-termux.sh
```

The default Android version code is derived from the Cargo semantic version as
`major * 1000000 + minor * 1000 + patch`; `RUSTDL_VERSION_CODE` and
`RUSTDL_VERSION_NAME` can override it. Increase the version for every published
release, retain the same production signing key permanently, upload the APK, then
generate and upload its manifest:

```sh
sh android/make-update-manifest.sh \
  https://downloads.example.com/rustdl/rustdl-0.2.0.apk
```

RustDL checks at most once every six hours and only in normal user mode. A newer APK
downloads to private cache without interrupting playback. The update control appears
only after the SHA-256 digest, package ID, higher version code, and APK signing
certificate all match. One tap installs immediately when Android permits it; otherwise
it opens Android's one-time per-source authorization and resumes automatically on
return. Inspection mode never checks for or installs updates.

## Command line

```sh
cargo run -- 'https://x.com/AshtonLaxsma/status/2091257067264733401/video/1'
```

Choose an output file or replace one that already exists:

```sh
cargo run -- -o video.mp4 'https://x.com/user/status/123/video/1'
cargo run -- --force -o video.mp4 'https://x.com/user/status/123/video/1'
```

For X posts, the default filename is `<status-id>-<video-number>.mp4` inside the
platform's `Downloads/RustDL` folder. Existing completed files are treated as duplicates. The
downloader writes to a temporary `.part` file and renames it only after the transfer
succeeds.

Public post metadata is resolved through the third-party FxTwitter API; downloading
private or login-only posts is not supported. Only download media you are permitted
to save and follow the platform's terms and applicable law.

## Source layout

Functions use explicit namespaces at call sites:

- `local::` contains local parsing, formatting, UI, filesystem operations, and app
  state. It does not initiate provider requests or schedule network downloads.
- `external::` contains outbound HTTP and provider integrations. YouTube, X,
  Snapchat, AniWaves, and peer-device requests have separate modules.
- `workflows::` coordinates local state and external operations, including discovery,
  download workers, streaming requests, and server startup.

For example, `local::youtube::video_id(url)` only parses a URL;
`external::youtube::resolve_candidate(url)` requests video metadata and
streams. `local::queue::persist_download_jobs()` saves local queue state, while
`workflows::downloads::start_web_download(...)` resolves a source and starts work.
`local::sources::classify_url(url)` returns a named `SourceUrl` with the parsed
identifier for download/discovery links, or `Unsupported`. Link extraction,
discovery dispatch, playlist routing, and download refresh use this classifier.
AniWaves catalog navigation retains its separate streaming route.
Provider-specific parsers remain under `local::` even when their input came from
an external service. Peer HTTP requests belong under `external::peers`.

| Directory | Main responsibilities |
| --- | --- |
| `assets/html/`, `assets/css/`, `assets/js/` | Page templates, stylesheets, and browser scripts embedded with `include_str!()` |
| `src/local/` | Media serving, UI assets and pages, storage, queue state, runtime hooks, and provider parsers |
| `src/local/pages/` | Gallery, player, download result, settings, diagnostics, and changelog page features |
| `src/external/` | YouTube, X, Snapchat, AniWaves, peer requests, and HTTP downloads |
| `src/workflows/` | CLI/server startup and discovery, download, transfer, and streaming flows |

`src/main.rs` declares these namespaces and delegates to `workflows::cli::run()`.
`src/lib.rs` includes the same application for Android's JNI library and calls its
namespaced runtime hooks. Keep function calls qualified rather than importing
application functions into a flat scope.

`local::pages::<feature>::render(...)` builds HTML without an HTTP request.
Page `respond(...)` functions validate request-specific state and send the result.
Player rendering takes `PlaybackState::Complete` or `PlaybackState::Growing` so
callers state the playback mode explicitly. Avoid repeating a provider name in
its functions: use `external::youtube::resolve_candidate(...)`, for example.
Shared HTML responses and script JSON escaping live in `local::html`; gallery/player
transition names live in `local::web_assets`. Settings keeps its existing renderer
in `local::settings`.

Application regression tests live in `src/app_tests.rs`, alongside focused test
modules. Provider network tests are opt-in and remain ignored in the normal suite.
Run `cargo fmt --check` and `make test` before submitting changes.
After `cargo build --release`, run `python3 tests/integration/download-pool.py`
to exercise 500 local HTTP downloads, pause/cancel, retry, byte-range resume, and
restart recovery in a temporary library. It also checks that thread count stays
bounded and writes measurements to `target/download-pool-integration.json`.

### Editing UI assets

Edit page templates in `assets/html/`, styles in `assets/css/`, and scripts in
`assets/js/`. Rust embeds them at compile time with `include_str!()`, so the CLI
and APK remain self-contained and need no asset directory at runtime. Both reload
watchers rebuild when assets change.

HTML templates use Rust format fields such as `{heading}` or `{page_css}`;
the corresponding renderer supplies escaped text, generated markup, or embedded
assets explicitly. Literal braces in HTML format templates must be doubled.
CSS and JavaScript files use normal single braces. A few scripts contain
`__RUSTDL_*__` markers for values supplied by Rust; preserve the existing escaping
when editing their renderers.

Shared styles and scripts retain their content-hashed immutable URLs. Page-specific
styles and scripts are assembled into their existing inline positions. Small dynamic
markup fragments stay beside the Rust code that renders them.

Gallery refresh regression tests use synthetic media entries only:

```sh
npm --prefix tests/ui install
npm --prefix tests/ui test
```

### Anime episode downloads

Start an episode in the anime player, then tap **Download episode** next to the
watchlist control. Choose a source offered by the player. Downloads run in an
Android foreground service, with progress and a Cancel action in notifications;
completed MP4 files appear in the gallery and the configured Downloads folder.
One episode downloads at a time. The stream player's JavaScript isolation stays
in place: only the native download control can start a download.

Direct MP4 and complete, unencrypted HLS playlists are supported, including
separate audio renditions and fragmented MP4 initialization segments. HLS uses
the highest-bandwidth variant offered by the selected playlist. Encrypted/live
streams, byte-range playlists, and discontinuous timelines are not supported;
the app reports an error so another source can be selected. Failed or cancelled
jobs can be retried from the player; downloads currently restart rather than resume.

Run `sh android/test-anime.sh` in Termux for synthetic playlist and Android media
conversion checks. The tests compare encoded track hashes and never play media.

### Optional development performance tests

`make dev-test` runs the synthetic gallery load, DOM, and scroll-loading benchmarks
under `dev::`. Install the UI dependencies first with `npm --prefix tests/ui install`.
These tests require the explicit Cargo `dev` feature and a test build; they are not
included in the application or normal test runs. See [dev test instructions](src/dev/README.md)
for individual commands and the distinction between synthetic timings and actual
WebView frame rate.

For actual on-screen Android measurements, use `make dev-apk`, install its
`target/android-termux-alongside-dev/rustdl.apk`, then run `make dev-visual`.
It renders discovered synthetic screen fixtures in the real WebView and prints
`PASS`, `FAIL`, `SCROLL`, and `MISSING` coverage lines to the terminal. New/changed
UI sources and an outdated installed APK stop the run instead of testing stale
code. Normal APKs exclude the visual benchmark activity and assets.

## GPUI Android interface

Run `make native-setup`, then `make native-apk` to build RustDL Next with GPUI
navigation, library, settings, queue, discovery, anime, transfers, storage,
diagnostics, updates and playback controls. The Rust engine and Android services
remain shared. Provider WebView content is retained as a restricted streaming
decoder, with native controls; initialization and TalkBack fallbacks remain
while validation is incomplete. See [native-ui/README.md](native-ui/README.md)
for pinned versions, scope and validation limits. The normal `make apk` build now uses GPUI; `make legacy-apk` explicitly builds
the compatibility WebView interface. Device validation is still outstanding.
