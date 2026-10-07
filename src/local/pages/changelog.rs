//! Changelog page rendering and responses.

use super::super::super::local;
use std::error::Error;
use tiny_http::Request;

pub(in super::super::super) const CHANGELOG: &[(&str, &[&str])] = &[
    (
        "0.1.45",
        &[
            "Restored the CI Rust test run by moving the synthetic Taffy scroll-layout fixture out of Cargo's auto-discovered integration tests, fetched the pinned, SHA-256-verified Android API 35 platform jar in CI so native navigation and poster-policy checks compile, and pre-fetched the locked native UI crates so the offline accessibility and scroll-layout fixtures resolve on clean runners.",
            "Gave every native GPUI screen the same top header with Back and title; Storage, Activity Center, Updates, Discover downloads, Anime and Device transfers no longer place Back at the bottom of the page and now draw the shared translucent page surface over the background.",
            "Restyled native Queue, Library, Storage, Activity and Discovery items as bordered cards with muted secondary text, compact wrapping action rows (Open player, Play and Resume emphasized, Delete marked as destructive) and Previous/Next/Refresh grouped on one row instead of stacked full-width buttons.",
            "Showed native empty and loading states inside cards, grouped Storage usage and cleanup actions, Activity status and Diagnostics metrics into cards, colored every native action and load error with the theme's danger color, and laid out Home and Library tool links in a two-column grid.",
            "Replaced numeric native screen identifiers with a typed screen enum that keeps the existing Java and accessibility values, and moved shared page, header, card, text and button-row styling into one module; inspection-privacy masking and semantic text identifiers are unchanged.",
            "Used Android’s timestamped velocity tracker and OverScroller for native anime pans and release momentum, retained one physical-to-logical distance conversion, stopped flings on new touches or lifecycle/navigation changes and rejected stale scroll callbacks.",
            "Kept native anime collage cards in stable columns when cached poster dimensions arrive, preventing asynchronous previews from reassigning tiles during scrolling while preserving each thumbnail’s proportions.",
            "Arranged native anime previews in responsive staggered collage columns using cached poster dimensions to retain each image’s original proportions without cropping; retained episode and watchlist actions, inspection-safe placeholders and mixed-ratio synthetic review fixtures.",
            "Grouped native Settings into Downloads, Appearance and motion, Playback, Privacy and Maintenance cards without changing setting actions or inspection masking; separated virtualized release notes into bordered cards for easier reading.",
            "Improved native Home and Anime layouts from privacy-safe Android captures: emphasized the library action, separated Watch, Queue and Tools cards, clarified headings and supporting text, paired anime navigation and strengthened item-card contrast while retaining all controls and private metadata masking.",
            "Kept the isolated synthetic review activity awake during visual checks without changing the main app screen-awake preference.",
            "Exposed only isolated synthetic visual-review readiness through a DUMP-permission status provider so ADB captures work with non-debuggable release APKs without private-file access or weaker production settings permissions.",
            "Added an isolated synthetic GPUI visual-review activity with closed screen fixtures, private frame acknowledgment and no engine, provider, media or saved-library access; corrected isolated-controller privacy acknowledgment to use its own page privacy.",
            "Required the current native privacy revision to acknowledge its rendered frame before releasing screenshot protection, including the isolated anime controller instead of its main-activity settings defaults.",
            "Made normal native and owned-anime windows honor Allow screenshots independently of Inspection privacy, retained redaction-before-release frame delays and fullscreen/PiP protection, and rechecked isolated-process consent before clearing secure flags.",
            "Added secure fullscreen and aspect-preserving layout for the owned anime video surface with native exit and transport controls, restored the normal surface on Back/background/privacy changes, retained screen-awake behavior on the visible owned surface, and blocked provider-page fullscreen requests.",
            "Cleared the offscreen anime resolver after media handoff to stop provider-page playback, preserved observed media referer and cookies, retained accessible Android transport fallback, paused the owned decoder on backgrounding and switched server recovery to actual decoder failures rather than page-load completion.",
            "Connected resolved anime HLS/MP4 requests to a RustDL-owned Android decoder and private-gated video surface, forwarded app-runtime cookies and referer, kept provider WebView resolution offscreen, suppressed its media requests, and added native playback status, Play/Pause and ten-second seek controls.",
            "Constrained the native anime catalog, watchlist, calendar and episode scroller to the actual viewport instead of flex-growing inside a non-flex parent, retaining touch and accessibility scroll state so long lists overflow within the screen.",
            "Separated native anime catalog and episode reads from long-running download and streaming actions with a dedicated bounded worker, discarded superseded queued reads, retained serialized mutations and stopped the read queue on activity destruction.",
            "Kept native anime refresh available during loading and failed catalog states, and distinguished provider timeout, connection, rejection and unreadable-response failures using closed privacy-safe messages with synthetic regression checks.",
            "Loaded native anime posters through a two-worker bounded Android cache with HTTPS validation, byte and decode-size limits, immediate private suppression and generation-checked publication; explained inspection-hidden video frames in native decoder controls.",
            "Handed native anime server selections to the restricted decoder through their cached manifest, rejecting expired or mismatched selections instead of resolving rotating provider addresses twice; added synthetic handoff checks and explicit expired-selection feedback.",
            "Rendered anime posters in native cards when inspection privacy is disabled, retained private artwork suppression, and distinguished a loaded player page from verified video playback in decoder status.",
            "Virtualized native release-history rows using retained variable-height list state, measuring and rendering only visible notes while preserving expansion, ordering, scroll position, background parallax and semantic scrolling, with a synthetic expansion regression test.",
            "Loaded native release notes in eight-note batches per release, with retained expansion and remaining counts, so a large current release does not lay out all of its notes when What’s new opens.",
            "Packaged the GPUI Android interface from Cargo’s optimized release profile instead of the unoptimized development profile, retaining pinned sources, the single-thread linker workaround and dependency caches.",
            "Stopped requesting redundant native privacy-acknowledgment frames after an unchanged revision has rendered, retaining protected-frame and stale-revision checks while reducing frame-source wakeups.",
            "Retained the existing accessible Android streaming controls when TalkBack touch exploration is enabled, matching the main activity’s required fallback until native provider geometry, actions, privacy and lifecycle are validated on-device.",
            "Ported Rainy City scroll parallax and the bundled reversible sunrise/sunset frame sequence to native app backgrounds, preserving reduced-motion behavior and cancellation on rapid theme changes without loading user-media artwork.",
            "Cleared the isolated streaming host’s cached accessibility nodes immediately on privacy or source-generation changes and invalidated queued semantic actions before replacing decoder controls.",
            "Bounded the owned GPUI semantic action queue and added execution-time generation checks so queued actions are rejected after privacy, navigation or accessibility lifecycle changes, without modifying pinned dependency checkouts.",
            "Routed Android shared video links into native discovery and its bounded typed worker instead of WebView discovery, retaining the legacy route only when native initialization or accessibility fallback requires it.",
            "Switched normal Android setup, prerequisite checks and APK builds to GPUI by default, retained explicit legacy APK and WebView development builds, and updated package paths and documentation without removing initialization or TalkBack fallbacks.",
            "Retained saved-audio M4A and saved-video MP4 format labels in native playback status and corrected the main documentation to describe the full native screen migration and its remaining validation limits.",
            "Removed the native player’s manual WebView playback detour after porting its transport, preview, queue, metadata and lifecycle controls, keeping library and queue playback inside GPUI while retaining initialization and accessibility fallbacks.",
            "Added native Up Next queue counts using the same validated and deduplicated saved selections as transport Next, distinguished empty queues from gallery fallback, and corrected the Next label for both selection paths without exposing media identities.",
            "Limited native transport artwork to one pending decode per track generation and added synthetic stale/privacy/lifecycle and decode-size checks to prevent overlapping retries and late private publication.",
            "Connected cache-only artwork to service-owned Android playback metadata with bounded sampled decoding, track and lifecycle generations, immediate inspection-privacy redaction, and stale-result rejection without exposing artwork paths to GPUI.",
            "Added a cache-only native transport artwork resolver that rejects inspection-private and audio-only selections, invalid media identities, symlinks and oversized cached files without generating thumbnails or contacting providers.",
            "Rejected activation and value-changing accessibility actions for fully clipped controls until their visible bounds return, with synthetic scroll-viewport checks retaining normal visible interaction.",
            "Completed semantic text for native release, pagination, home transfer and player frame/download messages, retaining existing metadata masking and page counts for screen-reader context.",
            "Added identified streaming-selection and isolated-decoder status text and actual semantic scrolling in both native streaming interfaces, keeping protected titles and decoder addresses behind their existing privacy boundaries.",
            "Added semantic anime, discovery and device-transfer status text and real accessibility scrolling, keeping media fields inspection-redacted, candidate rows opaque and repeated anime detail fields individually identified.",
            "Added readable semantic activity, storage and update status content with opaque row identities, distinct storage metric labels and actual accessibility scroll actions, retaining all existing privacy projections and user confirmations.",
            "Connected native player control scrolling to the same actual accessibility scroll-handle path as other native pages, allowing screen readers to reach transport and playback options below the video frame.",
            "Added semantic text identities to native player and mini-player titles, playback status, volume, progress, transfer details, buffering and sleep messages while preserving their existing inspection-privacy guards.",
            "Made native settings status and diagnostic metrics readable as identified semantic text and connected home, settings and diagnostics scroll containers to actual accessibility page-scroll actions.",
            "Connected accessibility page-scroll actions to actual GPUI scroll handles in release history, library and queue, retaining scroll offsets, bounded viewport movement and clipped semantic hit targets.",
            "Added identified semantic library and queue row text for redacted titles, status, progress, watched state and guarded quality/error details, using opaque per-row identities to keep repeated labels distinct for accessibility.",
            "Exposed native home headings, queue status, navigation errors and the privacy-projected library heading as identified semantic text, allowing screen readers to announce page context as well as buttons.",
            "Gave native release versions and notes explicit semantic text identities so Android accessibility can read release content, preserving per-entry identities and the existing bounded history loading.",
            "Used GPUI’s already-scaled physical semantic bounds directly for Android accessibility instead of multiplying display density twice, preserving hit targets at non-unit device densities.",
            "Captured native accessibility screen and privacy revisions as one coherent context and skipped unlabeled layout containers during touch exploration, retaining labeled content and actionable controls.",
            "Included synthetic native semantic-tree, action-validation and clipping checks in the regular native integration suite, using cached dependencies without querying app accessibility or media data.",
            "Clipped native accessibility descendant bounds to semantic scroll containers, with synthetic geometry checks so touch exploration cannot target content outside its visible container.",
            "Preserved selected, checked and mixed semantic states in Android accessibility nodes, mapped toggle controls to platform classes and exposed both keyboard and accessibility focus without duplicating control actions.",
            "Requested a fresh native semantic tree after bounded accessibility update-queue overflow, clearing stale Android state before recovery instead of leaving accessibility silently inactive.",
            "Added Android virtual accessibility nodes and touch exploration to both GPUI hosts, forwarding existing semantic actions with stable virtual identifiers, bounded snapshots, lifecycle cleanup and native slider ranges while retaining the TalkBack fallback pending validation.",
            "Connected semantic accessibility snapshots and validated action forwarding to JNI for both native processes, rebuilding semantic state on navigation and inspection-privacy changes without exposing raw debug trees.",
            "Validated native accessibility actions against current screen and privacy context, visible enabled nodes and advertised capabilities, rejecting malformed identifiers and non-finite or out-of-range slider values with synthetic action tests.",
            "Projected native semantic nodes into explicit Android accessibility fields with composed finite bounds, bounded text, hidden-subtree exclusion and supported actions, testing geometry and excluding debug identifiers and provenance.",
            "Rebuilt only the owned accessibility adapter crates in Cargo’s normal dependency graph to keep core semantic exports consistent, preserving all other dependency caches and rejecting duplicate semantic node identifiers.",
            "Added bounded native accessibility semantic state with synthetic tests for removed-node pruning, privacy and navigation invalidation, malformed trees, and oversized updates before connecting Android node providers.",
            "Included the pinned core documentation in its owned Android accessibility build copy so relative compile-time documentation imports remain intact without modifying the dependency checkout.",
            "Forwarded actual Android semantic tree updates through the owned accessibility adapter, retaining node geometry and actions and tagging updates with screen and privacy revisions instead of relying on debug tree snapshots.",
            "Added accessibility activation and semantic-action forwarding to a RustDL-owned build copy of the pinned Android window backend, keeping dependency revisions and checkouts unchanged while preparing the TalkBack bridge.",
            "Routed native UI Next through the same serialized service selection as system transport controls, using current actor state and validated saved-queue/gallery routes instead of reopening through stale Activity selection state.",
            "Reported actual decoder audio and caption track counts in native playback using only RustDL’s staged helper copy, retaining track details without exposing language metadata or modifying the pinned dependency checkout.",
            "Added native player download-byte totals and quality details through a narrow cached-engine projection, omitting provider addresses and errors and clearing quality on late inspection-private callbacks with synthetic projection tests.",
            "Added bounded, debounced native scrub-frame previews with requested-time labels from validated local playback sources, keeping frames in a secure Android panel and cancelling stale results on privacy, navigation, media replacement, seek release, and decoder destruction.",
            "Restored the 0.75× native playback speed choice so the GPUI rate cycle includes every speed offered by the existing player.",
            "Updated migration documentation to distinguish native streaming controls from the restricted provider decoder and describe service-owned playback, notification return, and remaining lifecycle validation.",
            "Added a metadata-free playback notification return action that binds the native player to the existing service session without reopening media or autoplay, restoring private internal selection and current decoder snapshots to a recreated Activity.",
            "Removed obsolete Activity-owned recovery workers and counters after transferring growing-download transitions to the service decoder actor, leaving one recovery owner across foreground and background playback.",
            "Emitted internal playback selection notifications only when the decoder track changes, preventing periodic old-track snapshots from invalidating pending UI opens.",
            "Moved growing-download completion and bounded recovery checks to the service-owned decoder actor, using its current position and play intent during background playback and avoiding duplicate Activity-driven transitions.",
            "Cancelled pending native decoder opens when leaving or hiding the player before service binding completes, restricted delayed surface visibility to player layouts, and reported foreground-service start failures through a generic playback error.",
            "Advertised system Next availability from the same saved-queue and cached-gallery selection used by the service transport action, retaining gallery navigation after Up Next is empty.",
            "Synchronized service-owned track changes with native UI generations through a private selection callback, clearing obsolete share/source/recovery targets before resolving current opaque actions without adding media identity to GPUI snapshots.",
            "Handled native service Next commands on the serialized decoder actor, prioritizing saved Up Next before cached gallery order and consuming queued items only after local route validation without requiring an Activity.",
            "Bound native playback to the foreground decoder owner, moved media-session updates to service polling and weak UI callbacks, and hid video surfaces during UI inactivity while retaining playback ownership.",
            "Added a native-build foreground playback service that owns the decoder and application-context preferences, exposes bound playback operations, and uses metadata-free notification controls while preparing UI lifecycle transfer.",
            "Removed the native decoder’s strong Activity ownership, acquired audio management from application context, and gated surface attachment and snapshot delivery on a live UI while preparing background-service ownership.",
            "Separated playback preference storage from Activity lifetime using application-context preferences and weak UI delegation, preparing service-owned native playback without retaining destroyed Activities.",
            "Kept native playback keyboard shortcuts from consuming keys while a native control is focused, preserving button and slider keyboard interaction with synthetic focus checks.",
            "Saved native playback position at two-second intervals during playback and added Space, Left/Right and N keyboard controls to expanded and mini-player layouts without taking modified shortcuts.",
            "Entered native PiP when leaving an actively playing video, using the existing privacy, lifecycle, audio-only and platform gates while keeping paused or inspection-private playback out of PiP.",
            "Applied saved public background artwork and reduced-motion-aware space decoration across native app screens, using tinted readable content layers while leaving the isolated provider decoder and video surfaces separate.",
            "Preserved native sleep deadlines across growing-download decoder replacement, rejected stale timer callbacks with independent revisions, and prevented automatic restart after a sleep deadline expires.",
            "Added generic decoder-error reporting and up to three growing-download recovery attempts after new downloaded bytes, preserving actor-held playback intent and last known position with revision and lifecycle guards.",
            "Restarted native player snapshots on foreground return without autoplay, keeping transport controls and decoder state current after lifecycle pauses.",
            "Added native Android media-session transport controls, numeric position and duration updates, privacy-redacted metadata, and lifecycle cleanup for local playback and PiP.",
            "Added decoder buffer progress and buffering status through RustDL’s staged helper copy, and bounded forward seeks during growing downloads using reported buffer progress with synthetic boundary checks.",
            "Added a native mini-player beside the real library view, with privacy-gated frames, Play/Pause, Next, PiP, Expand and Close controls while retaining library search and actions.",
            "Rejected queued native Play actions after the activity becomes inactive, rechecked activity state after audio-focus acquisition, and released the decoder even if final position persistence fails.",
            "Added bounded native home space drift that respects the space-effect and reduced-motion preferences, with static decoration when motion is reduced and no effect over rainy-city artwork.",
            "Rendered existing public space and light/dark rainy-city artwork behind native home controls according to the saved background preference, without loading user-media artwork.",
            "Refreshed native PiP actions and decoder surfaces only while PiP is active, updated privacy-dependent actions immediately, and retained completed-download transitions while the main activity is paused in PiP.",
            "Restricted PiP playback actions to an explicitly addressed non-exported receiver, kept play and pause pending intents distinct, and blocked Play during inspection privacy while retaining Pause.",
            "Added private native PiP play/pause actions that follow decoder state, protected screenshot flags during PiP entry, and restored video gesture attachment after PiP exits or fails.",
            "Added explicit native picture-in-picture eligibility and decoder lifecycle handling, hiding GPUI controls in PiP, retaining playback, restoring controls on exit, and protecting the PiP window from screenshots.",
            "Added native video single-tap play/pause and left/right double-tap ten-second seeks through a transparent input layer, guarded by current media generation, visibility, and inspection privacy.",
            "Prevented automatic completed-download playback from starting after the native host pauses or is destroyed during decoder preparation.",
            "Read the decoder’s latest position and play/pause state on its serialized worker immediately before reopening a completed download, preserving controls issued after a route check.",
            "Discarded completed-download reopen requests when playback controls or media changed during route checks, and made route-check failures release the pending transition safely.",
            "Reopened completed growing downloads through the validated local media route after background engine checks, preserving playback position, speed, volume, and play/pause intent with existing privacy and decoder-epoch guards.",
            "Prevented partial-download decoder completion from marking media watched, with a typed growing-download flag and synthetic completion checks.",
            "Moved isolated decoder loading progress into GPUI and removed retained Android status overlays once native controls are ready, while preserving the standard-build fallback.",
            "Rejected stale isolated streaming controls after decoder replacement, restored manual-server failover reset behavior, and presented generic GPUI loading, ready, and retry status without provider errors.",
            "Filtered isolated GPUI streaming server pages before pagination with stable decoder indexes, and closed or blocked decoder fullscreen when inspection privacy is active.",
            "Preserved streaming watchlist edits and server-filter controls in the isolated GPUI decoder interface through existing Android validation and failover behavior.",
            "Connected isolated streaming GPUI server, episode, refresh, and download controls to the existing restricted decoder, with bounded pages, readiness fallback, inspection-redacted metadata, and hidden decoder frames during privacy inspection.",
            "Added an optional GPUI controls host and typed decoder presentation for the isolated streaming process, keeping decoder addresses outside the native renderer while preparing Android control routing.",
            "Shared streaming download candidate selection between presentation adapters, preserving HLS priority, MP4 fallback order, and decoder-held addresses while testing metadata-free format labels with synthetic choices.",
            "Validated native streaming watchlist selections with synthetic trusted-link, invalid-title, and expired-session checks without accessing saved user data.",
            "Added native streaming watchlist save and removal through cached manifest selections and the existing validated watchlist store, keeping provider metadata inside the engine.",
            "Kept the selected streaming server filter when switching episodes and marked the active filter in GPUI controls.",
            "Added native streaming All, Sub, Dub, Ready, and Issues filters before server pagination, preserving opaque source indexes and inspection-private language labels.",
            "Preserved native streaming episode and server page positions during inspection-privacy rereads, and discarded streaming callbacks after leaving the native view.",
            "Added paginated native streaming server selection with stable source indexes, preserving access beyond the first 64 servers and keeping decoder URLs private.",
            "Added native streaming refresh and Back controls, retained streaming-screen navigation on return, and reloaded cached redacted pages when inspection privacy changes.",
            "Connected GPUI streaming server and episode selection to bounded Android workers, privacy-safe cached rereads, and selected-server launch through the retained restricted streaming decoder.",
            "Added typed GPUI streaming server and episode controls with private labels and bounded pagination as the presentation layer for the upcoming Android decoder routing.",
            "Added native streaming episode pages with opaque indexes, inspection-redacted titles and numbers, and validated episode switching through bounded cached manifest sessions.",
            "Added a decoder-only native streaming source bridge with bounded session selections and HTTPS source validation, keeping source addresses and host restrictions separate from GPUI presentation data.",
            "Added validated native streaming manifest commands with eight bounded opaque sessions, privacy-safe cached rereads, closed request fields, and generic provider failures.",
            "Added a bounded native streaming-manifest presentation model that hides provider URLs, host restrictions, raw errors, and inspection-private anime details before GPUI integration.",
            "Replaced the retained library-tools WebView entry with GPUI links to anime, activity, queue, storage, diagnostics, settings, and release history, preserving discovery and device-transfer access.",
            "Added a GPUI volume slider synchronized with decoder snapshots, preserving drag previews and restoring the previous nonzero level when unmuting.",
            "Presented downloaded audio with compact GPUI playback controls and prevented audio-only decoder surface attachment, while keeping media identity hidden during inspection.",
            "Added Clear Up Next to native playback and corrected player source and transfer routing to the existing validated library action adapter; updated native migration documentation.",
            "Added native player source, quality rediscovery, and encrypted device-transfer actions through opaque library handles, keeping provider URLs and filenames out of player snapshots.",
            "Added explicit native playback retry with local-route revalidation and saved-position restoration, and tested fullscreen eligibility against privacy, visibility, lifecycle, and screen state.",
            "Added GPUI playback fullscreen expansion with Android immersive controls, Back-to-exit behavior, lifecycle restoration, and screenshot protection while expanded.",
            "Added sharing from native playback through Android’s existing download-sharing flow, explicit audio-focus denial status, and exception-safe decoder and focus cleanup.",
            "Connected native playback to Android audio focus with pause on interruption and focus release on pause, completion, or disposal; native playback now honors the keep-screen-awake preference without enabling automatic picture-in-picture.",
            "Added a draggable GPUI playback timeline that follows decoder progress, preserves drag previews during snapshots, and seeks only on release; fixed the exact-seek dialog Java qualification.",
            "Added exact-position seeking from GPUI with validated numeric input and decoder duration bounds, alongside the existing ten-second seek controls.",
            "Added GPUI playback rotation locking and reset it on leaving or suspending native playback; sleep timers now clear when the native video surface becomes inactive.",
            "Added native playback sleep timers for 15, 30, or 60 minutes with remaining-time status and cancellation on media replacement or player destruction.",
            "Shared gallery playback ordering with native Next controls, preserving queued-item priority and end-of-list behavior without exposing fallback filenames to GPUI.",
            "Marked completed native playback as watched through the shared playback store, while rejecting paused, still-playing, and invalid-duration completion states.",
            "Added GPUI playback of the next saved Up Next item using the shared validated queue, without exposing queued filenames in player snapshots.",
            "Synchronized native playback speed and mute controls with decoder snapshots, and converted decoder failures into generic errors without leaving queued actions unhandled.",
            "Discarded outdated native playback commands and callbacks when replacing media, and hid the previous surface immediately during replacement.",
            "Connected native library and queue playback to GPUI controls for play, pause, seeking, speed, and mute, with privacy-gated video surfaces and retained advanced playback tools during migration.",
            "Added a native playback decoder adapter with validated local media routes, background preparation, saved progress and speed, and privacy-gated video surfaces while retaining the existing player during control migration.",
            "Added GPUI update status, explicit update checks, and installation controls using existing checksum and package validation, with duplicate checks prevented and legacy banners hidden over native screens.",
            "Moved Activity Center filters, progress, system indicators, update status, and play, queue, or transfer actions into GPUI with refresh paused offscreen and in the background.",
            "Added typed native activity pages with status filters, system counters, opaque play and transfer selections, and inspection-redacted filenames and provider errors.",
            "Moved storage usage, watched and duplicate markers, paginated file rows, and confirmed cleanup actions into GPUI, keeping cleanup work off the Android UI thread.",
            "Added typed paginated native storage snapshots and cleanup commands with opaque video selections, generic failure messages, and inspection-redacted filenames and paths.",
            "Shared storage cleanup between interface adapters while preserving watched-video deletion, stale partial cleanup, thumbnail cache clearing, and Android deletion hooks.",
            "Moved the anime release calendar into GPUI using shared weekday and new-episode logic, cached pagination, unavailable-schedule notices, and privacy-redacted release details.",
            "Passed initial native episode selection separately from the manifest endpoint so later episode switches do not reuse an old query parameter.",
            "Added native paginated anime episode selection with opaque playback handles, inspection-redacted episode details, and selected-episode launch through the existing streaming service.",
            "Moved anime catalog browsing, search, verified category selection, paginated watchlists, and save/remove controls into GPUI, retaining episode playback and release calendar during their migration.",
            "Added typed native anime catalog and paginated watchlist commands with bounded opaque selections, validated watchlist edits, generic errors, and inspection-redacted metadata.",
            "Routed pairing deep links into native device transfers and refreshed visible receiver state so expired QR pairing is removed without background polling.",
            "Added GPUI receiver QR rendering, expiring pairing renewal, a library receiving entry, and explicit clipboard copying without exposing copied secrets to native snapshots.",
            "Added native receiver pairing data with expiring QR matrices, explicit pairing-copy actions, and QR, address, and key redaction during inspection privacy.",
            "Moved library Send to Device into GPUI with manual Android pairing, encrypted transfer actions, privacy-redacted progress, and refresh limited to the visible active screen.",
            "Added typed native peer pairing, transfer startup, and progress commands with opaque library handles, generic errors, hidden pairing keys, and duplicate-transfer checks.",
            "Separated encrypted peer-transfer startup from web response rendering so native controls can reuse filename validation, completion checks, pairing validation, and transfer progress.",
            "Added native library source actions using validated provider links and direct GPUI quality rediscovery, keeping source URLs out of library snapshots.",
            "Added native playlist preparation retries after workers stop, automatic format continuation after successful preparation, and cancellation progress through worker shutdown.",
            "Added native selection across all discovery pages, clear-selection controls, and bulk quality or audio choices applied to selected candidates.",
            "Added native playlist selection and bounded preparation with progress, cancellation, format continuation, and refresh paused while the app or screen is inactive.",
            "Connected native discovery to GPUI candidate pages, persistent batch selection, format cycling, link input, and bounded background import actions.",
            "Added typed native discovery pages and import commands with bounded requests, format handles, metadata-redacted candidate rows, and generic provider errors.",
            "Shared discovery import selection and outcomes between interface adapters while preserving quality choices, duplicate prevention, playlist grouping, and escaped web errors.",
            "Restored native library, queue, and diagnostics visibility after returning from retained screens, refreshing saved playback and transfer state.",
            "Added native Up Next library filtering and add/remove controls, sharing a validated 200-item playback queue with the existing player and merging legacy browser state once.",
            "Added a native Continue Watching library filter that selects saved unfinished playback records before pagination and orders them by latest activity.",
            "Displayed saved playback progress and watched state on native library cards using opaque media handles, with immediate refresh after marking watched.",
            "Rejected stale native library responses when inspection privacy changes while a background request is running.",
            "Kept native screenshots secure until GPUI acknowledges a privacy-redacted frame, with stale acknowledgements and fullscreen or picture-in-picture captures rejected.",
            "Added GPUI library cards and playlist navigation, native search, privacy-gated thumbnails, sharing, watched markers, and confirmed deletion while retaining advanced library tools during migration.",
            "Added paginated native library data with opaque playlist/media handles, search, privacy-redacted rows, and validated media actions.",
            "Extracted a shared typed library model to retain playlist grouping and ordering across native and web renderers.",
            "Moved diagnostics into GPUI with device metric availability, preference-paced refresh while visible, and clipboard export through Android.",
            "Added a paginated GPUI download queue with direct Rust engine actions, opaque item handles, redacted snapshots, live progress, mobile-data approval, and notification routing.",
            "Added GPUI settings controls that share Android preference validation, preserve download policies, and use system dialogs for folder edits and reset confirmation.",
            "Moved release history into GPUI with bounded rendering of older notes, shared release data, and native Android Back navigation.",
            "Added an optional GPUI Kit Android home with native navigation, aggregate download progress, shared appearance, retained WebView screens, and bounded Termux build concurrency.",
            "Added inspection privacy to conceal thumbnails, titles, filenames, and video frames before agent screenshots while keeping app controls and progress visible.",
            "Made release history load older entries on demand, with direct version jumps, retryable loading, and readable notes during inspection privacy.",
            "Added a Git push guard and CI check requiring new changelog notes for changed content.",
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
        "0.1.45" => &[
            ("/settings", "Inspection privacy"),
            ("/changelog", "Release history"),
        ],
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

#[cfg(test)]
mod lazy_tests {
    use super::*;

    #[test]
    fn changelog_batches_are_bounded_and_can_render_the_oldest_version_directly() {
        let batch = render_releases(INITIAL_RELEASES, RELEASE_BATCH);
        assert_eq!(batch.matches("<article ").count(), RELEASE_BATCH);
        assert!(!batch.contains("data-release-index=\"0\""));
        let oldest = render_releases(CHANGELOG.len() - 1, 1);
        assert_eq!(oldest.matches("<article ").count(), 1);
        assert!(oldest.contains("id=\"version-0-1-0\""));
        assert!(render_releases(CHANGELOG.len(), RELEASE_BATCH).is_empty());
    }

    #[test]
    fn changelog_api_paginates_and_validates_requested_versions() {
        use std::time::Duration;
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let address = format!("http://{}", server.server_addr());
        let worker = std::thread::spawn(move || {
            for _ in 0..5 {
                let request = server
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap()
                    .unwrap();
                respond_batch(request).unwrap();
            }
        });
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let load = |query: &str| {
            client
                .get(format!("{address}/__app/changelog.json?{query}"))
                .send()
                .unwrap()
        };
        let batch: serde_json::Value = load("offset=5").json().unwrap();
        assert_eq!(batch["next"], 13);
        assert_eq!(batch["total"], CHANGELOG.len());
        assert_eq!(
            batch["html"].as_str().unwrap().matches("<article ").count(),
            8
        );
        let direct: serde_json::Value = load("version=version-0-1-0").json().unwrap();
        assert_eq!(
            direct["html"]
                .as_str()
                .unwrap()
                .matches("<article ")
                .count(),
            1
        );
        assert!(
            direct["html"]
                .as_str()
                .unwrap()
                .contains("id=\"version-0-1-0\"")
        );
        let end: serde_json::Value = load(&format!("offset={}", CHANGELOG.len())).json().unwrap();
        assert_eq!(end["html"], "");
        assert_eq!(load("offset=999999").status().as_u16(), 400);
        assert_eq!(load("version=unknown").status().as_u16(), 404);
        worker.join().unwrap();
    }
}

const INITIAL_RELEASES: usize = 5;
const RELEASE_BATCH: usize = 8;

fn release_id(version: &str) -> String {
    format!("version-{}", version.replace('.', "-"))
}

fn render_releases(start: usize, count: usize) -> String {
    let current = env!("CARGO_PKG_VERSION");
    CHANGELOG
        .iter()
        .enumerate()
        .skip(start)
        .take(count)
        .map(|(index, (version, changes))| {
            let id = release_id(version);
            let badge = if *version == current {
                r#"<span class="current">Current</span>"#
            } else {
                ""
            };
            let items = changes
                .iter()
                .map(|change| format!("<li>{}</li>", local::html::escape_html(change)))
                .collect::<String>();
            let destinations = destinations(version)
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
                r##"<article id="{id}" data-release-index="{index}"><header><h2><a class="version-link" href="#{id}">Version {version}</a></h2>{badge}</header><ul>{items}</ul>{actions}</article>"##
            )
        })
        .collect()
}

pub(in super::super::super) fn respond_batch(request: Request) -> Result<(), Box<dyn Error>> {
    let url = reqwest::Url::parse(&format!("http://localhost{}", request.url()))?;
    let version = url
        .query_pairs()
        .find(|(key, _)| key == "version")
        .map(|(_, value)| value.into_owned());
    let start = if let Some(version) = version.as_deref() {
        match CHANGELOG
            .iter()
            .position(|(value, _)| release_id(value) == version)
        {
            Some(index) => index,
            None => return local::html::respond_text(request, 404, "Unknown changelog version"),
        }
    } else {
        match url
            .query_pairs()
            .find(|(key, _)| key == "offset")
            .and_then(|(_, value)| value.parse::<usize>().ok())
        {
            Some(offset) if offset <= CHANGELOG.len() => offset,
            _ => return local::html::respond_text(request, 400, "Invalid changelog offset"),
        }
    };
    let count = if version.is_some() { 1 } else { RELEASE_BATCH };
    let next = start.saturating_add(count).min(CHANGELOG.len());
    let response = tiny_http::Response::from_string(
        serde_json::json!({
            "html": render_releases(start, count), "next": next, "total": CHANGELOG.len()
        })
        .to_string(),
    )
    .with_header(local::html::header(
        "Content-Type",
        "application/json; charset=utf-8",
    ))
    .with_header(local::html::header("Cache-Control", "no-store"));
    request.respond(response)?;
    Ok(())
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
    let releases = render_releases(0, INITIAL_RELEASES);
    {
        let dev_reload = &(local::dev::dev_reload_script());
        format!(
            include_str!("../../../assets/html/changelog.html"),
            dev_reload = dev_reload,
            page_css = include_str!("../../../assets/css/changelog.css"),
            page_script = include_str!("../../../assets/js/changelog.js"),
            options = options,
            releases = releases,
            next = INITIAL_RELEASES.min(CHANGELOG.len()),
            total = CHANGELOG.len()
        )
    }
}

pub(in super::super::super) fn respond(request: Request) -> Result<(), Box<dyn Error>> {
    local::html::respond_html(request, local::pages::changelog::render())
}
