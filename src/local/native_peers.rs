//! Peer UI snapshots and commands, invoked only by app controls.
use super::super::{external, workflows};
use super::peers::PeerSendJob;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Mutex;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    #[serde(default)]
    id: String,
    #[serde(default)]
    address: String,
    #[serde(default)]
    key: String,
}
#[derive(Clone)]
struct ReceiveView {
    address: String,
    key: String,
    size: usize,
    cells: Vec<bool>,
}
static RECEIVER: Mutex<Option<ReceiveView>> = Mutex::new(None);
fn receive_fields(view: Option<&ReceiveView>, privacy: bool) -> Value {
    json!({"receiveEnabled":view.is_some(),
        "receiveAddress":if privacy {""} else {view.map_or("",|v|v.address.as_str())},
        "receiveKey":if privacy {""} else {view.map_or("",|v|v.key.as_str())},
        "qrSize":if privacy {0} else {view.map_or(0,|v|v.size)},
        "qrCells":if privacy {Vec::new()} else {view.map(|v|v.cells.clone()).unwrap_or_default()}})
}
fn current_receiver() -> Option<ReceiveView> {
    let current = super::peers::PEER_PAIRING
        .get()?
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()?;
    if current.expires <= super::format::unix_seconds() {
        return None;
    }
    let view = RECEIVER.lock().unwrap_or_else(|p| p.into_inner()).clone()?;
    (view.key == super::format::hex_encode(&current.key)).then_some(view)
}
fn make_receive_view(address: String, key: String) -> Result<ReceiveView, &'static str> {
    let mut url =
        reqwest::Url::parse("rustdl://pair").map_err(|_| "Could not create pairing code")?;
    url.query_pairs_mut()
        .append_pair("address", &address)
        .append_pair("key", &key);
    let code = qrcode::QrCode::new(url.as_str().as_bytes())
        .map_err(|_| "Could not create pairing code")?;
    let size = code.width() + 8;
    let mut cells = vec![false; size * size];
    for y in 0..code.width() {
        for x in 0..code.width() {
            cells[(y + 4) * size + x + 4] = code[(x, y)] == qrcode::Color::Dark;
        }
    }
    Ok(ReceiveView {
        address,
        key,
        size,
        cells,
    })
}
fn transfer_row(job: Option<&PeerSendJob>) -> Value {
    let phase = job.map(|job| job.phase.as_str()).unwrap_or("idle");
    let phase = match phase {
        "idle" | "hashing" | "sending" | "transferring" | "verifying" | "complete" | "ready"
        | "failed" => phase,
        _ => "working",
    };
    json!({"phase":phase,"sent":job.map_or(0,|job|job.sent),"total":job.map_or(0,|job|job.total),
        "issue":job.is_some_and(|job|job.error.is_some())})
}
fn snapshot(id: &str, privacy: bool) -> Value {
    let paired = super::peers::current_outbound_peer_pairing();
    let filename = super::native_api::library_media(id);
    let job = filename.as_ref().and_then(|name| {
        super::peers::PEER_SEND_JOBS.get().and_then(|jobs| {
            jobs.lock()
                .unwrap_or_else(|p| p.into_inner())
                .get(name)
                .cloned()
        })
    });
    let mut data = transfer_row(job.as_ref());
    data["ok"] = json!(true);
    data["paired"] = json!(paired.is_some());
    data["itemSelected"] = json!(filename.is_some());
    data["title"] = json!(if privacy {
        "Downloaded media".to_owned()
    } else {
        filename.clone().unwrap_or_default()
    });
    data["address"] = json!(if privacy {
        String::new()
    } else {
        paired.map(|p| p.address).unwrap_or_default()
    });
    if let Value::Object(fields) = receive_fields(current_receiver().as_ref(), privacy) {
        for (key, value) in fields {
            data[key] = value;
        }
    }
    data
}
pub(crate) fn command(action: &str, payload: &str, privacy: bool) -> String {
    let result = (|| -> Result<Value, &'static str> {
        if payload.len() > 8192 {
            return Err("Peer request is too large");
        }
        let request: Request = serde_json::from_str(payload).map_err(|_| "Invalid peer request")?;
        match action {
            "snapshot" => Ok(snapshot(&request.id, privacy)),
            "receive" => {
                let port = super::peers::PEER_PORT
                    .get()
                    .copied()
                    .ok_or("Receiver is starting. Try again.")?;
                let view = super::peers::generate_peer_pairing_view(port)
                    .map_err(|_| "Could not enable receiving")?;
                *RECEIVER.lock().unwrap_or_else(|p| p.into_inner()) =
                    Some(make_receive_view(view.address, view.key)?);
                Ok(snapshot(&request.id, privacy))
            }
            "copy" => {
                let view = current_receiver().ok_or("Pairing expired. Generate a new code.")?;
                // Android consumes this action result directly; it never reaches GPUI snapshots.
                Ok(json!({"ok":true,"copyText":format!("{}\n{}",view.address,view.key)}))
            }
            "pair" => {
                super::peers::set_outbound_peer_pairing(&request.address, &request.key)
                    .map_err(|_| "Invalid receiver address or pairing key")?;
                Ok(snapshot(&request.id, privacy))
            }
            "send" => {
                let filename = super::native_api::library_media(&request.id)
                    .ok_or("Select a saved item from the library")?;
                let pairing = super::peers::current_outbound_peer_pairing()
                    .ok_or("Pair with a receiving device first")?;
                if let Some(jobs) = super::peers::PEER_SEND_JOBS.get() {
                    let jobs = jobs.lock().unwrap_or_else(|p| p.into_inner());
                    if jobs.get(&filename).is_some_and(|job| {
                        !matches!(job.phase.as_str(), "complete" | "ready" | "failed")
                    }) {
                        return Err("This transfer is already running");
                    }
                }
                let output = super::queue::QUEUE_OUTPUT_DIR
                    .get()
                    .ok_or("Download engine is starting")?;
                let client =
                    external::http::build_client().map_err(|_| "Could not start transfer")?;
                workflows::peers::schedule_peer_send(
                    &client,
                    output,
                    Some(&filename),
                    Some(&pairing.address),
                    Some(&super::format::hex_encode(&pairing.key)),
                )
                .map_err(|_| "Could not send this item. Check pairing and download completion.")?;
                Ok(snapshot(&request.id, privacy))
            }
            _ => Err("Unsupported peer action"),
        }
    })();
    result
        .unwrap_or_else(|detail| json!({"ok":false,"detail":detail}))
        .to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_peer_progress_never_exposes_address_error_or_pairing_key() {
        let job = PeerSendJob {
            phase: "sending".into(),
            sent: 12,
            total: 24,
            error: Some("private-title.mp4 /private/path token-secret".into()),
            peer: "private-host".into(),
        };
        let row = transfer_row(Some(&job));
        assert_eq!(row["sent"], 12);
        assert_eq!(row["total"], 24);
        assert_eq!(row["issue"], true);
        for hidden in ["private", "secret", "peer", "key", "error"] {
            assert!(!row.to_string().contains(hidden));
        }
        assert_eq!(transfer_row(None)["phase"], "idle");
        let receive = make_receive_view("192.0.2.1:9001".into(), "synthetic-key".into()).unwrap();
        let redacted = receive_fields(Some(&receive), true);
        assert_eq!(redacted["qrSize"], 0);
        assert_eq!(redacted["qrCells"], json!([]));
        assert!(!redacted.to_string().contains("synthetic-key"));
        assert!(!redacted.to_string().contains("192.0.2.1"));
        assert_eq!(receive.cells.len(), receive.size * receive.size);
        assert!(receive.cells.iter().any(|cell| *cell));
        assert!(receive.cells[..4 * receive.size].iter().all(|cell| !*cell));

        assert_eq!(
            serde_json::from_str::<Value>(&command("unsupported", "{}", true)).unwrap()["ok"],
            false
        );
    }
}
