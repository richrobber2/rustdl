use std::{env, fs, path::PathBuf};

fn main() {
    let source = "../src/local/pages/changelog.rs";
    println!("cargo:rerun-if-changed={source}");
    let text = fs::read_to_string(source).expect("read public release notes");
    let start = text
        .find("const CHANGELOG:")
        .expect("release note declaration");
    let tail = &text[start..];
    let end = tail.find("\n];").expect("release note terminator") + 3;
    let declaration = format!("pub {}", &tail[..end]);
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("build output"));
    fs::write(output.join("release_notes.rs"), declaration).expect("stage release notes");
}
