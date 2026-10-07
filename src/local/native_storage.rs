//! Storage controls invoked by app UI only. Never call snapshots from agent tools.
use super::storage::{PartialFile, StoredVideo};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::Mutex;
#[derive(Default)]
struct Handles {
    next: u64,
    items: VecDeque<(String, String)>,
}
static HANDLES: Mutex<Handles> = Mutex::new(Handles {
    next: 0,
    items: VecDeque::new(),
});
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Request {
    offset: usize,
    id: String,
    action: String,
}
fn video_row(video: &StoredVideo, id: &str, privacy: bool) -> Value {
    json!({"id":id,"title":if privacy {"Downloaded media"}else{video.filename.as_str()},
        "bytes":video.bytes,"watched":video.watched,"duplicate":video.duplicate,"kind":"video"})
}
fn partial_row(partial: &PartialFile, privacy: bool) -> Value {
    json!({"id":"","title":if privacy {"Partial download"}else{partial.filename.as_str()},
        "bytes":partial.bytes,"stale":partial.stale,"kind":"partial"})
}
fn snapshot(offset: usize, privacy: bool) -> Result<Value, &'static str> {
    let output = super::queue::QUEUE_OUTPUT_DIR
        .get()
        .ok_or("Download engine is starting")?;
    let snapshot =
        super::storage::storage_snapshot(output).map_err(|_| "Could not inspect storage")?;
    let total = snapshot.videos.len() + snapshot.partials.len();
    let offset = if total == 0 {
        0
    } else {
        offset.min((total - 1) / 25 * 25)
    };
    let mut registry = HANDLES.lock().unwrap_or_else(|p| p.into_inner());
    let items: Vec<_> = snapshot
        .videos
        .iter()
        .map(|video| {
            registry.next = registry.next.wrapping_add(1);
            let id = format!("storage-{:x}", registry.next);
            registry
                .items
                .push_back((id.clone(), video.filename.clone()));
            while registry.items.len() > 4096 {
                registry.items.pop_front();
            }
            video_row(video, &id, privacy)
        })
        .chain(
            snapshot
                .partials
                .iter()
                .map(|partial| partial_row(partial, privacy)),
        )
        .skip(offset)
        .take(25)
        .collect();
    Ok(
        json!({"ok":true,"offset":offset,"totalItems":total,"items":items,"videoBytes":snapshot.video_bytes,
        "partialBytes":snapshot.partial_bytes,"thumbnailBytes":snapshot.thumbnail_bytes,"metadataBytes":snapshot.metadata_bytes,
        "watchedCount":snapshot.videos.iter().filter(|v|v.watched).count(),"duplicateCount":snapshot.videos.iter().filter(|v|v.duplicate).count(),
        "staleCount":snapshot.partials.iter().filter(|p|p.stale).count()}),
    )
}
pub(crate) fn command(command: &str, payload: &str, privacy: bool) -> String {
    let result = (|| -> Result<Value, &'static str> {
        if payload.len() > 4096 {
            return Err("Storage request is too large");
        }
        let request: Request =
            serde_json::from_str(payload).map_err(|_| "Invalid storage request")?;
        if request.offset > 100000 || request.id.len() > 64 {
            return Err("Invalid storage page");
        }
        match command {
            "snapshot" => snapshot(request.offset, privacy),
            "cleanup" => {
                if !matches!(
                    request.action.as_str(),
                    "delete" | "watched" | "stale" | "thumbnails"
                ) {
                    return Err("Unsupported storage action");
                }
                let filename = if request.action == "delete" {
                    Some(
                        HANDLES
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .items
                            .iter()
                            .find(|(id, _)| id == &request.id)
                            .map(|(_, name)| name.clone())
                            .ok_or("This item selection expired. Refresh storage.")?,
                    )
                } else {
                    None
                };
                let output = super::queue::QUEUE_OUTPUT_DIR
                    .get()
                    .ok_or("Download engine is starting")?;
                super::storage::cleanup_storage(output, &request.action, filename.as_deref())
                    .map_err(|_| "Cleanup failed. Refresh storage and try again.")?;
                let mut data = snapshot(request.offset, privacy)?;
                data["detail"] = json!("Storage cleanup completed.");
                Ok(data)
            }
            _ => Err("Unsupported storage command"),
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
    fn native_storage_rows_hide_names_paths_and_reject_untrusted_actions() {
        let video = StoredVideo {
            filename: "synthetic-private.mp4".into(),
            bytes: 123,
            watched: true,
            duplicate: false,
        };
        let partial = PartialFile {
            filename: "synthetic-private.mp4".into(),
            path: "/synthetic/private/part".into(),
            bytes: 12,
            stale: true,
        };
        let rows = json!([
            video_row(&video, "opaque-item", true),
            partial_row(&partial, true)
        ]);
        assert!(!rows.to_string().contains("synthetic"));
        assert!(!rows.to_string().contains("private"));
        assert_eq!(rows[0]["bytes"], 123);
        assert_eq!(rows[1]["stale"], true);
        assert_eq!(
            video_row(&video, "opaque-item", false)["title"],
            video.filename
        );
        assert!(
            command("cleanup", "{\"action\":\"unsupported\"}", true)
                .contains("Unsupported storage action")
        );
        assert!(
            command("snapshot", "{\"path\":\"/private\"}", true)
                .contains("Invalid storage request")
        );
    }
}
