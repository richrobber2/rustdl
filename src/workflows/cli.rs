//! CLI orchestration across local arguments and external media requests.

use super::super::local::cli::Args;
use super::super::{external, local, workflows};
use std::error::Error;
use std::{env, fs};

pub(in super::super) fn run() -> Result<(), Box<dyn Error>> {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    if raw_args.first().is_some_and(|arg| arg == "serve") {
        return workflows::server::serve(local::cli::parse_serve_args(&raw_args[1..])?);
    }
    if raw_args.first().is_some_and(|arg| arg == "dev") {
        return local::dev::dev(local::cli::parse_serve_args(&raw_args[1..])?);
    }
    workflows::cli::download_command(local::cli::parse_download_args(raw_args.into_iter())?)
}

pub(in super::super) fn download_command(args: Args) -> Result<(), Box<dyn Error>> {
    let client = external::http::build_client()?;
    let resolved = workflows::discovery::resolve_video(&client, &args.url)?;
    let output = args
        .output
        .unwrap_or_else(|| local::files::default_download_dir().join(resolved.filename()));

    if local::files::is_complete_download(&output)? && !args.force {
        eprintln!("Duplicate detected; already saved at {}", output.display());
        return Ok(());
    }
    if output.exists() && !args.force {
        fs::remove_file(&output)?;
    }
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    eprintln!("Downloading best MP4 to {}...", output.display());
    external::http::download(&client, &resolved.media_url, &output, args.force)?;
    eprintln!("Saved {}", output.display());
    Ok(())
}
