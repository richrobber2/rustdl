//! Public Snapchat Spotlight page requests.

use super::super::local;
use super::super::local::models::DiscoveryCandidate;
use reqwest::blocking::Client;
use std::error::Error;
use std::io::Read;

pub(in super::super) const MAX_SNAPCHAT_PAGE_BYTES: u64 = 2 * 1024 * 1024;

pub(in super::super) fn resolve_candidate(
    client: &Client,
    source_url: &str,
) -> Result<DiscoveryCandidate, Box<dyn Error>> {
    let spotlight_id =
        local::snapchat::spotlight_id(source_url).ok_or("invalid Snapchat Spotlight URL")?;
    let canonical = format!("https://www.snapchat.com/spotlight/{spotlight_id}");
    eprintln!("Resolving Snapchat Spotlight {spotlight_id}...");
    let response = client.get(source_url).send()?.error_for_status()?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_SNAPCHAT_PAGE_BYTES)
    {
        return Err("Snapchat returned an unexpectedly large page".into());
    }
    let final_url = response.url().clone();
    if !matches!(
        final_url.host_str().map(str::to_ascii_lowercase).as_deref(),
        Some("snapchat.com" | "www.snapchat.com")
    ) {
        return Err("Snapchat redirected outside snapchat.com".into());
    }
    let mut page = Vec::new();
    response
        .take(MAX_SNAPCHAT_PAGE_BYTES + 1)
        .read_to_end(&mut page)?;
    if page.len() as u64 > MAX_SNAPCHAT_PAGE_BYTES {
        return Err("Snapchat returned an unexpectedly large page".into());
    }
    let html = String::from_utf8(page)?;
    local::snapchat::candidate_from_html(&spotlight_id, &canonical, &final_url, &html)
}
