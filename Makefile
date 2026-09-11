.DEFAULT_GOAL := help

.PHONY: help setup check apk test run dev dev-test dev-apk dev-visual dev-real-gallery

help:
	@echo "RustDL"
	@echo "  make setup  Install Termux build tools and fetch the pinned API-35 android.jar"
	@echo "  make check  Verify Android build prerequisites without changing anything"
	@echo "  make apk    Build and sign target/android-termux/rustdl.apk"
	@echo "  make test   Run the optimized Rust test suite"
	@echo "  make dev-test  Run optional synthetic gallery performance tests"
	@echo "  make dev-apk   Build the optional Android visual test APK"
	@echo "  make dev-visual  Run installed visual suite and print coverage"
	@echo "  make run    Start the local web app"
	@echo "  make dev    Start the web app with hot reload"

setup:
	sh android/setup-termux.sh

check:
	sh android/setup-termux.sh --check

apk: check
	sh android/build-termux.sh

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
dev-apk: check
	RUSTDL_VARIANT=alongside RUSTDL_DEV=1 sh android/build-termux.sh

# Requires the development APK to be installed; results stream to the terminal.
dev-visual:
	python3 src/dev/run-visual.py

# Actual gallery, numeric counters only. Requires the installed development APK.
dev-real-gallery:
	python3 src/dev/run-real-gallery.py
