use super::{INDEX_HTML, settings};

#[test]
fn settings_are_persistent_apk_local_and_scoped() {
    let html = settings::render("");
    let bridge = include_str!("../android/SettingsBridge.java");
    let activity = include_str!("../android/MainActivity.java");
    assert!(INDEX_HTML.contains(r#"href="/settings""#));
    assert!(html.contains("Completed download folder"));
    assert!(html.contains("RustDLSettings"));
    assert!(html.contains("bridge.save"));
    assert!(html.contains("Restore defaults"));
    assert!(html.contains("Existing exports stay where they are"));
    assert!(html.contains("Moving space background"));
    assert!(html.contains(r#"id="appearance""#));
    assert!(bridge.contains("SharedPreferences"));
    assert!(bridge.contains("normalizeFolder"));
    assert!(bridge.contains("diagnosticsRefreshSeconds"));
    assert!(bridge.contains("setAppearance"));
    assert!(bridge.contains("spaceEffectEnabled"));
    assert!(activity.contains("published-path:"));
    assert!(activity.contains("settingsBridge.relativeDownloadPath()"));
}
