//! Opt-in synthetic performance tests. Not compiled into application builds.
//!
//! Run `make dev-test`; see README.md for dependencies and measurement limits.

use std::path::PathBuf;
use std::process::Command;

fn run_script(runtime: &str, filename: &str, node: bool) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(root.join("target/gallery-performance"))
        .expect("create benchmark output directory");
    let mut command = Command::new(runtime);
    command.current_dir(&root);
    if node {
        command.arg("--expose-gc");
        if std::env::var_os("NODE_PATH").is_none() {
            let dependencies = [
                root.join("tests/ui/node_modules"),
                root.join("target/ui-tests/node_modules"),
            ]
            .into_iter()
            .find(|path| path.join("jsdom").is_dir())
            .expect("Install UI test dependencies with: npm --prefix tests/ui install");
            command.env("NODE_PATH", dependencies);
        }
    }
    let status = command
        .arg(root.join("src/dev").join(filename))
        .status()
        .unwrap_or_else(|error| panic!("Could not run {runtime} for {filename}: {error}"));
    assert!(status.success(), "{filename} failed: {status}");
}

#[test]
#[ignore = "opt-in performance measurement; use make dev-test"]
fn gallery_load() {
    assert!(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/release/rustdl")
            .is_file(),
        "Build the benchmark server first: cargo build --release --bin rustdl"
    );
    run_script("python3", "gallery-http.py", false);
}

#[test]
#[ignore = "opt-in synthetic DOM measurement; not WebView paint time"]
fn gallery_dom() {
    run_script("node", "gallery-dom.cjs", true);
}

#[test]
#[ignore = "opt-in synthetic scroll-loading measurement; not screen FPS"]
fn gallery_scroll() {
    run_script("node", "gallery-scroll.cjs", true);
}
