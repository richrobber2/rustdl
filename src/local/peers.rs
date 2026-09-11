//! Local pairing state, transfer UI, authenticated receive handling, and files.

use super::super::local;
use super::super::local::queue::{DownloadJob, DownloadPhase};
use super::super::local::runtime::PUBLISH_HOOK;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use qrcode::{Color as QrColor, QrCode};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, Once, OnceLock};
use tiny_http::{Method, Request, Response, StatusCode};

#[derive(Clone, Debug)]
pub(in super::super) struct PeerPairing {
    pub(in super::super) key: [u8; 32],
    pub(in super::super) expires: u64,
}

#[derive(Clone, Debug)]
pub(in super::super) struct OutboundPeerPairing {
    pub(in super::super) address: String,
    pub(in super::super) key: [u8; 32],
    pub(in super::super) expires: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(in super::super) struct PeerManifest {
    pub(in super::super) filename: String,
    pub(in super::super) size: u64,
    pub(in super::super) hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(in super::super) struct PeerSendJob {
    pub(in super::super) phase: String,
    pub(in super::super) sent: u64,
    pub(in super::super) total: u64,
    pub(in super::super) error: Option<String>,
    pub(in super::super) peer: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(in super::super) struct PeerStatus {
    pub(in super::super) offset: u64,
    pub(in super::super) complete: bool,
}

pub(in super::super) static PEER_PAIRING: OnceLock<Mutex<Option<PeerPairing>>> = OnceLock::new();

pub(in super::super) static OUTBOUND_PEER_PAIRING: OnceLock<Mutex<Option<OutboundPeerPairing>>> =
    OnceLock::new();

pub(in super::super) static PEER_SEND_JOBS: OnceLock<Mutex<HashMap<String, PeerSendJob>>> =
    OnceLock::new();

pub(in super::super) static PEER_RECEIVE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub(in super::super) static PEER_SERVER_STARTED: Once = Once::new();

pub(in super::super) static PEER_PORT: OnceLock<u16> = OnceLock::new();

pub(in super::super) const PEER_CHUNK_BYTES: usize = 1024 * 1024;

pub(in super::super) const PEER_PAIRING_SECONDS: u64 = 10 * 60;

pub(in super::super) fn peer_bind_for(app_bind: &str) -> Result<String, Box<dyn Error>> {
    let port = app_bind
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse::<u16>().ok())
        .ok_or("the app bind address has no valid port")?;
    let peer_port = port.checked_add(2).ok_or("the peer port is out of range")?;
    Ok(format!("0.0.0.0:{peer_port}"))
}

pub(in super::super) fn args_peer_port() -> u16 {
    PEER_PORT.get().copied().unwrap_or(37_660)
}

pub(in super::super) const PEER_CSS: &str = include_str!("../../assets/css/peers.css");

pub(in super::super) const PEER_PAIRING_SCRIPT: &str =
    include_str!("../../assets/js/peer-pairing.js");

#[derive(Serialize)]
pub(in super::super) struct PeerPairingView {
    pub(in super::super) qr: String,
    pub(in super::super) address: String,
    pub(in super::super) key: String,
}

pub(in super::super) fn generate_peer_pairing_view(
    peer_port: u16,
) -> Result<PeerPairingView, Box<dyn Error>> {
    let key = local::peers::enable_peer_pairing()?;
    let address = format!("{}:{peer_port}", local::peers::local_ipv4());
    let mut pairing_url = Url::parse("rustdl://pair")?;
    pairing_url
        .query_pairs_mut()
        .append_pair("address", &address)
        .append_pair("key", &key);
    let qr = local::peers::render_pairing_qr(pairing_url.as_str())?;
    Ok(PeerPairingView { qr, address, key })
}

pub(in super::super) fn respond_peer_pairing_refresh(
    request: Request,
    peer_port: u16,
) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(serde_json::to_string(
        &local::peers::generate_peer_pairing_view(peer_port)?,
    )?)
    .with_status_code(StatusCode(200))
    .with_header(local::html::header(
        "Content-Type",
        "application/json; charset=utf-8",
    ))
    .with_header(local::html::header("Cache-Control", "no-store"))
    .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

pub(in super::super) fn respond_peer_receive_page(
    request: Request,
    peer_port: u16,
) -> Result<(), Box<dyn Error>> {
    let PeerPairingView { qr, address, key } = local::peers::generate_peer_pairing_view(peer_port)?;
    let body = {
        let dev_reload = &(local::dev::dev_reload_script());
        format!(
            include_str!("../../assets/html/transfer-receive.html"),
            PEER_CSS = PEER_CSS,
            PEER_PAIRING_SCRIPT = PEER_PAIRING_SCRIPT,
            address = address,
            dev_reload = dev_reload,
            key = key,
            qr = qr
        )
    };
    local::html::respond_html(request, body)
}

pub(in super::super) fn render_pairing_qr(payload: &str) -> Result<String, Box<dyn Error>> {
    let code = QrCode::new(payload.as_bytes())?;
    let quiet = 4_usize;
    let size = code.width() + quiet * 2;
    let mut path = String::new();
    for y in 0..code.width() {
        for x in 0..code.width() {
            if code[(x, y)] == QrColor::Dark {
                path.push_str(&format!("M{} {}h1v1h-1z", x + quiet, y + quiet));
            }
        }
    }
    Ok(format!(
        r##"<svg class="pairing-qr" viewBox="0 0 {size} {size}" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="RustDL pairing QR code" shape-rendering="crispEdges"><rect width="{size}" height="{size}" rx="2" fill="#fff"/><path d="{path}" fill="#05070a"/></svg>"##
    ))
}

pub(in super::super) fn respond_peer_connected_page(
    request: Request,
    output_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    let Some(pairing) = local::peers::current_outbound_peer_pairing() else {
        let body = {
            let dev_reload = &(local::dev::dev_reload_script());
            format!(
                include_str!("../../assets/html/transfer-connect.html"),
                PEER_CSS = PEER_CSS,
                dev_reload = dev_reload
            )
        };
        return local::html::respond_html(request, body);
    };
    let mut filenames = Vec::new();
    if output_dir.is_dir() {
        for entry in fs::read_dir(output_dir)? {
            let entry = entry?;
            let filename = entry.file_name().to_string_lossy().into_owned();
            if local::media::valid_video_filename(&filename)
                && local::files::is_complete_download(&entry.path())?
            {
                filenames.push(filename);
            }
        }
    }
    filenames.sort_by_key(|filename| filename.to_ascii_lowercase());
    let rows = if filenames.is_empty() {
        r#"<p class="empty">No completed media yet. Finish a download, then return here.</p>"#
            .to_owned()
    } else {
        filenames
            .iter()
            .map(|filename| {
                let search_key = local::html::escape_html(&filename.to_lowercase());
                let filename = local::html::escape_html(filename);
                format!(
                    r#"<article class="media-row" data-name="{filename}" data-search-key="{search_key}"><strong>{filename}</strong><form action="/peers/send/paired" method="post"><input type="hidden" name="file" value="{filename}"><button type="submit">Send</button></form></article>"#
                )
            })
            .collect::<String>()
    };
    let body = {
        let receiver_address = &(local::html::escape_html(&pairing.address));
        let item_count = &(filenames.len());
        let receiver_label = &(local::html::escape_html(&pairing.address));
        let dev_reload = &(local::dev::dev_reload_script());
        format!(
            include_str!("../../assets/html/transfer-library.html"),
            PEER_CSS = PEER_CSS,
            receiver_address = receiver_address,
            item_count = item_count,
            dev_reload = dev_reload,
            page_script = include_str!("../../assets/js/transfer-library.js")
                .replace("__RUSTDL_RECEIVER_LABEL__", &format!("{}", receiver_label)),
            rows = rows
        )
    };
    local::html::respond_html(request, body)
}

pub(in super::super) fn respond_peer_send_page(
    request: Request,
    output_dir: &Path,
    filename: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let Some(filename) = filename.filter(|value| local::media::valid_video_filename(value)) else {
        return local::html::respond_text(
            request,
            400,
            "Choose Send to device from a saved media item",
        );
    };
    if !local::files::is_complete_download(&output_dir.join(filename))? {
        return local::html::respond_text(
            request,
            409,
            "Finish downloading this item before sending it",
        );
    }
    let paired = local::peers::current_outbound_peer_pairing().map_or_else(String::new, |pairing| {
        format!(
            r#"<form action="/peers/send/paired" method="post"><input type="hidden" name="file" value="{}"><button type="submit">Send to paired device · {}</button></form><p class="status">Or use manual pairing below.</p>"#,
            local::html::escape_html(filename),
            local::html::escape_html(&pairing.address)
        )
    });
    let body = {
        let display_filename = &(local::html::escape_html(filename));
        let form_filename = &(local::html::escape_html(filename));
        let dev_reload = &(local::dev::dev_reload_script());
        format!(
            include_str!("../../assets/html/transfer-send.html"),
            PEER_CSS = PEER_CSS,
            display_filename = display_filename,
            form_filename = form_filename,
            dev_reload = dev_reload,
            paired = paired
        )
    };
    local::html::respond_html(request, body)
}

pub(in super::super) fn respond_peer_send_state(request: Request) -> Result<(), Box<dyn Error>> {
    let parsed = Url::parse(&format!("http://localhost{}", request.url()))?;
    let filename = parsed
        .query_pairs()
        .find(|(key, _)| key == "file")
        .map(|(_, value)| value.into_owned())
        .filter(|value| local::media::valid_video_filename(value));
    let jobs = PEER_SEND_JOBS.get_or_init(|| Mutex::new(HashMap::new()));
    let jobs = jobs.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(job) = filename.as_deref().and_then(|filename| jobs.get(filename)) else {
        return local::html::respond_text(request, 404, "Transfer not found");
    };
    let response = Response::from_string(serde_json::to_string(job)?)
        .with_status_code(StatusCode(200))
        .with_header(local::html::header("Content-Type", "application/json"))
        .with_header(local::html::header("Cache-Control", "no-store"))
        .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

pub(in super::super) fn set_peer_send_job(filename: &str, job: PeerSendJob) {
    PEER_SEND_JOBS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(filename.to_owned(), job);
    local::runtime::notify_simple_event("peer");
}

pub(in super::super) fn update_peer_send_job(
    filename: &str,
    update: impl FnOnce(&mut PeerSendJob),
) {
    if let Some(job) = PEER_SEND_JOBS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get_mut(filename)
    {
        update(job);
    }
    local::runtime::notify_simple_event("peer");
}

pub(in super::super) fn enable_peer_pairing() -> Result<String, Box<dyn Error>> {
    let mut key = [0_u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut key)?;
    let expires = local::format::unix_seconds().saturating_add(PEER_PAIRING_SECONDS);
    *PEER_PAIRING
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(PeerPairing { key, expires });
    Ok(local::format::hex_encode(&key))
}

#[allow(dead_code)]
pub(crate) fn set_outbound_peer_pairing(address: &str, key: &str) -> Result<(), String> {
    let address = address.trim();
    local::peers::peer_base_url(address).map_err(|error| error.to_string())?;
    let key = local::peers::decode_peer_key(key).map_err(|error| error.to_string())?;
    let pairing = OutboundPeerPairing {
        address: address.to_owned(),
        key,
        expires: local::format::unix_seconds().saturating_add(PEER_PAIRING_SECONDS),
    };
    *OUTBOUND_PEER_PAIRING
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(pairing);
    Ok(())
}

pub(in super::super) fn current_outbound_peer_pairing() -> Option<OutboundPeerPairing> {
    let mut pairing = OUTBOUND_PEER_PAIRING
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if pairing
        .as_ref()
        .is_some_and(|value| value.expires <= local::format::unix_seconds())
    {
        *pairing = None;
    }
    pairing.clone()
}

pub(in super::super) fn local_ipv4() -> Ipv4Addr {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|socket| {
            socket.connect((Ipv4Addr::new(192, 0, 2, 1), 80))?;
            socket.local_addr()
        })
        .ok()
        .and_then(|address| match address.ip() {
            IpAddr::V4(ip) => Some(ip),
            IpAddr::V6(_) => None,
        })
        .unwrap_or(Ipv4Addr::LOCALHOST)
}

pub(in super::super) fn peer_base_url(address: &str) -> Result<Url, Box<dyn Error>> {
    let url = Url::parse(&format!("http://{address}/"))?;
    if url.username() != "" || url.password().is_some() || url.port().is_none() {
        return Err("receiver address must be a local IP address and port".into());
    }
    let ip = url
        .host_str()
        .and_then(|host| host.parse::<Ipv4Addr>().ok())
        .filter(|ip| ip.is_private() || ip.is_loopback() || ip.is_link_local())
        .ok_or("receiver must use a private, loopback, or link-local IPv4 address")?;
    let port = url.port().ok_or("receiver port is missing")?;
    Url::parse(&format!("http://{ip}:{port}/")).map_err(Into::into)
}

pub(in super::super) fn decode_peer_key(value: &str) -> Result<[u8; 32], Box<dyn Error>> {
    if value.len() != 64 {
        return Err("pairing key must contain 64 hexadecimal characters".into());
    }
    let mut key = [0_u8; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| "pairing key is not valid hexadecimal")?;
    }
    Ok(key)
}

pub(in super::super) fn append_peer_manifest_query(
    url: &mut Url,
    filename: &str,
    size: u64,
    hash: &str,
) {
    url.query_pairs_mut()
        .append_pair("file", filename)
        .append_pair("size", &size.to_string())
        .append_pair("hash", hash);
}

pub(in super::super) fn peer_chunk_aad(
    filename: &str,
    offset: u64,
    total: u64,
    hash: &str,
) -> String {
    format!("rustdl-v1\n{filename}\n{offset}\n{total}\n{hash}")
}

pub(in super::super) fn handle_peer_request(
    mut request: Request,
    output_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    let key = match local::peers::authenticate_peer_request(&request) {
        Some(key) => key,
        None => {
            return local::html::respond_text(
                request,
                401,
                "Pairing is missing, invalid, or expired",
            );
        }
    };
    if request.method() != &Method::Post {
        return local::html::respond_text(request, 405, "Method not allowed");
    }
    let parsed = Url::parse(&format!("http://peer{}", request.url()))?;
    let manifest = match local::peers::peer_manifest_from_url(&parsed) {
        Ok(manifest) => manifest,
        Err(error) => return local::html::respond_text(request, 400, &error),
    };
    fs::create_dir_all(output_dir)?;
    let _guard = PEER_RECEIVE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match parsed.path() {
        "/v1/status" => match local::peers::prepare_peer_receive(output_dir, &manifest) {
            Ok(status) => local::peers::respond_peer_status(request, status),
            Err(error) => local::html::respond_text(request, 422, &error.to_string()),
        },
        "/v1/chunk" => {
            let offset = parsed
                .query_pairs()
                .find(|(name, _)| name == "offset")
                .and_then(|(_, value)| value.parse::<u64>().ok());
            let Some(offset) = offset else {
                return local::html::respond_text(request, 400, "Chunk offset is missing");
            };
            match local::peers::receive_peer_chunk(
                &mut request,
                output_dir,
                &manifest,
                offset,
                &key,
            ) {
                Ok(()) => local::peers::respond_peer_status(
                    request,
                    PeerStatus {
                        offset: local::peers::peer_part_path(output_dir, &manifest.filename)
                            .metadata()?
                            .len(),
                        complete: false,
                    },
                ),
                Err(error) => local::html::respond_text(request, 422, &error.to_string()),
            }
        }
        "/v1/finish" => match local::peers::finish_peer_receive(output_dir, &manifest) {
            Ok(status) => local::peers::respond_peer_status(request, status),
            Err(error) => local::html::respond_text(request, 422, &error.to_string()),
        },
        _ => local::html::respond_text(request, 404, "Peer endpoint not found"),
    }
}

pub(in super::super) fn authenticate_peer_request(request: &Request) -> Option<[u8; 32]> {
    let supplied = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Authorization"))?
        .value
        .as_str()
        .strip_prefix("RustDL ")
        .and_then(|value| local::peers::decode_peer_key(value).ok())?;
    let now = local::format::unix_seconds();
    let mut pairing = PEER_PAIRING
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let active = pairing.as_mut()?;
    if active.expires < now {
        *pairing = None;
        return None;
    }
    let different = active
        .key
        .iter()
        .zip(supplied.iter())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        });
    if different != 0 {
        return None;
    }
    active.expires = now.saturating_add(PEER_PAIRING_SECONDS);
    Some(active.key)
}

pub(in super::super) fn peer_manifest_from_url(url: &Url) -> Result<PeerManifest, String> {
    let values = url.query_pairs().collect::<HashMap<_, _>>();
    let filename = values
        .get("file")
        .map(|value| value.to_string())
        .filter(|value| local::media::valid_video_filename(value))
        .ok_or_else(|| "Media filename is invalid".to_owned())?;
    let size = values
        .get("size")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|size| *size > 0)
        .ok_or_else(|| "Media size is invalid".to_owned())?;
    let hash = values
        .get("hash")
        .map(|value| value.to_string())
        .filter(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| "Media digest is invalid".to_owned())?;
    Ok(PeerManifest {
        filename,
        size,
        hash: hash.to_ascii_lowercase(),
    })
}

pub(in super::super) fn prepare_peer_receive(
    output_dir: &Path,
    manifest: &PeerManifest,
) -> Result<PeerStatus, Box<dyn Error>> {
    let output = output_dir.join(&manifest.filename);
    if output.is_file() {
        if fs::metadata(&output)?.len() == manifest.size
            && local::files::blake3_file(&output)? == manifest.hash
        {
            if let Some(publish) = PUBLISH_HOOK.get() {
                publish(&output, &manifest.filename)
                    .map_err(|error| format!("could not publish received media: {error}"))?;
            }
            return Ok(PeerStatus {
                offset: manifest.size,
                complete: true,
            });
        }
        return Err("a different local file already uses this media name".into());
    }
    let partial = local::peers::peer_part_path(output_dir, &manifest.filename);
    let metadata = local::peers::peer_manifest_path(output_dir, &manifest.filename);
    let existing = fs::read_to_string(&metadata)
        .ok()
        .and_then(|value| serde_json::from_str::<PeerManifest>(&value).ok());
    if existing.as_ref() != Some(manifest) {
        local::files::remove_if_exists(&partial)?;
        local::files::remove_if_exists(&metadata)?;
        fs::write(&metadata, serde_json::to_vec(manifest)?)?;
    }
    let offset = fs::metadata(partial).map(|value| value.len()).unwrap_or(0);
    if offset > manifest.size {
        return Err("the saved peer partial exceeds the declared media size".into());
    }
    Ok(PeerStatus {
        offset,
        complete: false,
    })
}

pub(in super::super) fn receive_peer_chunk(
    request: &mut Request,
    output_dir: &Path,
    manifest: &PeerManifest,
    offset: u64,
    key: &[u8; 32],
) -> Result<(), Box<dyn Error>> {
    let partial = local::peers::peer_part_path(output_dir, &manifest.filename);
    let saved = fs::metadata(&partial).map(|value| value.len()).unwrap_or(0);
    if saved != offset {
        return Err(format!("resume offset mismatch: receiver has {saved} bytes").into());
    }
    let stored_manifest: PeerManifest = serde_json::from_slice(&fs::read(
        local::peers::peer_manifest_path(output_dir, &manifest.filename),
    )?)?;
    if &stored_manifest != manifest {
        return Err("peer manifest changed during transfer".into());
    }
    let max_encrypted = PEER_CHUNK_BYTES + 24 + 16;
    let mut body = Vec::new();
    request
        .as_reader()
        .take((max_encrypted + 1) as u64)
        .read_to_end(&mut body)?;
    if body.len() <= 24 || body.len() > max_encrypted {
        return Err("encrypted peer chunk has an invalid size".into());
    }
    let (nonce, encrypted) = body.split_at(24);
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| "could not initialize peer decryption")?;
    let aad =
        local::peers::peer_chunk_aad(&manifest.filename, offset, manifest.size, &manifest.hash);
    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: encrypted,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| "peer chunk authentication failed")?;
    if plaintext.is_empty()
        || plaintext.len() > PEER_CHUNK_BYTES
        || offset + plaintext.len() as u64 > manifest.size
    {
        return Err("decrypted peer chunk has an invalid size".into());
    }
    let mut file = OpenOptions::new().create(true).append(true).open(partial)?;
    file.write_all(&plaintext)?;
    file.flush()?;
    Ok(())
}

pub(in super::super) fn finish_peer_receive(
    output_dir: &Path,
    manifest: &PeerManifest,
) -> Result<PeerStatus, Box<dyn Error>> {
    let partial = local::peers::peer_part_path(output_dir, &manifest.filename);
    if fs::metadata(&partial)?.len() != manifest.size
        || local::files::blake3_file(&partial)? != manifest.hash
    {
        return Err("received media failed its BLAKE3 verification".into());
    }
    let output = output_dir.join(&manifest.filename);
    fs::rename(&partial, &output)?;
    if let Err(error) =
        local::storage::record_file_fingerprint(output_dir, &manifest.filename, &manifest.hash)
    {
        eprintln!("could not cache received media fingerprint: {error}");
    }
    local::files::remove_if_exists(&local::peers::peer_manifest_path(
        output_dir,
        &manifest.filename,
    ))?;
    if let Some(publish) = PUBLISH_HOOK.get() {
        publish(&output, &manifest.filename)
            .map_err(|error| format!("could not publish received media: {error}"))?;
    }
    local::queue::set_download_job(
        &manifest.filename,
        DownloadJob {
            phase: DownloadPhase::Ready,
            downloaded: manifest.size,
            total: Some(manifest.size),
            error: None,
            source_url: None,
            media_url: None,
            audio_url: None,
            extract_audio: false,
            quality_label: Some("Received from RustDL".to_owned()),
            quality_height: None,
        },
    );
    local::queue::persist_download_jobs();
    Ok(PeerStatus {
        offset: manifest.size,
        complete: true,
    })
}

pub(in super::super) fn peer_part_path(output_dir: &Path, filename: &str) -> PathBuf {
    output_dir.join(format!(".{filename}.peer.part"))
}

pub(in super::super) fn peer_manifest_path(output_dir: &Path, filename: &str) -> PathBuf {
    output_dir.join(format!(".{filename}.peer.json"))
}

pub(in super::super) fn respond_peer_status(
    request: Request,
    status: PeerStatus,
) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(serde_json::to_string(&status)?)
        .with_status_code(StatusCode(200))
        .with_header(local::html::header("Content-Type", "application/json"))
        .with_header(local::html::header("Cache-Control", "no-store"))
        .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}
