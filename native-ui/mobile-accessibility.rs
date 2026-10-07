//! RustDL's Android build adapter. Semantic actions remain in GPUI.
use gpui::{A11yCallbacks, RustdlTreeUpdate};
use std::sync::{Mutex, atomic::{AtomicBool, Ordering}};
static CALLBACKS: Mutex<Option<A11yCallbacks>> = Mutex::new(None);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static RESET_REQUIRED: AtomicBool = AtomicBool::new(false);
static CONTEXT: Mutex<(u64,u8)> = Mutex::new((0,0));
static UPDATES: Mutex<Vec<(u64,u8,RustdlTreeUpdate)>> = Mutex::new(Vec::new());
pub fn install(callbacks: A11yCallbacks) {
    if ACTIVE.load(Ordering::Acquire) { let _ = (callbacks.activation)(); }
    *CALLBACKS.lock().unwrap_or_else(|p| p.into_inner()) = Some(callbacks);
}
pub fn set_context(revision:u64,screen:u8) {
    let mut context=CONTEXT.lock().unwrap_or_else(|p|p.into_inner());
    if *context!=(revision,screen) {*context=(revision,screen);gpui::rustdl_a11y_action_policy::invalidate();}
}
pub fn update(tree:RustdlTreeUpdate) {
    if !ACTIVE.load(Ordering::Acquire) {return;}
    let mut updates=UPDATES.lock().unwrap_or_else(|p|p.into_inner());
    if updates.len()>=64 {updates.clear();RESET_REQUIRED.store(true,Ordering::Release);return;}
    let (revision,screen)=*CONTEXT.lock().unwrap_or_else(|p|p.into_inner());
    updates.push((revision,screen,tree));
}
pub fn take_updates()->Vec<(u64,u8,RustdlTreeUpdate)> {std::mem::take(&mut *UPDATES.lock().unwrap_or_else(|p|p.into_inner()))}
pub fn set_active(active: bool) {
    if ACTIVE.swap(active, Ordering::AcqRel) == active { return; }
    gpui::rustdl_a11y_action_policy::set_active(active);
    if !active {UPDATES.lock().unwrap_or_else(|p|p.into_inner()).clear();}
    let callbacks = CALLBACKS.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(callbacks) = callbacks.as_ref() {
        if active { let _ = (callbacks.activation)(); } else { (callbacks.deactivation)(); }
    }
}
pub fn dispatch(invoke: impl FnOnce(&A11yCallbacks)) -> bool {
    if !ACTIVE.load(Ordering::Acquire) { return false; }
    let callbacks = CALLBACKS.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(callbacks) = callbacks.as_ref() { invoke(callbacks); true } else { false }
}

pub fn take_reset_required() -> bool { RESET_REQUIRED.swap(false,Ordering::AcqRel) }
