//! Activity projection for native UI. Agent tools must never call real snapshots.
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Request {
    offset: usize,
    filter: String,
}
fn row(item: &Value, kind: &str, id: &str, privacy: bool) -> Value {
    let phase = match item["phase"].as_str().unwrap_or("unknown") {
        value @ ("queued" | "starting" | "downloading" | "paused" | "failed" | "ready"
        | "cancelled" | "hashing" | "sending" | "transferring" | "verifying") => value,
        _ => "working",
    };
    let label = match phase {
        "queued" => "Queued",
        "starting" => "Starting",
        "downloading" => "Downloading",
        "paused" => "Paused",
        "failed" => "Failed",
        "ready" => "Completed",
        "cancelled" => "Cancelled",
        "hashing" => "Preparing transfer",
        "sending" | "transferring" => "Transferring",
        "verifying" => "Verifying",
        _ => "Working",
    };
    let category = match phase {
        "failed" | "paused" => "issue",
        "ready" | "cancelled" => "complete",
        _ => "active",
    };
    json!({"id":id,"kind":kind,"category":category,"phase":phase,"phaseLabel":label,
        "title":if privacy {"Downloaded media"}else{item["filename"].as_str().unwrap_or("Downloaded media")},
        "done":item[if kind=="transfer" {"sent"}else{"downloaded"}].as_u64().unwrap_or(0),
        "total":item["total"].as_u64().unwrap_or(0),"issue":item["error"].as_str().is_some_and(|s|!s.is_empty()),
        "issueDetail":if privacy {None}else{item["error"].as_str()},"canPlay":kind=="download"&&phase=="ready"})
}
pub(crate) fn snapshot(payload: &str, privacy: bool) -> String {
    let result = (|| -> Result<Value, &'static str> {
        if payload.len() > 1024 {
            return Err("Activity request is too large");
        }
        let request: Request =
            serde_json::from_str(payload).map_err(|_| "Invalid activity request")?;
        if request.offset > 100000
            || !matches!(
                request.filter.as_str(),
                "" | "all" | "active" | "issue" | "complete"
            )
        {
            return Err("Invalid activity filter");
        }
        let state = super::activity_state::snapshot();
        let all: Vec<_> = [("downloads", "download"), ("transfers", "transfer")]
            .into_iter()
            .flat_map(|(key, kind)| {
                state[key]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(move |item| (item, kind))
            })
            .collect();
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|(item, kind)| {
                let category = row(item, kind, "", true)["category"]
                    .as_str()
                    .unwrap_or("active")
                    .to_owned();
                matches!(request.filter.as_str(), "" | "all") || request.filter == category
            })
            .collect();
        let offset = if filtered.is_empty() {
            0
        } else {
            request.offset.min((filtered.len() - 1) / 25 * 25)
        };
        let items: Vec<_> = filtered
            .iter()
            .skip(offset)
            .take(25)
            .map(|(item, kind)| {
                let id = super::native_api::library_handle(item["filename"].as_str().unwrap_or(""))
                    .unwrap_or_default();
                row(item, kind, &id, privacy)
            })
            .collect();
        let counts = &state["counts"];
        let system = &state["system"];
        Ok(
            json!({"ok":true,"offset":offset,"totalItems":filtered.len(),"items":items,
            "active":counts["active"].as_u64().unwrap_or(0),"issues":counts["issues"].as_u64().unwrap_or(0),"completed":counts["completed"].as_u64().unwrap_or(0),
            "freeBytes":system["freeBytes"].as_u64(),"unmetered":system["unmetered"].as_bool().unwrap_or(false),
            "charging":system["charging"].as_bool().unwrap_or(false),"powerSave":system["powerSave"].as_bool().unwrap_or(false),
            "thermalStatus":system["thermalStatus"].as_u64().unwrap_or(0).min(6),"storageLow":system["storageLow"].as_bool().unwrap_or(false)}),
        )
    })();
    result
        .unwrap_or_else(|detail| json!({"ok":false,"detail":detail}))
        .to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_activity_rows_hide_metadata_and_keep_phase_actions() {
        let input = json!({"filename":"synthetic-private.mp4","phase":"ready","downloaded":12,"total":24,
            "error":"/private/path https://private.example token-secret","quality":"private-quality"});
        let hidden = row(&input, "download", "opaque", true);
        for marker in ["synthetic", "private", "secret", "quality", "https"] {
            assert!(!hidden.to_string().contains(marker));
        }
        assert_eq!(hidden["canPlay"], true);
        assert_eq!(hidden["category"], "complete");
        assert_eq!(hidden["done"], 12);
        assert_eq!(
            row(&json!({"phase":"failed"}), "transfer", "opaque", true)["category"],
            "issue"
        );
        assert_eq!(
            row(
                &json!({"phase":"unexpected-private"}),
                "transfer",
                "opaque",
                true
            )["phase"],
            "working"
        );
        assert!(snapshot("{\"filter\":\"unsupported\"}", true).contains("Invalid activity filter"));
        assert!(snapshot("{\"filename\":\"private\"}", true).contains("Invalid activity request"));
    }
}
