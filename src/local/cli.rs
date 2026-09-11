//! Command-line argument parsing and help text.

use super::super::local;
use std::error::Error;
use std::path::PathBuf;

pub(in super::super) const DEFAULT_BIND: &str = "127.0.0.1:8080";

pub(in super::super) struct Args {
    pub(in super::super) url: String,
    pub(in super::super) output: Option<PathBuf>,
    pub(in super::super) force: bool,
}

#[derive(Clone)]
pub(in super::super) struct ServeArgs {
    pub(in super::super) bind: String,
    pub(in super::super) output_dir: PathBuf,
}

pub(in super::super) fn parse_download_args(
    mut args: impl Iterator<Item = String>,
) -> Result<Args, Box<dyn Error>> {
    let mut url = None;
    let mut output = None;
    let mut force = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                local::cli::print_help();
                std::process::exit(0);
            }
            "-o" | "--output" => {
                output = Some(PathBuf::from(
                    args.next().ok_or("--output needs a file path")?,
                ));
            }
            "-f" | "--force" => force = true,
            _ if arg.starts_with('-') => return Err(format!("unknown option: {arg}").into()),
            _ if url.is_none() => url = Some(arg),
            _ => return Err("only one video URL may be supplied".into()),
        }
    }

    Ok(Args {
        url: url.ok_or("missing video URL; run with --help for usage")?,
        output,
        force,
    })
}

pub(in super::super) fn print_help() {
    println!(
        "rustdl - download a public social video\n\n\
         Usage:\n  \
           rustdl [OPTIONS] <VIDEO-URL>\n  \
           rustdl serve [--bind <ADDRESS>] [--output-dir <DIRECTORY>]\n\n\
           rustdl dev [--bind <ADDRESS>] [--output-dir <DIRECTORY>]\n\n\
         Options:\n  \
           -o, --output <FILE>  Override the default Downloads/RustDL output path\n  \
           -f, --force          Overwrite an existing output file\n  \
               --bind <ADDRESS> Web server address (default: 127.0.0.1:8080)\n  \
               --output-dir <DIRECTORY>  Web download folder\n  \
           -h, --help           Show this help"
    );
}

pub(in super::super) fn parse_serve_args(args: &[String]) -> Result<ServeArgs, Box<dyn Error>> {
    let mut bind = DEFAULT_BIND.to_owned();
    let mut output_dir = local::files::default_download_dir();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => {
                local::cli::print_help();
                std::process::exit(0);
            }
            "--bind" => {
                index += 1;
                bind = args.get(index).ok_or("--bind needs an address")?.to_owned();
            }
            "--output-dir" => {
                index += 1;
                output_dir =
                    PathBuf::from(args.get(index).ok_or("--output-dir needs a directory")?);
            }
            option => return Err(format!("unknown serve option: {option}").into()),
        }
        index += 1;
    }
    Ok(ServeArgs { bind, output_dir })
}
