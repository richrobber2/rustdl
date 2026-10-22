use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source = root.join("assets/js");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("js");
    println!("cargo:rerun-if-changed=assets/js");
    println!("cargo:rerun-if-changed=scripts/minify-js.cjs");
    println!("cargo:rerun-if-changed=tests/ui/package-lock.json");
    println!("cargo:rerun-if-env-changed=OXC_MINIFY_BIN");
    // Never let a deleted source survive as a stale embedded output.
    if output.exists() {
        fs::remove_dir_all(&output).unwrap();
    }
    if env::var_os("CARGO_FEATURE_MINIFIED_JS").is_some() {
        if let Some(binary) = env::var_os("OXC_MINIFY_BIN") {
            println!("cargo:rerun-if-changed={}", PathBuf::from(binary).display());
        }
        let result = Command::new("node")
            .current_dir(&root)
            .arg(root.join("scripts/minify-js.cjs"))
            .arg(&output)
            .output()
            .expect("minified-js requires Node and npm --prefix tests/ui ci");
        assert!(
            result.status.success(),
            "JavaScript minification failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        println!(
            "cargo:warning={}",
            String::from_utf8_lossy(&result.stdout).trim()
        );
    } else {
        fs::create_dir_all(&output).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "js")
            {
                fs::copy(entry.path(), output.join(entry.file_name())).unwrap();
            }
        }
    }
}
