//! Locally generated action tokens.

use super::super::local;
use std::fs::File;
use std::io::Read;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub(in super::super) static ACTION_TOKEN: OnceLock<String> = OnceLock::new();

pub(in super::super) fn random_token() -> String {
    let mut bytes = [0_u8; 16];
    let filled = File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .is_ok();
    if !filled {
        let fallback = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        bytes.copy_from_slice(&fallback.to_le_bytes());
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(in super::super) fn action_token() -> &'static str {
    ACTION_TOKEN.get_or_init(local::security::random_token)
}
