//! Local media paths, file completeness, and content hashing.

use super::super::local::runtime::PUBLISH_HOOK;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

pub(in super::super) const DOWNLOAD_FOLDER_NAME: &str = "RustDL";

pub(in super::super) fn blake3_file(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub(in super::super) fn blake3_hasher_for_existing(
    path: &Path,
    bytes: u64,
) -> Result<blake3::Hasher, String> {
    let mut hasher = blake3::Hasher::new();
    if bytes == 0 {
        return Ok(hasher);
    }
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut remaining = bytes;
    let mut buffer = vec![0_u8; 256 * 1024];
    while remaining > 0 {
        let limit = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let count = file
            .read(&mut buffer[..limit])
            .map_err(|error| error.to_string())?;
        if count == 0 {
            return Err("resumable download prefix ended before its saved offset".to_owned());
        }
        hasher.update(&buffer[..count]);
        remaining -= count as u64;
    }
    Ok(hasher)
}

pub(in super::super) fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(in super::super) fn is_complete_download(path: &Path) -> io::Result<bool> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file() && metadata.len() > 0),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(in super::super) fn default_download_dir() -> PathBuf {
    let android_downloads = Path::new("/sdcard/Download");
    if android_downloads.is_dir() {
        return android_downloads.join(DOWNLOAD_FOLDER_NAME);
    }
    if let Some(home) = env::var_os("HOME") {
        let downloads = PathBuf::from(home).join("Downloads");
        if downloads.is_dir() {
            return downloads.join(DOWNLOAD_FOLDER_NAME);
        }
    }
    PathBuf::from("downloads").join(DOWNLOAD_FOLDER_NAME)
}

pub(in super::super) fn display_output_path(output: &Path) -> String {
    if PUBLISH_HOOK.get().is_some()
        && let Some(filename) = output.file_name().and_then(|name| name.to_str())
    {
        return format!("Downloads/{DOWNLOAD_FOLDER_NAME}/{filename}");
    }
    output.display().to_string()
}

pub(in super::super) fn part_path(output: &Path) -> PathBuf {
    let mut name = output.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}
