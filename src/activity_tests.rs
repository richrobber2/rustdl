use super::*;

#[test]
fn activity_center_is_live_filterable_and_actionable() {
    let html = activity::render("");
    assert!(html.contains("RustDL Activity Center"));
    assert!(html.contains("data-filter=\"active\""));
    assert!(html.contains("data-filter=\"issue\""));
    assert!(html.contains("/__app/activity.json"));
    assert!(html.contains("rustdl:state"));
    assert!(html.contains("setInterval"));
    assert!(html.contains("textContent=item.filename"));
    assert!(html.contains("'/watch/'"));
}

#[test]
fn activity_snapshot_omits_sensitive_transfer_fields() {
    let source = include_str!("activity_state.rs");
    let transfer_json = source
        .split("\"transfers\":")
        .nth(1)
        .expect("transfer snapshot");
    assert!(transfer_json.contains("\"filename\""));
    assert!(transfer_json.contains("\"phase\""));
    assert!(!transfer_json.contains("\"peer\""));
    assert!(source.contains("storageLow"));
    assert!(source.contains("take(60)"));
}

#[test]
fn native_updater_and_rust_events_feed_activity_center() {
    let activity_java = include_str!("../android/MainActivity.java");
    let update_java = include_str!("../android/UpdateManager.java");
    let bridge_java = include_str!("../android/ActivityBridge.java");
    assert!(activity_java.contains("addJavascriptInterface(activityBridge, \"RustDLActivity\")"));
    assert!(activity_java.contains("\"peer\".equals(type)"));
    assert!(activity_java.contains("activityCenterStatus()"));
    assert!(update_java.contains("String activityStatus()"));
    assert!(update_java.contains("setActivityState(\"checking\""));
    assert!(update_java.contains("setActivityState(\"ready\""));
    assert!(bridge_java.contains("@JavascriptInterface"));
}

#[test]
fn activity_center_stays_out_of_inspection_mode() {
    let source = include_str!("main.rs");
    assert!(source.contains("\"/activity\" if !inspection_mode()"));
    assert!(source.contains("\"/__app/activity.json\" if !inspection_mode()"));
    assert!(source.contains("id=\"activity-link\""));
}
