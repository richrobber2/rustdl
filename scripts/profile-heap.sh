#!/bin/sh
# Opt-in profiling only; never packages or installs an Android app.
set -eu
umask 077
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
workload=${1:-activity}
iterations=${2:-10}
case "$workload" in playlist-preview-100|playlist-preview-1000|playlist-preview-5000|page-activity|page-settings|page-changelog|gallery-500|gallery-5000|queue-page-500|queue-page-5000|activity|changelog|catalog|queue-500|queue-5000|quality-500|playlist-500|playlist-cancel-500|playlist-page|playlist-page-baseline) ;; *) echo 'Choose playlist-preview-100, playlist-preview-1000, playlist-preview-5000, page-activity, page-settings, page-changelog, gallery-500, gallery-5000, queue-page-500, queue-page-5000, activity, changelog, catalog, queue-500, queue-5000, quality-500, playlist-500, playlist-cancel-500, playlist-page or playlist-page-baseline' >&2; exit 2;; esac
case "$iterations" in ''|*[!0-9]*|0) echo 'Iterations must be 1..100' >&2; exit 2;; esac
[ "$iterations" -le 100 ] || { echo 'Iterations must be 1..100' >&2; exit 2; }
if [ "$(uname -o 2>/dev/null || true)" = Android ]; then
    export BINDGEN_EXTRA_CLANG_ARGS="${BINDGEN_EXTRA_CLANG_ARGS:---target=aarch64-linux-android24}"
fi
cargo build --locked --profile profiling --bin rustdl-heap-profile --features heap-profile
exec target/profiling/rustdl-heap-profile "$workload" "$iterations"
