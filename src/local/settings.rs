//! Local settings UI backed by Android preferences.

pub(crate) fn render(dev_reload: &str) -> String {
    format!(
        include_str!("../../assets/html/settings.html"),
        page_css = include_str!("../../assets/css/settings.css"),
        page_script = include_str!("../../assets/js/settings.js")
    )
    .replace("<!--DEV_RELOAD-->", dev_reload)
}
