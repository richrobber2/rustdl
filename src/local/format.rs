//! Local text, byte-count, token encoding, and progress formatting.

use super::super::local;
use std::io;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

pub(in super::super) fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(in super::super) fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(in super::super) fn truncate_text(text: &str, max_chars: usize) -> String {
    let mut value = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        value.push('…');
    }
    value
}

pub(in super::super) fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub(in super::super) fn show_progress(
    downloaded: u64,
    total: Option<u64>,
    last_progress: &mut Option<u64>,
) -> io::Result<()> {
    let progress = match total {
        Some(total) if total > 0 => {
            let percent = downloaded.saturating_mul(100) / total;
            if *last_progress != Some(percent) {
                eprint!(
                    "\r{percent:3}%  {} / {}",
                    local::format::human_bytes(downloaded),
                    local::format::human_bytes(total)
                );
            }
            percent
        }
        _ => {
            let mebibytes = downloaded / (1024 * 1024);
            if *last_progress != Some(mebibytes) {
                eprint!("\r{} downloaded", local::format::human_bytes(downloaded));
            }
            mebibytes
        }
    };
    if *last_progress != Some(progress) {
        *last_progress = Some(progress);
        io::stderr().flush()?;
    }
    Ok(())
}

pub(in super::super) fn human_bytes(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / MIB)
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}
