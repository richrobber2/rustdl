# dev:: gallery performance tests

These tests require **both** `cfg(test)` and the Cargo `dev` feature. They are
excluded from normal tests, CLI builds, and APK builds, even if an application is
built with `--features dev`. Each benchmark is also ignored until explicitly run.

Install UI test dependencies once, then run the complete suite:

```sh
npm --prefix tests/ui install
make dev-test
```

`make dev-test` builds the current release server and runs the benchmarks serially
on the binary target, avoiding duplicate runs through the Android library target.
The runner accepts an existing `NODE_PATH` or finds the project's UI dependencies.
Python 3 and Node are required. It never installs tools or dependencies itself.

To run just the scrolling test:

```sh
cargo test --release --features dev --bin rustdl dev::gallery_scroll -- --ignored --nocapture
```

Tests and limits:

- `dev::gallery_load`: first and 30 warm localhost responses at 0, 38, 100, 1,000,
  and 5,000 synthetic entries, plus a request after an import. Starts a separate
  server on port 39080 and cleans up its fixture directory. First request is not
  a cold app launch. Build the release binary first when invoking this directly.
- `dev::gallery_dom`: production gallery JavaScript, one warmup and seven samples
  per size. Checks the initial 32-card limit.
- `dev::gallery_scroll`: three trials at 100, 1,000, and 5,000 entries after a warmup.
  Checks final card counts and compares early/late 32-card batches. Only full
  batches enter per-batch statistics; total append work includes the final batch.

All fixtures are synthetic; no user media is read, displayed, or played. JSON
results are saved under `target/gallery-performance/`. Run benchmarks without
other CPU-heavy tasks. Timings are reported rather than used as flaky pass/fail
thresholds; failures concern execution and correctness.

The Node/jsdom tests do **not** measure WebView layout, painting, GPU frames,
thumbnail decoding, or actual swipe speed. Do not convert their times into FPS
or add them to HTTP timings as an app load-time estimate.

## Actual Android WebView visual test

Build the optional development APK:

```sh
make dev-apk
```

This creates `target/android-termux-alongside-dev/rustdl.apk`. Only
`RUSTDL_DEV=1` adds the benchmark activity and its bundled assets. Normal APKs
exclude them. The Cargo `dev` feature remains for the separate command-line tests.

After installing the development APK, launch the visible test:

```sh
am start -n app.rustdl.next/app.rustdl.GalleryBenchmarkActivity --ez auto true
```

Keep the screen visible and unlocked for approximately 40 seconds. It renders
1,000 synthetic video cards using the production gallery CSS/JavaScript and
synthetic SVG thumbnails in a hardware-accelerated WebView. Three 12-second
passes target 1,000, 3,000, and 8,000 CSS pixels/second. Cards accumulate as in the
real gallery. This is programmatic scrolling, not an injected finger fling.

While the test activity remains open, retrieve anonymous timing counters:

```sh
curl http://127.0.0.1:39090/report.json
```

The report separates requestAnimationFrame cadence from native Window FrameMetrics
(total duration, deadline misses where available, and dropped metric reports).
rAF cadence is not proof that every frame was presented. SVG thumbnail rendering
is real; production thumbnail generation, network delays, and video decoding are
excluded. Runs interrupted by leaving the foreground are marked aborted.

A PixelCopy capture of the synthetic test window becomes available at
`http://127.0.0.1:39090/screenshot.png` after completion. The activity never loads
personal media, blocks WebView navigation/network requests, and serves only its
in-memory synthetic screenshot and counters on localhost. Closing the activity
closes the report server.

### Discovered visual coverage suite

After `make dev-apk` and installation, run `make dev-visual`. It launches the actual
Android WebView test and streams `PASS`, `FAIL`, `SCROLL`, and `MISSING` lines to the
terminal. It returns nonzero for failed cases, interrupted runs, incomplete cases,
or a timeout. Coverage gaps are reported separately and are not hidden by passing
visual checks.

The development build discovers every `assets/html/*.html` template, generates a
synthetic visual-smoke fixture, and runs native sizing plus 320-dp width with 150%
text zoom. It checks document overflow, clipped controls, JS errors, scrolling,
and frame timing. The complete production gallery scroll test runs afterward.

These fixtures do not execute page behavior scripts or real external actions.
Populated list states, specific fixture values, behavior scripts, and native
activity integrations without tests are explicitly listed as `MISSING`. Source
hashes identify the templates used. New templates automatically enter discovery;
unmapped stylesheet/fixture needs are reported. A rendered fixture is not claimed
as complete functional coverage.

Full counters and gaps are saved to `target/gallery-performance/visual-suite-report.json`.
Keep the screen visible for approximately two minutes. The localhost report stays
available while the development activity is open. Normal APKs exclude this suite.

Before launch, `make dev-visual` verifies the installed APK hash and hashes of UI
sources captured during the development build. It refuses stale builds and runs
clipping-detector regression checks. Horizontally scrollable controls count as
reachable; controls wider than their scrolling container still fail. Clipped
control reports include a short synthetic label for identification.

### Actual gallery, metrics only

Build/install `make dev-apk`, then run `make dev-real-gallery`. This explicitly
instruments MainActivity in the staged development build and opens the actual
local gallery. There is no screenshot capture or media export in this path.
The Java bridge accepts only a fixed list of numeric counters and an abort flag;
no filenames, URLs, page text, image pixels, or media contents are reported.

Keep the gallery visible and untouched for approximately 50 seconds. Three
15-second passes use the same 1,000 CSS-pixel/second target: initial images-visible,
repeat images-visible, then images temporarily hidden with layout preserved.
The image style and scroll position are restored. Existing thumbnail caches are
not cleared. Resource timing includes initial-page thumbnail requests in the
first pass; load/error event counters begin when the observer attaches. Queue
counters are global to the app and can include concurrent work.

`target/gallery-performance/real-gallery-metrics.json` contains the anonymous
report. Visible frame cadence, native window timings, thumbnail request latency,
HTTP errors where supported, load/error events, pending/broken image counts, and
thumbnail cache/queue counters are reported separately. Repeated passes are
ordered and progressively warm caches, so differences alone do not prove cause.

### 500-item playlist rendering

After building and installing `make dev-apk`, run:

```sh
python3 src/dev/run-visual.py --batch500
```

This runs six populated synthetic cases in the actual WebView: playlist selection,
format selection, and queued downloads at native size and 320 CSS pixels with
150% text. Keep the test visible for approximately one minute. It checks production
Select all, filtering, Select visible, Clear, First 10, and bulk format changes,
checks every 32-item page, then scrolls and checks clipping and access to the final entry. Reports
include interaction timing, rAF intervals, long tasks, and native frame metrics.
Layout/interaction pass status is separate from frame timings; it does not imply
a smoothness threshold was met. Queue actions, provider resolution, video decoding,
and real network downloads are excluded.

Results: `target/gallery-performance/batch500-visual-report.json`.

### Clipping and truncation audit

`make dev-visual` now runs `visual-audit.js` in the actual Android WebView. It checks
horizontal document overflow, controls/media clipped by nested ancestors (both
axes), text rectangles clipped by containers, hard text truncation, completely
obscured visible controls, and broken loaded images. It samples the top and bottom
of smoke fixtures and every page of the populated playlist/quality cases. A
scrollable container is allowed only when its item fits; outer clipping still
counts. Hidden elements and hidden ancestors are excluded.

Hard clipping fails the case. CSS ellipses and line clamps are reported as
warnings for review. A deliberate truncation can be documented with
`data-visual-allow-truncation="reason"`; it remains visible as an allowed finding.
`data-visual-audit-ignore` excludes a subtree and should be reserved for fixture
infrastructure. Counts remain in the report even when detail limits are reached.
Counts in merged samples may include the same issue at multiple checkpoints.

The first case is an intentionally broken detector canary: it passes only if the
WebView detects the expected defects and accepts a reachable scrolling control.
Smoke templates run at fixed 280, 300, 320, 344, 360, 375, 393, 412, 414, 428, 480, 540, 600, 640, 720, 768, 800, 900, and 1024 CSS-pixel widths, plus a 150% text case and fixed-width light/dark Rainy City cases. Production behavior remains
excluded from smoke fixtures; populated playlist/quality interactions are tested
separately. Allow several minutes with the phone unlocked.

The runner writes JSON and a standalone HTML report under
`target/gallery-performance/visual-suite-report.{json,html}`. Up to twelve failing
synthetic windows are captured before navigation using PixelCopy and downloaded
to `target/gallery-performance/visual-failures/`; available images are embedded in
the HTML report. Capture failures do not turn a failed layout into a pass. Report
selectors, labels, rectangles, ancestor selectors and scroll coordinates identify
what needs fixing. The harness does not load the user's media.

Run detector logic checks without installing an APK:

```sh
NODE_PATH=target/ui-tests/node_modules node --test src/dev/visual-audit.test.cjs src/dev/visual-smoke.test.cjs
```

These Node tests use explicit geometry mocks, not browser layout. Actual visual
validation requires installing the freshly built development APK and running
`make dev-visual`; stale builds are rejected. This geometry harness does not prove
contrast, aesthetic quality, native control text painting, partial occlusion,
video rendering, or every dynamic app state. Screenshots support manual review.


Singular detector coverage

Pages and fixtures may mark an individual control or surface with
`data-visual-audit-required="reason"`. The geometry audit reports it as
`missingCoverage` when the element is visible but no detector exercised it. These
review findings appear in JSON and the standalone HTML report; they do not pass or
fail layout by themselves. Use `data-visual-audit-ignore` only for fixture
infrastructure. This makes a newly added one-off element visible to reviewers
instead of silently disappearing from detector coverage.


The matrix includes widescreen monitor profiles through 3840 CSS pixels. Android
WebView cannot physically expose a viewport wider than the device screen, so the
development activity caps oversized profiles at the device width. Those profiles
are still present in the manifest for desktop WebView/Browser execution; on-phone
results identify the effective viewport in each case.


Orientation coverage includes portrait and landscape profiles. The activity requests
the matching orientation before each case and records the effective viewport in the
case result. Landscape profiles still use fixed CSS widths; the device may cap a
profile larger than its physical display.
