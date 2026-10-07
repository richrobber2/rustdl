.DEFAULT_GOAL := help

.PHONY: help setup native-setup check apk native-apk legacy-apk test run dev dev-test dev-apk dev-visual dev-real-gallery hooks

help:
	@echo "RustDL"
	@echo "  make setup  Install Termux build tools and fetch the pinned API-35 android.jar"
	@echo "  make check  Verify Android build prerequisites without changing anything"
	@echo "  make apk    Build and sign the GPUI APK at target/android-termux-gpui/rustdl.apk"
	@echo "  make native-apk  Build RustDL Next with the GPUI interface"
	@echo "  make legacy-apk  Build the compatibility WebView APK"
	@echo "  make native-setup  Install native UI build tools"
	@echo "  make test   Run the optimized Rust test suite"
	@echo "  make dev-test  Run optional synthetic gallery performance tests"
	@echo "  make dev-apk   Build the optional Android visual test APK"
	@echo "  make dev-visual  Run installed visual suite and print coverage"
	@echo "  make run    Start the local web app"
	@echo "  make dev    Start the web app with hot reload"
	@echo "  make hooks  Enable the changelog pre-push guard in this checkout"

hooks:
	git config --local core.hooksPath .githooks

setup:
	sh android/setup-termux.sh

native-setup:
	RUSTDL_NATIVE_UI=1 sh android/setup-termux.sh

check:
	sh android/setup-termux.sh --check

apk: check
	sh android/build-termux.sh

legacy-apk:
	RUSTDL_NATIVE_UI=0 sh android/setup-termux.sh --check
	RUSTDL_NATIVE_UI=0 sh android/build-termux.sh

native-apk: check
	RUSTDL_VARIANT=alongside RUSTDL_NATIVE_UI=1 sh android/build-termux.sh

test:
	cargo test --release

run:
	cargo run -- serve

dev:
	cargo run -- dev

# Opt-in only: never part of apk or the normal test suite.
dev-test:
	cargo build --release --bin rustdl
	cargo test --release --features dev --bin rustdl dev:: -- --ignored --nocapture --test-threads=1

# Android visual benchmark is compiled and packaged only with this opt-in flag.
dev-apk:
	RUSTDL_NATIVE_UI=0 sh android/setup-termux.sh --check
	RUSTDL_NATIVE_UI=0 RUSTDL_VARIANT=alongside RUSTDL_DEV=1 sh android/build-termux.sh

# Requires the development APK to be installed; results stream to the terminal.
dev-visual:
	python3 src/dev/run-visual.py

# Actual gallery, numeric counters only. Requires the installed development APK.
dev-real-gallery:
	python3 src/dev/run-real-gallery.py
