//! Settings page rendering and responses.

use super::super::super::local;
use std::error::Error;
use tiny_http::Request;

pub(in super::super::super) fn respond(request: Request) -> Result<(), Box<dyn Error>> {
    local::html::respond_html(
        request,
        local::settings::render(&local::dev::dev_reload_script()),
    )
}
