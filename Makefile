.DEFAULT_GOAL := help

.PHONY: help setup check apk test run dev

help:
	@echo "RustDL"
	@echo "  make setup  Install Termux build tools and fetch the pinned API-35 android.jar"
	@echo "  make check  Verify Android build prerequisites without changing anything"
	@echo "  make apk    Build and sign target/android-termux/rustdl.apk"
	@echo "  make test   Run the optimized Rust test suite"
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
