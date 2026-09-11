//! Encrypted outbound HTTP transfers to a paired device.

use super::super::local;
use super::super::local::peers::{PEER_CHUNK_BYTES, PeerStatus};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use reqwest::Url;
use reqwest::blocking::Client;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub(in super::super) fn send_file_to_peer(
    client: &Client,
    base: &Url,
    key: &[u8; 32],
    path: &Path,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    let total = fs::metadata(path)?.len();
    let hash = local::files::blake3_file(path)?;
    let authorization = format!("RustDL {}", local::format::hex_encode(key));
    let mut status_url = base.join("v1/status")?;
    local::peers::append_peer_manifest_query(&mut status_url, filename, total, &hash);
    let status: PeerStatus = client
        .post(status_url)
        .header("Authorization", &authorization)
        .send()?
        .error_for_status()?
        .json()?;
    if status.complete {
        local::peers::update_peer_send_job(filename, |job| {
            job.phase = "ready".to_owned();
            job.sent = total;
        });
        return Ok(());
    }
    if status.offset > total {
        return Err("receiver reported an invalid resume offset".into());
    }

    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| "could not initialize peer encryption")?;
    let mut input = File::open(path)?;
    input.seek(SeekFrom::Start(status.offset))?;
    let mut offset = status.offset;
    let mut buffer = vec![0_u8; PEER_CHUNK_BYTES];
    local::peers::update_peer_send_job(filename, |job| {
        job.phase = "sending".to_owned();
        job.sent = offset;
    });
    while offset < total {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            return Err("the source file ended before its declared size".into());
        }
        let mut nonce = [0_u8; 24];
        File::open("/dev/urandom")?.read_exact(&mut nonce)?;
        let aad = local::peers::peer_chunk_aad(filename, offset, total, &hash);
        let encrypted = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &buffer[..count],
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| "could not encrypt peer chunk")?;
        let mut body = Vec::with_capacity(nonce.len() + encrypted.len());
        body.extend_from_slice(&nonce);
        body.extend_from_slice(&encrypted);
        let mut chunk_url = base.join("v1/chunk")?;
        local::peers::append_peer_manifest_query(&mut chunk_url, filename, total, &hash);
        chunk_url
            .query_pairs_mut()
            .append_pair("offset", &offset.to_string());
        let next: PeerStatus = client
            .post(chunk_url)
            .header("Authorization", &authorization)
            .header("Content-Type", "application/octet-stream")
            .body(body)
            .send()?
            .error_for_status()?
            .json()?;
        let expected = offset + count as u64;
        if next.offset != expected {
            return Err("receiver did not acknowledge the complete chunk".into());
        }
        offset = next.offset;
        local::peers::update_peer_send_job(filename, |job| job.sent = offset);
    }
    let mut finish_url = base.join("v1/finish")?;
    local::peers::append_peer_manifest_query(&mut finish_url, filename, total, &hash);
    let finished: PeerStatus = client
        .post(finish_url)
        .header("Authorization", authorization)
        .send()?
        .error_for_status()?
        .json()?;
    if !finished.complete || finished.offset != total {
        return Err("receiver did not verify the completed file".into());
    }
    local::peers::update_peer_send_job(filename, |job| {
        job.phase = "ready".to_owned();
        job.sent = total;
    });
    Ok(())
}
