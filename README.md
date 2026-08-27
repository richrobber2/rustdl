# rustdl

A Rust CLI, web page, and Android app for downloading MP4 video from public X posts,
YouTube videos, and Snapchat Spotlight. The server, HTML, and download handling are all hosted by the Rust
binary; no Node, Python, JavaScript, `yt-dlp`, or `ffmpeg` is needed.

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
signature. The result is `target/android-termux/rustdl.apk`.

Install it with Android's package installer or with ADB:

```sh
adb install -r target/android-termux/rustdl.apk
```

The app embeds the Rust server in a native library and displays its UI in a secured
localhost WebView. Paste a link normally, or use **Share → Download with RustDL**
from X, YouTube, or Snapchat to begin a download immediately. Android's MediaStore publishes completed files to
`Downloads/RustDL` without broad storage permissions; an app-private copy backs the
built-in player and duplicate cache.

Downloads are progressive: once the upstream MP4 headers arrive, RustDL opens the
player and streams from the growing `.part` file while the background worker keeps
writing it. Requests that reach the write edge wait for the next chunk, then resume;
completed files still use atomic rename, duplicate detection, byte-range seeking, and
MediaStore publication. The persistent Smart Queue keeps partial files and resumes
them with HTTP byte ranges after a network interruption or app restart. It runs at
most two transfers concurrently and exposes pause, resume, retry, cancel, progress,
and direct-play controls. Paste up to 50 links at once or share text containing
multiple X links; duplicate links and already-completed videos are not added twice.
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
sessions are short-lived and capped at 50 videos.

YouTube Shorts, watch, embed, live, and `youtu.be` links resolve through the
Rust-native `rustypipe` extractor. RustDL offers each progressive MP4 quality that
already contains both video and audio, so no Python or FFmpeg runtime is required.
Signed YouTube stream links are refreshed automatically before a resumed transfer
when they are close to expiry while retaining the selected resolution. Private,
paid, age-restricted, and region-blocked videos are not bypassed.

YouTube `/playlist?list=…` links are also supported. RustDL follows playlist
continuations to load the full public entry list, then opens a searchable selection
screen without resolving hundreds of media streams up front. Choose individual
entries, the first 10, or up to 50 visible results per queue batch; nothing starts
until the selected entries are resolved and the user chooses video quality or
audio-only for each one. Selected entries retain their playlist title and original
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
downloads for fast duplicate scans, while worker count and I/O buffers adapt to the
phone's network, charging, power-save, thermal, storage, and CPU conditions.

Run `make help` for the short command list. `make check` verifies the Android toolchain
without modifying the device, `make test` runs the optimized Rust suite, `make run`
starts the web app, and `make dev` starts hot reload. Advanced builds can set
`ANDROID_JAR=/path/to/android.jar`. Generated platform files and signing keys remain
local and are ignored by Git.

### Live diagnostics

The normal-mode **Diagnostics** page samples Android APIs from inside the installed
APK every three seconds while visible. It reports battery level, charging state,
battery temperature, Android thermal severity, normalized CPU load, available/total
memory, available/total internal storage, device uptime, and the RustDL app process.
A manual refresh and copyable JSON snapshot are available for troubleshooting. No
ADB process, external executable, privileged receiver, or setup step is required.

Diagnostics intentionally exclude logcat, notifications, media filenames, Wi-Fi
identity, location, other-app enumeration, and screen content. The route and Java
bridge are unavailable in inspection mode. Sampling pauses whenever the page is
hidden and resumes when it becomes visible.

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
user-mode windows use Android's secure-window protection, which blocks screenshots
and display capture. Only the isolated synthetic inspection UI is renderable by the
guarded inspection workflow.

The home screen also exposes an explicit mode control. **Preview safe UI** launches
the isolated synthetic task, while **Return to my gallery** returns to the secure
normal task. User videos and generated thumbnails are never mounted into inspection
mode.

## Web page

```sh
cargo run -- serve
```

Open <http://127.0.0.1:8080>, paste one or more X links into the input, and select
**Add to queue**.
On Android, videos are saved to `/sdcard/Download/RustDL`. The status ID and video
number form a stable filename, so submitting the same link again detects the existing
file and skips the duplicate download.

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

Changes under `src/` or to `Cargo.toml` trigger a rebuild. A successful build restarts
the Rust server and reloads connected browser pages automatically. If compilation
fails, the last working server stays online while the compiler error is shown in the
terminal. `--bind` and `--output-dir` work in development mode too.

For Android development, the APK watcher rebuilds, reinstalls, and relaunches a chosen
mode whenever Rust, Java, XML, or Android script sources change:

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

The default filename is `<status-id>-<video-number>.mp4` inside the platform's
`Downloads/RustDL` folder. Existing completed files are treated as duplicates. The
downloader writes to a temporary `.part` file and renames it only after the transfer
succeeds.

Public post metadata is resolved through the third-party FxTwitter API; downloading
private or login-only posts is not supported. Only download media you are permitted
to save and follow the platform's terms and applicable law.
