//! Peer server startup and outbound-transfer orchestration.

use super::super::local::peers::{PEER_CSS, PEER_PORT, PEER_SERVER_STARTED, PeerSendJob};
use super::super::{external, local, workflows};
use reqwest::Url;
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::error::Error;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::{fs, thread};
use tiny_http::{Request, Server};

pub(in super::super) fn start_peer_server(bind: String, output_dir: PathBuf) {
    if let Some(port) = bind
        .rsplit_once(':')
        .and_then(|(_, value)| value.parse().ok())
    {
        let _ = PEER_PORT.set(port);
    }
    PEER_SERVER_STARTED.call_once(move || {
        thread::spawn(move || match Server::http(&bind) {
            Ok(server) => {
                eprintln!("RustDL encrypted peer receiver listening on {bind}");
                for request in server.incoming_requests() {
                    let output_dir = output_dir.clone();
                    thread::spawn(move || {
                        if let Err(error) = local::peers::handle_peer_request(request, &output_dir)
                        {
                            eprintln!("peer request error: {error}");
                        }
                    });
                }
            }
            Err(error) => eprintln!("peer receiver could not bind {bind}: {error}"),
        });
    });
}

pub(in super::super) fn respond_peer_send_paired_post(
    mut request: Request,
    client: &Client,
    output_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    const MAX_PAIRED_FORM_BYTES: u64 = 1024;
    let mut bytes = Vec::new();
    request
        .as_reader()
        .take(MAX_PAIRED_FORM_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PAIRED_FORM_BYTES {
        return local::html::respond_text(request, 413, "Send form is too large");
    }
    let form = String::from_utf8(bytes)?;
    let parsed = Url::parse(&format!("http://localhost/?{form}"))?;
    let filename = parsed
        .query_pairs()
        .find(|(name, _)| name == "file")
        .map(|(_, value)| value.into_owned());
    let Some(pairing) = local::peers::current_outbound_peer_pairing() else {
        return local::html::respond_text(
            request,
            410,
            "Pairing expired; scan a new RustDL QR code",
        );
    };
    let key = local::format::hex_encode(&pairing.key);
    workflows::peers::start_peer_send(
        request,
        client,
        output_dir,
        filename.as_deref(),
        Some(&pairing.address),
        Some(&key),
    )
}

pub(in super::super) fn respond_peer_send_post(
    mut request: Request,
    client: &Client,
    output_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    const MAX_PAIRING_FORM_BYTES: u64 = 4 * 1024;
    let mut bytes = Vec::new();
    request
        .as_reader()
        .take(MAX_PAIRING_FORM_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PAIRING_FORM_BYTES {
        return local::html::respond_text(request, 413, "Pairing form is too large");
    }
    let form = String::from_utf8(bytes)?;
    let parsed = Url::parse(&format!("http://localhost/?{form}"))?;
    let values = parsed
        .query_pairs()
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect::<HashMap<_, _>>();
    workflows::peers::start_peer_send(
        request,
        client,
        output_dir,
        values.get("file").map(String::as_str),
        values.get("address").map(String::as_str),
        values.get("key").map(String::as_str),
    )
}

/// Starts the existing encrypted transfer independently of HTTP UI rendering.
pub(in super::super) fn schedule_peer_send(
    client: &Client,
    output_dir: &Path,
    filename: Option<&str>,
    address: Option<&str>,
    key: Option<&str>,
) -> Result<(), (u16, String)> {
    let Some(filename) = filename.filter(|value| local::media::valid_video_filename(value)) else {
        return Err((400, "Invalid media filename".to_owned()));
    };
    let path = output_dir.join(filename);
    if !local::files::is_complete_download(&path).map_err(|error| (500, error.to_string()))? {
        return Err((
            409,
            "Finish downloading this item before sending it".to_owned(),
        ));
    }
    let Some(address) = address else {
        return Err((400, "Missing receiver address".to_owned()));
    };
    let base = match local::peers::peer_base_url(address) {
        Ok(base) => base,
        Err(error) => return Err((400, error.to_string())),
    };
    let Some(key) = key else {
        return Err((400, "Missing pairing key".to_owned()));
    };
    let key = match local::peers::decode_peer_key(key) {
        Ok(key) => key,
        Err(error) => return Err((400, error.to_string())),
    };
    let total = fs::metadata(&path)
        .map_err(|error| (500, error.to_string()))?
        .len();
    local::peers::set_peer_send_job(
        filename,
        PeerSendJob {
            phase: "hashing".to_owned(),
            sent: 0,
            total,
            error: None,
            peer: address.to_owned(),
        },
    );
    let client = client.clone();
    let filename_owned = filename.to_owned();
    thread::spawn(move || {
        if let Err(error) =
            external::peers::send_file_to_peer(&client, &base, &key, &path, &filename_owned)
        {
            local::peers::update_peer_send_job(&filename_owned, |job| {
                job.phase = "failed".to_owned();
                job.error = Some(error.to_string());
            });
        }
    });
    Ok(())
}

pub(in super::super) fn start_peer_send(
    request: Request,
    client: &Client,
    output_dir: &Path,
    filename: Option<&str>,
    address: Option<&str>,
    key: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    if let Err((status, detail)) = schedule_peer_send(client, output_dir, filename, address, key) {
        return local::html::respond_text(request, status, &detail);
    }
    let filename = filename.unwrap_or_default();
    let body = {
        let display_filename = &(local::html::escape_html(filename));
        let filename_json = &(serde_json::to_string(filename)?);
        let dev_reload = &(local::dev::dev_reload_script());
        format!(
            include_str!("../../assets/html/transfer-progress.html"),
            PEER_CSS = PEER_CSS,
            display_filename = display_filename,
            dev_reload = dev_reload,
            page_script = include_str!("../../assets/js/transfer-progress.js")
                .replace("__RUSTDL_FILENAME_JSON__", &format!("{}", filename_json))
        )
    };
    local::html::respond_html(request, body)
}
