//! Local Activity snapshots and page responses.

use super::super::local;
use super::super::local::peers::PEER_SEND_JOBS;
use super::super::local::queue::DownloadPhase;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Mutex;
use tiny_http::{Request, Response, StatusCode};

pub(in super::super) fn download_category(phase: DownloadPhase) -> &'static str {
    match phase {
        DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading => "active",
        DownloadPhase::Paused | DownloadPhase::Failed => "issue",
        DownloadPhase::Ready | DownloadPhase::Cancelled => "complete",
    }
}

pub(in super::super) fn category_rank(category: &str) -> u8 {
    match category {
        "active" => 0,
        "issue" => 1,
        _ => 2,
    }
}

pub(crate) fn snapshot() -> serde_json::Value {
    let tuning = local::runtime::runtime_tuning();
    let mut downloads = local::queue::download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .filter(|(filename, _)| local::media::valid_video_filename(filename))
        .map(|(filename, job)| (filename.clone(), job.clone()))
        .collect::<Vec<_>>();
    downloads.sort_unstable_by(|left, right| {
        local::activity_state::category_rank(local::activity_state::download_category(left.1.phase))
            .cmp(&local::activity_state::category_rank(
                local::activity_state::download_category(right.1.phase),
            ))
            .then_with(|| right.0.cmp(&left.0))
    });

    let mut transfers = PEER_SEND_JOBS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .filter(|(filename, _)| local::media::valid_video_filename(filename))
        .map(|(filename, job)| (filename.clone(), job.clone()))
        .collect::<Vec<_>>();
    transfers.sort_unstable_by(|left, right| {
        let left_category = if left.1.phase == "failed" {
            "issue"
        } else if left.1.phase == "ready" {
            "complete"
        } else {
            "active"
        };
        let right_category = if right.1.phase == "failed" {
            "issue"
        } else if right.1.phase == "ready" {
            "complete"
        } else {
            "active"
        };
        local::activity_state::category_rank(left_category)
            .cmp(&local::activity_state::category_rank(right_category))
            .then_with(|| right.0.cmp(&left.0))
    });

    let download_counts = downloads.iter().fold([0_u64; 3], |mut counts, (_, job)| {
        match local::activity_state::download_category(job.phase) {
            "active" => counts[0] += 1,
            "issue" => counts[1] += 1,
            _ => counts[2] += 1,
        }
        counts
    });
    let transfer_counts = transfers.iter().fold([0_u64; 3], |mut counts, (_, job)| {
        if job.phase == "failed" {
            counts[1] += 1;
        } else if job.phase == "ready" {
            counts[2] += 1;
        } else {
            counts[0] += 1;
        }
        counts
    });
    let free_bytes = if tuning.free_bytes == u64::MAX {
        serde_json::Value::Null
    } else {
        serde_json::json!(tuning.free_bytes)
    };

    serde_json::json!({
        "counts": {
            "active": download_counts[0] + transfer_counts[0],
            "issues": download_counts[1] + transfer_counts[1],
            "completed": download_counts[2] + transfer_counts[2],
        },
        "downloads": downloads.iter().take(60).map(|(filename, job)| serde_json::json!({
            "filename": filename,
            "phase": local::queue::download_phase_name(job.phase),
            "phaseLabel": match job.phase {
                DownloadPhase::Queued => "Queued", DownloadPhase::Starting => "Starting",
                DownloadPhase::Downloading => "Downloading", DownloadPhase::Paused => "Paused",
                DownloadPhase::Ready => "Completed", DownloadPhase::Failed => "Failed",
                DownloadPhase::Cancelled => "Cancelled",
            },
            "downloaded": job.downloaded,
            "sizeLabel": match job.total {
                Some(total) => format!("{} / {}", local::format::format_bytes(job.downloaded), local::format::format_bytes(total)),
                None => local::format::format_bytes(job.downloaded),
            },
            "total": job.total,
            "quality": job.quality_label,
            "height": job.quality_height,
            "error": job.error,
        })).collect::<Vec<_>>(),
        "transfers": transfers.iter().take(30).map(|(filename, job)| serde_json::json!({
            "filename": filename,
            "phase": job.phase,
            "phaseLabel": match job.phase.as_str() {
                "ready" => "Completed", "failed" => "Failed", "sending" => "Sending",
                "queued" => "Queued", _ => "Transferring",
            },
            "sent": job.sent,
            "sizeLabel": if job.total > 0 {
                format!("{} / {}", local::format::format_bytes(job.sent), local::format::format_bytes(job.total))
            } else {
                local::format::format_bytes(job.sent)
            },
            "total": job.total,
            "error": job.error,
        })).collect::<Vec<_>>(),
        "system": {
            "freeBytesLabel": if tuning.free_bytes == u64::MAX { "Unavailable".to_owned() } else { local::format::format_bytes(tuning.free_bytes) },
            "unmetered": tuning.unmetered,
            "charging": tuning.charging,
            "powerSave": tuning.power_save,
            "thermalStatus": tuning.thermal_status,
            "freeBytes": free_bytes,
            "storageLow": tuning.free_bytes != u64::MAX
                && tuning.free_bytes < 1024 * 1024 * 1024,
        },
    })
}

pub(crate) fn respond_state(request: Request) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(local::activity_state::snapshot().to_string())
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

pub(crate) fn respond_page(request: Request) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(local::web_assets::decorate_app_html(
        local::activity::render(&local::dev::dev_reload_script()),
    ))
    .with_status_code(StatusCode(200))
    .with_header(local::html::header(
        "Content-Type",
        "text/html; charset=utf-8",
    ))
    .with_header(local::html::html_csp())
    .with_header(local::html::header("Cache-Control", "no-store"))
    .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}
