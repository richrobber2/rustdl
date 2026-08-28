use super::*;

fn download_category(phase: DownloadPhase) -> &'static str {
    match phase {
        DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading => "active",
        DownloadPhase::Paused | DownloadPhase::Failed => "issue",
        DownloadPhase::Ready | DownloadPhase::Cancelled => "complete",
    }
}

fn category_rank(category: &str) -> u8 {
    match category {
        "active" => 0,
        "issue" => 1,
        _ => 2,
    }
}

pub(crate) fn snapshot() -> serde_json::Value {
    let tuning = runtime_tuning();
    let mut downloads = download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .filter(|(filename, _)| valid_video_filename(filename))
        .map(|(filename, job)| (filename.clone(), job.clone()))
        .collect::<Vec<_>>();
    downloads.sort_unstable_by(|left, right| {
        category_rank(download_category(left.1.phase))
            .cmp(&category_rank(download_category(right.1.phase)))
            .then_with(|| right.0.cmp(&left.0))
    });

    let mut transfers = PEER_SEND_JOBS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .filter(|(filename, _)| valid_video_filename(filename))
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
        category_rank(left_category)
            .cmp(&category_rank(right_category))
            .then_with(|| right.0.cmp(&left.0))
    });

    let download_counts = downloads.iter().fold([0_u64; 3], |mut counts, (_, job)| {
        match download_category(job.phase) {
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
            "phase": download_phase_name(job.phase),
            "downloaded": job.downloaded,
            "total": job.total,
            "quality": job.quality_label,
            "height": job.quality_height,
            "error": job.error,
        })).collect::<Vec<_>>(),
        "transfers": transfers.iter().take(30).map(|(filename, job)| serde_json::json!({
            "filename": filename,
            "phase": job.phase,
            "sent": job.sent,
            "total": job.total,
            "error": job.error,
        })).collect::<Vec<_>>(),
        "system": {
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
    let response = Response::from_string(snapshot().to_string())
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "application/json; charset=utf-8"))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

pub(crate) fn respond_page(request: Request) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(activity::render(&dev_reload_script()))
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(html_csp())
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}
