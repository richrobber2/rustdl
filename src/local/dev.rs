//! Local source watching, build processes, and development reloads.

use super::super::local;
use super::super::local::cli::ServeArgs;
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, fs, io, thread};
use tiny_http::{Request, Response, StatusCode};

pub(in super::super) const DEV_TOKEN_ENV: &str = "RUSTDL_DEV_TOKEN";

pub(in super::super) fn dev(args: ServeArgs) -> Result<(), Box<dyn Error>> {
    let executable = env::current_exe()?;
    let mut snapshot = local::dev::source_snapshot()?;
    let mut generation = 0_u64;
    let mut child = local::dev::spawn_dev_server(&executable, &args, generation)?;
    eprintln!("Watching src/, assets/, and Cargo.toml for changes...");

    loop {
        thread::sleep(Duration::from_millis(500));
        if let Some(status) = child.try_wait()? {
            return Err(format!("development server exited with {status}").into());
        }

        let next_snapshot = local::dev::source_snapshot()?;
        if next_snapshot == snapshot {
            continue;
        }
        snapshot = next_snapshot;
        eprintln!("Change detected; rebuilding...");
        let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let status = Command::new(cargo).arg("build").status()?;
        if !status.success() {
            eprintln!("Build failed; the previous server is still running.");
            continue;
        }

        local::dev::stop_child(&mut child)?;
        generation += 1;
        child = local::dev::spawn_dev_server(&executable, &args, generation)?;
        eprintln!("Build succeeded; server restarted and browsers will reload.");
    }
}

pub(in super::super) fn spawn_dev_server(
    executable: &Path,
    args: &ServeArgs,
    generation: u64,
) -> Result<Child, Box<dyn Error>> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let token = format!("{timestamp}-{generation}");
    Ok(Command::new(executable)
        .arg("serve")
        .arg("--bind")
        .arg(&args.bind)
        .arg("--output-dir")
        .arg(&args.output_dir)
        .env(DEV_TOKEN_ENV, token)
        .spawn()?)
}

pub(in super::super) fn stop_child(child: &mut Child) -> io::Result<()> {
    child.kill()?;
    child.wait()?;
    Ok(())
}

pub(in super::super) fn source_snapshot() -> io::Result<u64> {
    let mut hasher = DefaultHasher::new();
    local::dev::hash_watch_path(Path::new("Cargo.toml"), &mut hasher)?;
    local::dev::hash_watch_path(Path::new("src"), &mut hasher)?;
    local::dev::hash_watch_path(Path::new("assets"), &mut hasher)?;
    Ok(hasher.finish())
}

pub(in super::super) fn hash_watch_path(path: &Path, hasher: &mut DefaultHasher) -> io::Result<()> {
    path.hash(hasher);
    let metadata = fs::metadata(path)?;
    metadata.len().hash(hasher);
    metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(hasher);
    if metadata.is_dir() {
        let mut children = fs::read_dir(path)?
            .map(|entry| entry.map(|value| value.path()))
            .collect::<io::Result<Vec<_>>>()?;
        children.sort_unstable();
        for child in children {
            local::dev::hash_watch_path(&child, hasher)?;
        }
    }
    Ok(())
}

pub(in super::super) fn respond_dev_version(request: Request) -> Result<(), Box<dyn Error>> {
    let Ok(token) = env::var(DEV_TOKEN_ENV) else {
        return local::html::respond_text(request, 404, "Hot reload is disabled");
    };
    let response = Response::from_string(token)
        .with_status_code(StatusCode(200))
        .with_header(local::html::header(
            "Content-Type",
            "text/plain; charset=utf-8",
        ))
        .with_header(local::html::header("Cache-Control", "no-store"));
    request.respond(response)?;
    Ok(())
}

pub(in super::super) fn dev_reload_script() -> String {
    let Ok(token) = env::var(DEV_TOKEN_ENV) else {
        return String::new();
    };
    format!(
        "<script>{}</script>",
        include_str!("../../assets/js/dev-reload.js").replace("__RUSTDL_VERSION_TOKEN__", &token)
    )
}
