//! HTML parsing and escaping, response headers, and local page responses.

use super::super::local;
use serde::Serialize;
use std::error::Error;
use std::io;
use tiny_http::{Header, Request, Response, StatusCode};

pub(in super::super) fn respond_html(request: Request, body: String) -> Result<(), Box<dyn Error>> {
    let response = local::html::html_response(body)
        .with_header(local::html::header("Cache-Control", "no-store"));
    request.respond(response)?;
    Ok(())
}

pub(in super::super) fn html_response(body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(local::web_assets::decorate_app_html(body))
        .with_status_code(StatusCode(200))
        .with_header(local::html::header(
            "Content-Type",
            "text/html; charset=utf-8",
        ))
        .with_header(local::html::html_csp())
        .with_header(local::html::header("X-Content-Type-Options", "nosniff"))
}

pub(in super::super) fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(in super::super) fn html_csp() -> Header {
    local::html::header(
        "Content-Security-Policy",
        "default-src 'none'; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; img-src 'self'; media-src 'self'; connect-src 'self'; frame-src 'self'; form-action 'self'; base-uri 'none'",
    )
}

pub(in super::super) fn respond_text(
    request: Request,
    status: u16,
    body: &str,
) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(body)
        .with_status_code(StatusCode(status))
        .with_header(local::html::header(
            "Content-Type",
            "text/plain; charset=utf-8",
        ))
        .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

pub(in super::super) fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name, value).expect("static header name and safe header value")
}

pub(in super::super) fn html_meta_content(html: &str, wanted: &str) -> Option<String> {
    html.split("<meta").skip(1).find_map(|remainder| {
        let tag = remainder.split_once('>')?.0;
        let key = local::html::html_attribute(tag, "property")
            .or_else(|| local::html::html_attribute(tag, "name"))?;
        (key.eq_ignore_ascii_case(wanted)).then(|| {
            local::html::html_attribute(tag, "content").map(local::html::html_entity_decode)
        })?
    })
}

pub(in super::super) fn html_attribute<'a>(tag: &'a str, wanted: &str) -> Option<&'a str> {
    let bytes = tag.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let key_start = index;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'-' | b'_' | b':'))
        {
            index += 1;
        }
        if key_start == index {
            index += 1;
            continue;
        }
        let key = &tag[key_start..index];
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) != Some(&b'=') {
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let quote = *bytes.get(index)?;
        if !matches!(quote, b'\'' | b'"') {
            continue;
        }
        index += 1;
        let value_start = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index >= bytes.len() {
            return None;
        }
        if key.eq_ignore_ascii_case(wanted) {
            return Some(&tag[value_start..index]);
        }
        index += 1;
    }
    None
}

pub(in super::super) fn html_entity_decode(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&#039;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

pub(in super::super) fn safe_script_json<T: Serialize>(value: &T) -> io::Result<String> {
    serde_json::to_string(value)
        .map(|json| {
            json.replace('<', "\\u003c")
                .replace('>', "\\u003e")
                .replace('&', "\\u0026")
        })
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::html_entity_decode;

    #[test]
    fn decodes_common_apostrophe_entities_without_leaking_markup() {
        assert_eq!(
            html_entity_decode("It&#039;s Always Love"),
            "It's Always Love"
        );
        assert_eq!(
            html_entity_decode("It&apos;s Always Love"),
            "It's Always Love"
        );
        assert_eq!(html_entity_decode("It&amp;#039;s"), "It's");
    }
}
