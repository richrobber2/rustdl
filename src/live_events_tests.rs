use super::{PLAYBACK_SCRIPT, live_events};

#[test]
fn rust_java_and_webview_share_a_typed_state_event_path() {
    let rust_bridge = include_str!("lib.rs");
    let rust_app = include_str!("main.rs");
    let android = include_str!("../android/MainActivity.java");

    assert!(rust_bridge.contains("set_event_hook(dispatch_android_event)"));
    assert!(rust_bridge.contains("dispatchRustEvent"));
    assert!(rust_app.contains(r#""type": "queue""#));
    assert!(rust_app.contains("EVENT_REVISION.fetch_add"));
    assert!(android.contains("eventJson.length() > 2_048"));
    assert!(android.contains("current.getPort() != 37658"));
    assert!(android.contains("new CustomEvent('rustdl:state'"));
    assert!(android.contains(r#"dispatchRustEvent("{\"type\":\"sync\",\"version\":1}")"#));
}

#[test]
fn live_ui_updates_are_event_driven_with_a_slow_recovery_poll() {
    assert!(PLAYBACK_SCRIPT.contains("addEventListener('rustdl:state'"));
    assert_eq!(PLAYBACK_SCRIPT.matches("15000").count(), 2);
    assert!(!PLAYBACK_SCRIPT.contains("setInterval(refreshState,2000)"));
    assert!(live_events::QUEUE_SCRIPT.contains("article[data-filename]"));
    assert!(live_events::QUEUE_SCRIPT.contains("updateArticle"));
    assert!(live_events::QUEUE_SCRIPT.contains("replaceChildren"));
    assert!(
        live_events::QUEUE_SCRIPT
            .contains("setInterval(()=>{if(!document.hidden)refresh()},15000)")
    );
    assert!(!live_events::QUEUE_SCRIPT.contains("http-equiv"));
}
