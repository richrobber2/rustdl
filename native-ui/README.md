# GPUI Kit Android interface

The target is to replace RustDL's full app interface with GPUI Kit while
keeping the Rust download engine and Android playback services. The current
migration renders home, navigation, settings, download queue, diagnostics, storage, Activity Center, update controls, and release history with GPUI.
Library Send to Device opens native pairing and encrypted transfer controls,
with redacted progress and refresh limited to the visible active screen. Native
receiving controls renew expiring QR pairing, hide pairing details during inspection,
and copy them only on an explicit user action. Pairing deep links enter native transfers.
Native discovery offers paginated candidates, batch selections across pages,
format cycling, and imports through the shared Rust workflow. Playlist discovery also provides native selection, bounded preparation
progress, cancellation, and continuation to formats.
Library cards, search, playlists, sharing, watched markers, deletion, discovery,
and tool navigation use GPUI. The library tool links open native destinations
instead of a retained WebView library. Anime catalog browsing and
watchlist edits, paginated episode selection and the release calendar use GPUI.
Streaming selection and the isolated provider controller use GPUI; the provider
WebView resolves provider media requests offscreen and is cleared after a validated HLS/MP4 handoff. A RustDL-owned Android MediaPlayer and privacy-gated surface decode the selected stream, with GPUI Play/Pause and seek controls and native Android transport fallback. The owned stream surface supports secure fullscreen and aspect fitting; track selection and actual provider playback still need validation and parity review.
Local library and queue playback now open GPUI play, pause, a draggable seek timeline, exact seeking, speed, volume, mute, Up Next, sleep timers, rotation, fullscreen, sharing and source/device actions
with a privacy-gated Android video surface. Scrub frames are extracted from
validated local sources on a bounded worker, remain in a secure Android panel,
and cancel on privacy, media replacement, PiP, navigation and seek release.
Growing-download previews are limited to reported available ranges. Browse with mini-player keeps the real
native library visible alongside Play/Pause, Next, PiP, Expand and Close controls;
Back expands the player. A native Android media session exposes transport controls
and numeric playback state. Cached video artwork is decoded with bounded size and
track/lifecycle guards outside inspection privacy; privacy changes immediately
clear artwork and titles. Media paths never enter session metadata. The native foreground service owns the decoder and transport session, using
metadata-free notification controls. Its notification returns to the current
player without reopening media or autoplay. Background lifecycle and recovery
parity still need validation.
Growing-download seeks use the decoder’s reported buffered
percentage, and GPUI shows buffering status. Growing decoder errors can retry up to
three times after download progress advances, with a five-second delay and preserved
actor-held position and play intent. Background and recovery parity remain under review. The staging script adds buffer callbacks only to the build copy of
the pinned video helper, leaving its checkout unchanged. Local playback controls stay in GPUI; the manual legacy-player detour has been
removed after auditing its controls against native transport, scrub preview,
queue, metadata and lifecycle paths. Device validation and accessibility coverage
remain outstanding migration work, so this is not a full completion claim. Leaving the native player hides its surface while the foreground service
retains playback ownership. It does not recreate the WebView. The mobile platform retains its render thread across Activity
recreation and reattaches surfaces.

The native home receives appearance and aggregate transfer counters. Native
settings receives a typed preference snapshot and saves through Android
validation. Folder edits and reset confirmation use system dialogs; no settings
control requires WebView. Native app screens use the bundled space and light/dark rainy-city backgrounds.
Rainy City follows native scroll position and uses the bundled forward/reverse
sunrise sequence on theme changes. Reduced motion disables those animations.
Space decoration respects the saved effect and reduced-motion preferences;
the isolated provider decoder retains its own background.
Home, settings, and release history do not receive downloaded-media metadata.
Native library rows use the shared cached gallery model, pages of 25 records,
and opaque handles. Private titles, filenames, and thumbnail references are
omitted from the bridge while inspection privacy is enabled. Search uses a
system text dialog, and deletion uses a confirmation dialog. Cards show saved
playback progress and watched state through opaque handles; marking watched
refreshes the native page. Search and filter offers Continue Watching, selected before pagination and
ordered by recent activity. Up Next filtering and add/remove actions share a validated, bounded Android
queue with the existing player, merging legacy browser state once. Native screenshots
remain secure until a GPUI frame acknowledges redacted rendering; stale frame
acknowledgements, fullscreen, and picture-in-picture are rejected.
The queue uses an in-process Rust bridge, opaque item handles, and pages of at
most 25 records. While inspection privacy is enabled, its Rust snapshots omit
filenames, source URLs, provider errors, and media paths. The native renderer
also conceals names when privacy is enabled or unknown. Pause, resume, and
cancel run through the existing engine on a bounded Android worker. Progress
refreshes from transfer notifications only while the native queue is visible.
Queue notification intents open the native queue. Queue playback and its transport tools stay in the native player. The destination folder preference is hidden while
inspection privacy is enabled.
Release history uses the existing public changelog at build time and renders
five releases initially, adding eight per request. It does not parse HTML or
fetch release notes at runtime. Both the native Back button and Android Back
return to native home. Remaining command IDs resolve to fixed local routes
in Java while those screens are being migrated. State changes use a bounded
notification channel. Diagnostics collects existing Android metrics on a
preference-paced timer only while visible and resumed; it can export its
snapshot through the existing clipboard bridge.
If native initialization does not reach its first frame, the app falls back
to the library. Touch exploration also keeps the existing WebView interface
because this mobile GPUI platform does not yet provide a verified TalkBack path.

## Build

```sh
make native-setup
make native-apk
```

The APK is `target/android-termux-alongside-gpui/rustdl.apk`, using the
existing RustDL Next package. The normal `make apk` build now compiles GPUI for the standard package at
`target/android-termux-gpui/rustdl.apk`; `make native-apk` selects RustDL Next.
`make legacy-apk` retains an explicit compatibility build. Device and TalkBack
validation remain incomplete, so the source switch is not a runtime validation claim. Building both native
and development flags produces a `-dev-gpui` build directory.

`Cargo.lock` and Git revisions pin GPUI Kit's Android branch and its compatible
mobile platform. The sibling `../gpui-android` project supplies the established
Termux platform reference, but uses an older GPUI core. The two cores must not
be linked as interchangeable platforms. Java helper classes are copied from
Cargo's exact resolved mobile source during packaging.
The native builder uses a local `cc` wrapper to keep LLD linking on one thread.
This applies to host build scripts and the Android library after the
intermittent linker crashes encountered in Termux, without changing device
security settings or discarding compiled dependencies.

## Verification

```sh
python3 tests/integration/test_native_home.py
cargo fmt --manifest-path native-ui/Cargo.toml --check
```

Compilation and APK verification establish packaging and API compatibility.
Device validation should exercise taps, keyboard focus, Back, background and
foreground transitions, rotation, and appearance before promoting this preview
to the default build. Follow `AGENTS.md` for inspection: use synthetic data and
keep downloaded media concealed before any screenshot. Native rendering alone
does not establish a startup, frame-time, or battery improvement.

Android APK builds compile the GPUI interface with Cargo’s optimized release profile. The first release build creates a separate dependency cache; debug artifacts are retained.

Synthetic Android visual review uses the same GPUI renderer in a separate
`:native_visual` process. It starts no RustDL server, provider requests, decoder,
or library reads. Fixtures always enable inspection privacy and contain no
remote artwork. With a specifically authorized wireless ADB endpoint:

```sh
python3 scripts/native-visual-review.py --serial HOST:PORT --screen anime --output target/native-visual/anime.png
```

Supported screens are home, history, settings and anime. The capture trigger
waits for the current private frame acknowledgment and Android presentation
frames, and checks its shell-only synthetic readiness status before publishing the PNG.
The status provider requires Android DUMP permission and exposes no private files
or settings, so production APKs remain non-debuggable.
Real app screenshots still require Allow screenshots plus Inspection privacy
saved and rendered; never capture media fullscreen or PiP.
