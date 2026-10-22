#!/bin/sh
# Synthetic offline timing with the normal allocator; never installs an APK.
set -eu
umask 077
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
workload=${1:-gallery-5000}
samples=${2:-31}
case "$workload" in page-activity|page-settings|page-changelog|gallery-500|gallery-5000|queue-page-500|queue-page-5000|quality-500) ;; *) echo 'Unknown page workload; see PERFORMANCE.md' >&2; exit 2;; esac
case "$samples" in ''|*[!0-9]*|0) echo 'Samples must be 1..1000' >&2; exit 2;; esac
[ "$samples" -le 1000 ] || { echo 'Samples must be 1..1000' >&2; exit 2; }
if [ "$(uname -o 2>/dev/null || true)" = Android ]; then
    export BINDGEN_EXTRA_CLANG_ARGS="${BINDGEN_EXTRA_CLANG_ARGS:---target=aarch64-linux-android24}"
fi
cargo build --locked --profile profiling --bin rustdl-page-bench --features heap-profile
exec target/profiling/rustdl-page-bench "$workload" "$samples"
