//! Local runtime tuning, inspection state, and Android callbacks.

use super::super::local;
use std::path::Path;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;

pub(in super::super) type PublishHook = fn(&Path, &str) -> Result<bool, String>;

pub(in super::super) type TransferHook = fn(TransferSummary) -> Result<(), String>;

pub(in super::super) type EventHook = fn(&str) -> Result<(), String>;

pub(in super::super) type WatchedHook = fn() -> Result<Vec<String>, String>;

pub(in super::super) type DeleteHook = fn(&str) -> Result<(), String>;

pub(in super::super) type MuxHook = fn(&Path, &Path, &Path) -> Result<(), String>;

pub(in super::super) type ExtractAudioHook = fn(&Path, &Path) -> Result<(), String>;

pub(in super::super) type ThumbnailHook = fn(&Path, &str) -> Result<bool, String>;

pub(in super::super) static PUBLISH_HOOK: OnceLock<PublishHook> = OnceLock::new();

pub(in super::super) static TRANSFER_HOOK: OnceLock<TransferHook> = OnceLock::new();

pub(in super::super) static EVENT_HOOK: OnceLock<EventHook> = OnceLock::new();

pub(in super::super) static WATCHED_HOOK: OnceLock<WatchedHook> = OnceLock::new();

pub(in super::super) static DELETE_HOOK: OnceLock<DeleteHook> = OnceLock::new();

pub(in super::super) static MUX_HOOK: OnceLock<MuxHook> = OnceLock::new();

pub(in super::super) static EXTRACT_AUDIO_HOOK: OnceLock<ExtractAudioHook> = OnceLock::new();

pub(in super::super) static THUMBNAIL_HOOK: OnceLock<ThumbnailHook> = OnceLock::new();

pub(in super::super) static INSPECTION_MODE: OnceLock<bool> = OnceLock::new();

pub(in super::super) static EVENT_REVISION: AtomicU64 = AtomicU64::new(0);

pub(in super::super) static RUNTIME_TUNING: OnceLock<Mutex<RuntimeTuning>> = OnceLock::new();

// CLI downloads are unrestricted. Android supplies a decision before queue startup.
static DOWNLOAD_NETWORK_STATE: AtomicU8 = AtomicU8::new(0);

pub(crate) fn set_download_network_state(state: u8) {
    DOWNLOAD_NETWORK_STATE.store(state, Ordering::Release);
    local::queue::download_gate().1.notify_all();
    local::queue::notify_transfer_state(true);
}

pub(in super::super) fn download_network_reason() -> Option<&'static str> {
    match DOWNLOAD_NETWORK_STATE.load(Ordering::Acquire) {
        0 => None,
        2 => Some("Waiting for unmetered Wi-Fi"),
        3 => Some("Waiting for mobile-data approval"),
        _ => Some("Waiting for an internet connection"),
    }
}

#[derive(Clone, Copy, Debug)]
pub(in super::super) struct RuntimeTuning {
    pub(in super::super) unmetered: bool,
    pub(in super::super) charging: bool,
    pub(in super::super) power_save: bool,
    pub(in super::super) thermal_status: i32,
    pub(in super::super) free_bytes: u64,
    pub(in super::super) processors: usize,
}

impl Default for RuntimeTuning {
    fn default() -> Self {
        Self {
            unmetered: false,
            charging: false,
            power_save: false,
            thermal_status: 0,
            free_bytes: u64::MAX,
            processors: thread::available_parallelism().map_or(2, usize::from),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TransferSummary {
    pub(crate) count: u32,
    pub(crate) downloaded: u64,
    pub(crate) total: u64,
}

pub(in super::super) fn runtime_tuning() -> RuntimeTuning {
    *RUNTIME_TUNING
        .get_or_init(|| Mutex::new(RuntimeTuning::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[allow(dead_code)]
pub(crate) fn set_publish_hook(hook: PublishHook) {
    let _ = PUBLISH_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_transfer_hook(hook: TransferHook) {
    let _ = TRANSFER_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_event_hook(hook: EventHook) {
    let _ = EVENT_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_watched_hook(hook: WatchedHook) {
    let _ = WATCHED_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_delete_hook(hook: DeleteHook) {
    let _ = DELETE_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_mux_hook(hook: MuxHook) {
    let _ = MUX_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_extract_audio_hook(hook: ExtractAudioHook) {
    let _ = EXTRACT_AUDIO_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_thumbnail_hook(hook: ThumbnailHook) {
    let _ = THUMBNAIL_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_runtime_tuning(
    unmetered: bool,
    charging: bool,
    power_save: bool,
    thermal_status: i32,
    free_bytes: u64,
    processors: usize,
) {
    *RUNTIME_TUNING
        .get_or_init(|| Mutex::new(RuntimeTuning::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = RuntimeTuning {
        unmetered,
        charging,
        power_save,
        thermal_status: thermal_status.max(0),
        free_bytes,
        processors: processors.max(1),
    };
    local::queue::download_gate().1.notify_all();
}

#[allow(dead_code)]
pub(crate) fn set_inspection_mode(enabled: bool) {
    let _ = INSPECTION_MODE.set(enabled);
}

pub(in super::super) fn inspection_mode() -> bool {
    INSPECTION_MODE.get().copied().unwrap_or(false)
}

pub(in super::super) fn notify_simple_event(kind: &str) {
    if let Some(hook) = EVENT_HOOK.get() {
        let event = serde_json::json!({
            "type": kind,
            "version": 1,
            "revision": EVENT_REVISION.fetch_add(1, Ordering::Relaxed) + 1,
        })
        .to_string();
        if let Err(error) = hook(&event) {
            eprintln!("Android WebView event warning: {error}");
        }
    }
}

pub(in super::super) fn adaptive_download_limit() -> usize {
    const GIB: u64 = 1024 * 1024 * 1024;
    let tuning = local::runtime::runtime_tuning();
    if tuning.free_bytes < GIB || tuning.power_save || tuning.thermal_status >= 3 {
        1
    } else if tuning.unmetered
        && tuning.thermal_status <= 1
        && tuning.free_bytes >= 3 * GIB
        && tuning.processors >= 6
        && (tuning.charging || tuning.processors >= 8)
    {
        3
    } else {
        2
    }
}

pub(in super::super) fn adaptive_download_buffer_bytes() -> usize {
    const GIB: u64 = 1024 * 1024 * 1024;
    let tuning = local::runtime::runtime_tuning();
    if tuning.free_bytes < GIB || tuning.power_save || tuning.thermal_status >= 3 {
        64 * 1024
    } else if tuning.unmetered && tuning.thermal_status <= 1 {
        512 * 1024
    } else {
        256 * 1024
    }
}
