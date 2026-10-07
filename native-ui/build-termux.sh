#!/data/data/com.termux/files/usr/bin/sh
set -eu
ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export CARGO_TARGET_DIR="$ROOT_DIR/target/native-ui"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-1}
export CARGO_NET_GIT_FETCH_WITH_CLI=true
export PATH="$ROOT_DIR/native-ui/tools:$PATH"
export CC=clang CXX=clang++ AR=llvm-ar
export CFLAGS=--target=aarch64-linux-android29
export CXXFLAGS=--target=aarch64-linux-android29
python3 "$ROOT_DIR/native-ui/stage-mobile-accessibility.py"
export RUSTDL_NATIVE_RUSTC_STAGE_MAP="$ROOT_DIR/target/native-ui-platform/compile-map.json"
RUSTDL_NATIVE_PREVIOUS_WRAPPER=${RUSTC_WRAPPER:-}
if [ "$RUSTDL_NATIVE_PREVIOUS_WRAPPER" = "$ROOT_DIR/native-ui/tools/rustc-stage" ]; then
    RUSTDL_NATIVE_PREVIOUS_WRAPPER=""
fi
export RUSTDL_NATIVE_PARENT_RUSTC_WRAPPER="$RUSTDL_NATIVE_PREVIOUS_WRAPPER"
export RUSTC_WRAPPER="$ROOT_DIR/native-ui/tools/rustc-stage"
# Invalidate only the adapter crates so Cargo uses their owned source copies
# consistently in the normal dependency graph (no separate rustc feature graph).
RUSTDL_ADAPTER_STAMP="$CARGO_TARGET_DIR/owned-accessibility-graph-v2"
if [ ! -f "$RUSTDL_ADAPTER_STAMP" ]; then
    cargo clean --manifest-path "$ROOT_DIR/native-ui/Cargo.toml" --target aarch64-linux-android --package gpui-pre --package gpui-pre-mobile
fi
cargo build --release --locked --manifest-path "$ROOT_DIR/native-ui/Cargo.toml" --target aarch64-linux-android
touch "$RUSTDL_ADAPTER_STAMP"
printf '%s\n' "$CARGO_TARGET_DIR/aarch64-linux-android/release/librustdl_ui.so"
