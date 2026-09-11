//! Diagnostics page rendering and responses.

use super::super::super::local;
use std::error::Error;
use tiny_http::Request;

pub(in super::super::super) fn render() -> String {
    {
        let app_version = &(env!("CARGO_PKG_VERSION"));
        let dev_reload = &(local::dev::dev_reload_script());
        format!(
            include_str!("../../../assets/html/diagnostics.html"),
            app_version = app_version,
            dev_reload = dev_reload,
            page_css = include_str!("../../../assets/css/diagnostics.css"),
            page_script = include_str!("../../../assets/js/diagnostics.js")
        )
    }
}

pub(in super::super::super) fn respond(request: Request) -> Result<(), Box<dyn Error>> {
    local::html::respond_html(request, local::pages::diagnostics::render())
}
