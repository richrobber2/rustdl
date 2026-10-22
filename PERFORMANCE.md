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
