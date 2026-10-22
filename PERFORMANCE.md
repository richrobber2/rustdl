# Heap profiling

`sh scripts/profile-heap.sh activity 10` builds and runs a separate optimized,
symbolized executable using pinned DHAT 0.3.3. It requires a Rust toolchain and
the normal engine build prerequisites. On Android, use Termux; root and ADB are
unnecessary. This command does not build or install an APK.

Available workloads:

| Name | Work measured |
| --- | --- |
| `activity` | Build the embedded activity page HTML |
| `changelog` | Build the embedded release-history page HTML |
| `catalog` | Parse 60 synthetic catalog entries with the real parser |
| `queue-500` | Snapshot and serialize activity state for 500 mixed-phase jobs |
| `queue-5000` | Snapshot and serialize activity state for 5,000 mixed-phase jobs |
| `quality-500` | Select and render 500 synthetic items with 12 formats each, including page decoration |
| `playlist-500` | Resolve 500 synthetic entries with the real three-worker loop |
| `playlist-cancel-500` | Cancel once all three initial requests enter the resolver |
| `playlist-page-baseline` | Parse captured playlist HTML using a full JSON Value tree |
| `playlist-page` | Parse the same captured HTML with the incremental section decoder |

The second argument is an iteration count, from 1 through 100. Each invocation
is a fresh process. Fixture creation and CLI setup happen before profiling;
the loop includes creation and destruction of each result. The page and catalog
cases have no warmup, so lazy initialization within the first iteration is
included. Queue warmup is described below. Results are kept
observable with `black_box` and a checksum. Catalog parsing also checks that all
60 synthetic entries were returned.

The queue cases seed a process-local queue and warm its globals **before** the
profiling window. They measure activity refresh/serialization cost, not the
memory already held by those queued jobs. They check that all jobs are counted
while the returned download list stays bounded at 60 rows. The queue contains a
mix of queued, starting, downloading, paused, ready, failed and cancelled jobs.
No queue persistence or Android notifications are invoked.

The playlist cases clone a prepared input into a new owned resolution, start
three scoped worker threads and use a resolver that constructs synthetic
candidate metadata without networking. Successful completion checks all 500
positions, then removes the synthetic published discovery session. The session
registry reserves its small backing capacity before profiling. Cancellation
uses barriers to cancel while exactly three requests are in flight and checks
that no more selections start and no partial session is published. Worker
threads are joined before the iteration ends. Scheduling can vary peak memory;
thread creation and per-iteration resolution/snapshot cleanup are measured.
The production playlist start path, HTTP cancellation handler, extraction,
deduplication and registry eviction behavior are not covered by these cases.

No network, app stores or downloaded media are used. The two playlist-page
cases require `RUSTDL_PLAYLIST_HTML` pointing at an explicitly supplied, captured
HTML response of at most 6 MiB. The fixture is loaded before profiling; parsed
entry fields and continuation token are hashed in order to check equivalence.
These cases process recorded metadata, not synthetic video content.
DHAT records allocation
sizes/lifetimes and code backtraces, not heap contents. The executable finishes
the profiler explicitly and writes a private report under `target/heap-profiles`.
An Android process killed before completion cannot flush its report.

For text-only analysis:

```sh
node scripts/summarize-heap.cjs target/heap-profiles/activity-1234.json
```

Use the actual filename printed by the executable. The summary reports total
allocated bytes, allocation count, bytes live at the global peak, end-of-window
live bytes and the largest allocation sites. Total allocated bytes measure
allocation churn; they are not simultaneous RAM use. End-of-window allocations
can be caches or retained runtime state, and do not alone establish a leak.
Reports stay local; no upload or visual viewer is required.

The feature `heap-profile` gates the dependency and dedicated executable. Its
DHAT global allocator is defined only in that executable. Normal app/CLI builds
retain their allocator. The `profiling` Cargo profile inherits release
optimization and adds debug line information without stripping it, leaving the
normal release profile unchanged. The script uses `--locked`.

This profiles Rust allocations routed through the instrumented executable's
global allocator. It does not measure Java, WebView, the native GPUI library,
GPU/driver allocations, direct C allocation calls or whole-app PSS. DHAT slows
execution, so these runs are allocation measurements rather than timing
benchmarks. Use Perfetto/heapprofd for the installed application's broader
native memory behavior.

DHAT's implementation is experimental; use it in this isolated executable.
See the [upstream documentation](https://docs.rs/dhat/0.3.3/dhat/).

## Activity queue selection (2026-10-22)

Activity snapshots previously cloned and sorted every eligible download and
transfer before returning at most 60 downloads and 30 transfers. They now count
all valid entries, partition borrowed references to select the visible subset,
sort that subset, and clone only the returned jobs. Selection happens under the
existing queue mutex; JSON construction still happens after releasing it.
Scratch references remain proportional to queue size; this is not constant-space
selection. Concurrent lock-contention latency was not benchmarked.

Compared using the same optimized DHAT executable workload and five refreshes
per fresh process, on this Android/Termux device:

| Synthetic jobs | Total allocated, before → after | Peak live bytes, before → after |
| --- | --- | --- |
| 500 | 1,724,280 → 765,835 | 214,459 → 108,242 |
| 5,000 | 17,079,095 → 1,130,035 | 1,871,598 → 108,378 |

For 5,000 jobs this is 93.4% less allocation churn and a 94.2% lower peak.
For 500 jobs the reductions are 55.6% and 49.5%, respectively. All four runs
ended with zero tracked live bytes; serialized-length checksums matched
(54,625 for 500 jobs and 55,025 for 5,000 jobs). These checksums measure lengths,
not byte-for-byte content equality. Counts and visible-row bounds passed the
existing workload assertions. A new synthetic regression test checks full
counts, category/descending-filename ordering, invalid-name exclusion, empty
queues, both production row caps, and exactly one clone per visible job.

`cargo test --locked --profile profiling` passed 98 tests in each of the library
and binary targets, with three existing ignored tests per target. The initial
uncached default-profile build was stopped in favor of this existing optimized
profile. The source formatter check and `git diff --check` also passed. Normal
application allocators and profiles were not changed; no APK was installed.

Before/after JSON reports are `queue-500-24051.json`, `queue-500-26408.json`,
`queue-5000-23944.json`, and `queue-5000-25801.json` under `target/heap-profiles`.
Logs and aggregate comparisons are outside the repository under
`../xerj-test/reports/rustdl-queue*` and `rustdl-opt-comparison.json`.

Xerj helped find this code after refining a broad query; ordinary source reads
were used to verify it. No model token savings were measured. Its incremental
index refresh failed here when hard-linking unchanged snapshot files (EACCES),
also reproduced with an independent synthetic hard-link probe. A fresh index
under a separate prefix succeeded. That refresh limitation and noisy broad
results make ordinary source tools the preferred primary workflow on this
device; indexed retrieval is useful for discovering unfamiliar code.

## Playlist metadata parsing and preview (2026-10-22)

The supplied public playlist was fetched through the updated Rust code: 473
unique entries over five responses, complete in 6.01 seconds in that test.
Only playlist metadata was requested; no per-video format resolution or media
download was started. The generated unminified preview was 52,015 bytes.
The same captured entries passed an offline browser test: every entry was
reachable, selecting/submitting all yielded 473 distinct indices, and at most
32 candidate rows were mounted while browsing. A separate 5,000-entry synthetic
test covers paging, filtering, selection persistence, and submission bounds.

The provider now releases each HTML/JSON response before requesting another
page. The section decoder ignores irrelevant top-level trees and consumes
current lockup items one at a time rather than retaining the entire JSON DOM.
It still allocates a temporary Value for each item and for small header data;
older nested-list compatibility may temporarily retain that wrapper's tree.
It accepts desktop/mobile layouts and JavaScript-escaped initial data without
executing scripts. Repeated continuation tokens and excess entries are rejected.

Three iterations per fresh DHAT process, using frozen first-page captures from
the supplied playlist (100 entries with RustDL's user agent; 20 with mobile):

| Response | Full-tree peak bytes | Section-decoder peak bytes | Peak reduction |
| --- | ---: | ---: | ---: |
| Desktop | 14,011,202 | 351,603 | 97.49% |
| Mobile | 1,992,864 | 538,507 | 72.98% |

Entry-field and continuation checksums matched for each pair, and all runs
ended with zero tracked live bytes. Fixture HTML storage is outside the
profiling window; HTTP buffers, Java/WebView memory, and full-app RSS are not
measured. Both arms use the improved initial-data decoder; the mobile baseline
therefore isolates full-tree allocation, rather than reproducing the previous
failure to accept escaped mobile responses.

This primarily lowers peak heap, not allocation churn or a proven latency
improvement: desktop total allocations over three parses changed from
42,284,157 to 41,635,875 bytes (-1.53%); mobile changed from 8,403,948 to
8,563,860 bytes (+1.90%). The mobile string decoder dominates its remaining
peak. HTML-response and JSON-tree lifetime reductions in the full fetch are
additional structural changes not separately quantified by this parse-only run.

Preview sessions now move the original entry Vec into one shared Arc session;
the ownership test verifies its allocation is retained without cloning entries.
The page embeds escaped compact JSON, releases that JSON DOM node after parsing,
and creates only the current 32 cards. Selection is stored as indices, with
hidden submission fields created only when leaving the preview. All-list
selection and filtering still include off-page entries. Selection sorting on
the server also compares borrowed playlist IDs rather than cloning sort keys.

Validation: 102 Rust library tests passed (four ignored, including the opt-in
network probe), the explicitly requested live probe passed separately, and all
44 browser tests passed against both source and minified scripts. No real
downloaded media, screenshots, or active app-data inspection was used.

Reproduce parsing with a captured response:

```sh
RUSTDL_PLAYLIST_HTML=/absolute/path/to/capture.html sh scripts/profile-heap.sh playlist-page-baseline 3
RUSTDL_PLAYLIST_HTML=/absolute/path/to/capture.html sh scripts/profile-heap.sh playlist-page 3
```

Captures and aggregate results are private local artifacts under
`target/playlist-test/`; `parser-comparison.json` includes fixture hashes.
The opt-in `supplied_youtube_playlist_preview_live` test requires a supplied
`RUSTDL_TEST_PLAYLIST_ID`; ordinary tests do not make its network requests.


## Shared discovery sessions during format preparation (2026-10-22)

Xerj source search located `respond_quality_page` in `src/local/discovery.rs`
and the full candidate-vector clone in `src/workflows/discovery.rs`. The source
index was freshly built from commit `c3897ed`: 280 files walked, 279 indexed,
3,024 records, with `assets/html/watchlist-card.html` skipped as unextractable
XML. Indexing reported 10.5 seconds. Retrieved definitions were checked against
the actual source before editing. The local Xerj node was stopped afterward;
its release binary did not include the separate experimental Xerj patches.

Discovery sessions now own candidates once behind an `Arc`. The discovery
page moves its input vector into that session. Format preparation retains
session references and item indices rather than copying every selected
candidate's format vector and provider URLs, then renders after releasing the
session-map mutex. In-flight rendering retains its immutable data even if the
bounded session map evicts that entry. Selection order, filename deduplication,
quality indices and the existing `token:index:quality` form protocol are retained.
Only the chosen format is still cloned when ownership passes to a download.

Run the closed, offline workload:

```sh
sh scripts/profile-heap.sh quality-500 3
node scripts/summarize-heap.cjs target/heap-profiles/quality-500-<PID>.json
```

The synthetic session contains 500 items with 12 formats each. Stream URLs
contain 2,048 placeholder bytes, and the 11 video formats per item also carry
an audio URL with 1,024 placeholder bytes. Session construction and storage
are outside profiling. The measured scope includes selection, complete page
rendering and appearance decoration. It verifies 500 cards, 6,000 format options,
and a BLAKE3-derived checksum of the complete decorated page. No network,
provider access, actual library inspection, downloads or Android service calls
are involved. These intentionally long synthetic URLs make copy costs visible;
the result is not an estimate for every playlist or overall app RSS.

Three fresh-process DHAT runs per variant, three renders per process, produced
identical totals within each variant and matching page checksums across variants:

| Metric | Previous deep-copy selection | Shared session selection | Reduction |
| --- | ---: | ---: | ---: |
| Peak tracked heap | 23,787,096 | 2,790,940 | 88.27% |
| Total allocated bytes | 100,821,735 | 37,458,699 | 62.85% |
| Allocation count | 136,728 | 52,728 | 61.44% |

Both variants retain 46,238 tracked bytes at the end in 18 blocks. Stack traces
attribute these to lazy appearance CSS and hashed asset-path `OnceLock` caches,
not selected candidates. Fixture memory already stored in the session remains
outside the tracked measurement. No elapsed-time improvement is claimed from
DHAT runs; compilation ran concurrently with some profiling runs.

The profiling executable uses source JavaScript, not the optional minified-js
feature. Results, raw reports and both saved executables are under
`target/quality-opt/` and `target/heap-profiles/`. A baseline instrumentation
patch is saved locally as `target/quality-opt/baseline-instrumentation.patch`:
it introduces the exact same workload and rendering extraction on the original
`c3897ed` implementation, before session sharing. This keeps the measured
baseline's previous candidate copies intact. Normal APK allocator/configuration
is unchanged; the profiling task itself did not package or install an app update.

Validation: `cargo test --locked --profile profiling --lib` passed 105 tests,
with zero failures and four existing ignored tests. This includes the 500-item
HTTP format-page/import test, shared-buffer identity, rendering after eviction,
malformed/expired selection handling, format escaping, and native import quality
and membership preservation. `cargo fmt --all` and `git diff --check` passed.

The tested optimization was subsequently packaged and installed at the user’s
request as `app.rustdl.next` versionCode 1049, versionName
`0.1.45-format-memory`, preserving UID 10200 and app data. APK signature, packaged
library hash, installed APK hash and launch were verified; installation records
are in `../rustdl-install/format-memory-result.json`.

## Page-navigation responsiveness (2026-10-22)

Xerj search of the existing `rustdl-opt2` source snapshot located the duplicated
page transition rules in `assets/css/player.css` and the common page decorator
in `src/local/web_assets.rs`. Those navigation files were unchanged since that
snapshot; current definitions and callers were checked directly before edits.
The temporary Xerj node was stopped after retrieval.

The common appearance stylesheet now owns the same-origin navigation policy.
The configured shared transition window is 160 ms instead of the former 380 ms;
root entry/exit fades are 140/100 ms instead of 280/160 ms. Whole-page scale
animations were removed. Pairing-code movement retains its 100 ms policy.
The cached navigation script is now included once on every decorated app page,
including pages that previously had no navigation feedback. Existing gallery
and player script tags are reused rather than duplicated.

A normal local page link immediately highlights and shows a static, fixed
2-pixel loading cue. It does not consume layout space, delay navigation,
prefetch responses, replace page content, or add requests. It clears on timeout,
page exit and back/forward restoration. Downloads, external/custom schemes,
modified/new-tab clicks, cancelled clicks and same-page anchors retain their
normal handling. Unsupported transition APIs still get navigation feedback.
The shared-thumbnail selector avoids repeating the style writes between a
pointerdown and click on the same thumbnail.

System and app reduced-motion settings retain disabled animations; supported
cross-document transition events additionally skip their transition when motion
is reduced. Chrome's cross-document API requires same-origin navigation and
opt-in on both documents; the implementation follows the
[Chrome cross-document transition guidance](https://developer.chrome.com/docs/web-platform/view-transitions/cross-document)
and feature-detects the optional event transition object.

Synthetic JSDOM interaction tests cover link eligibility, non-interception,
indicator reuse, timeout/page lifecycle cleanup, duplicate-script protection,
reduced-motion events, fallback behavior and avoiding repeated thumbnail writes.
The source and generated/minified UI suites both pass 51 tests; Oxlint passes.
These checks verify behavior and the configured timing policy, not real-device
frame times, GPU memory, perceived latency or a measured reduction in backend
response time. No screenshots or private-media inspection were used.

The Rust library suite passed 106 tests with zero failures and four existing
ignored tests. Build 1050 (`0.1.45-fast-navigation`) was then signed, installed
over the existing app with UID/data preserved, and launched. Signature, packaged
library hash and installed APK hash were verified; installation records are
in `../rustdl-install/fast-navigation-result.json`.

## Synthetic page-generation timing

`sh scripts/benchmark-pages.sh gallery-5000 31` builds an opt-in, optimized,
symbolized timing executable. Despite sharing the `heap-profile` feature's
workload module, this executable uses the normal system allocator, without
DHAT interception. Normal APKs do not include this executable or its fixtures.
Use a fresh process for each measurement and avoid simultaneous compilation,
profiling or other CPU-heavy work when comparing timings.

The timing workloads are `page-settings`, `page-activity`, `page-changelog`,
`gallery-500`, `gallery-5000`, `queue-page-500`, `queue-page-5000`, and
`quality-500`. The gallery creates its own private directory of empty synthetic
files below `target/page-bench-fixtures` and removes that directory on normal
completion or error. It never scans the installed app's library or opens media,
thumbnails or frames. A killed process may leave its synthetic directory behind.
Queue fixtures seed process-local state without workers, persistence or Android
notifications. Gallery and queue cases verify the expected entry/row counts.

Fixture setup is outside timing. The JSON result reports the first render,
then median, nearest-rank p95, minimum and maximum across the requested sample
count after five untimed warmups. Every render includes HTML decoration,
output validation and a BLAKE3 checksum; unequal output aborts the run. `firstNs`
is a fresh-process first render, **not** a cold-storage measurement: creating the
fixture has already warmed filesystem caches. These timings exclude HTTP,
WebView layout/painting, thumbnails, network requests and user interaction.
Use odd sample counts for an unambiguous median. These page workloads also work
with `scripts/profile-heap.sh` for separate allocation measurements; never use
DHAT timings as production latency estimates.

### Gallery shell insertion comparison (2026-10-22)

Filled shell placeholders before inserting the potentially large gallery block.
Previously, five subsequent `String::replace` calls repeatedly scanned/copied
that block, with additional growth allocations when appending the suffix.
The shell's remaining markers inside gallery data now stay literal instead of
being interpreted as template instructions. A synthetic regression covers this
in normal and inspection shell modes.

Three fresh processes per case/version, 31 measured renders per process after
five warmups, normal allocator, identical optimized profiling settings; cases
were deterministically shuffled within each pass. Values below are medians of
the three process medians, measured on this device without concurrent builds.

| Synthetic page | Before | After | Reduction |
| --- | ---: | ---: | ---: |
| Gallery, 500 entries | 5.452 ms | 4.294 ms | 21.2% |
| Gallery, 5,000 entries | 58.990 ms | 48.701 ms | 17.4% |
| Queue, 5,000 jobs (control) | 47.159 ms | 46.976 ms | 0.4% |
| Activity shell (control) | 0.062 ms | 0.062 ms | within noise |

Both gallery output checksums are identical across all before/after timing
runs. Settings, Activity, queue and quality checksums also match. The changelog
checksum intentionally differs because this change adds a release note; its
roughly 0.65 ms generation cost is not an equivalence control. Small control
changes are not claimed as performance gains.

A separate DHAT run of three 5,000-entry gallery renders measured:

| Allocation metric | Before | After |
| --- | ---: | ---: |
| Total allocated bytes | 135,678,547 | 84,982,087 |
| Peak measured live bytes | 20,718,988 | 8,942,201 |
| Total allocation blocks | 301,777 | 301,777 |
| Bytes retained at end | 286,290 | 286,290 |

That is 37.4% fewer allocated bytes and 56.8% lower measured peak heap, with
unchanged block count and end retention. Gallery cache creation and first-use
asset initialization are included in this allocation window; fixture creation
is excluded. This is not total process RSS or an end-to-end UI speed guarantee.
The full index and 32 initial gallery cards are preserved. Raw JSON timing,
comparison, logs and before/after binaries are under ignored `target/page-speed`;
DHAT reports are `target/heap-profiles/gallery-5000-6269.json` and
`target/heap-profiles/gallery-5000-6949.json`.

Validation: 107 Rust library tests passed, with four existing live-network tests
ignored. The shell-marker regression passed in both shell modes. Script argument
rejection, a one-sample percentile check and empty synthetic-fixture parent after
all runs were also verified. Formatting and whitespace checks passed.
