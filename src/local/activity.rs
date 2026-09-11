//! Activity page markup and browser-side state rendering.

pub(crate) fn render(dev_reload: &str) -> String {
    format!(
        include_str!("../../assets/html/activity.html"),
        page_css = include_str!("../../assets/css/activity.css"),
        page_script = include_str!("../../assets/js/activity.js")
    )
    .replace("<!--DEV_RELOAD-->", dev_reload)
}
