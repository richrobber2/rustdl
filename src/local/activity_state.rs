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

// Count the full queue, but copy only the rows that can reach the response.
// Select/sort borrowed entries while locked; JSON construction uses owned rows
// after the guard is released, avoiding resolver-data copies for omitted jobs.
fn bounded_jobs<T: Clone>(
    jobs: &HashMap<String, T>,
    limit: usize,
    category: impl Fn(&T) -> &'static str,
) -> (Vec<(String, T)>, [u64; 3]) {
    let mut counts = [0_u64; 3];
    let mut ordered = Vec::with_capacity(jobs.len());
    for (filename, job) in jobs {
        if !local::media::valid_video_filename(filename) {
            continue;
        }
        counts[category_rank(category(job)) as usize] += 1;
        ordered.push((filename, job));
    }
    let compare = |left: &(&String, &T), right: &(&String, &T)| {
        category_rank(category(left.1))
            .cmp(&category_rank(category(right.1)))
            .then_with(|| right.0.cmp(left.0))
    };
    if ordered.len() > limit {
        ordered.select_nth_unstable_by(limit, &compare);
        ordered.truncate(limit);
    }
    ordered.sort_unstable_by(compare);
    let visible = ordered
        .into_iter()
        .map(|(filename, job)| (filename.clone(), job.clone()))
        .collect();
    (visible, counts)
}

pub(crate) fn snapshot() -> serde_json::Value {
    let tuning = local::runtime::runtime_tuning();
    let (downloads, download_counts) = bounded_jobs(
        &local::queue::download_jobs()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        60,
        |job| download_category(job.phase),
    );
    let (transfers, transfer_counts) = bounded_jobs(
        &PEER_SEND_JOBS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        30,
        |job| match job.phase.as_str() {
            "failed" => "issue",
            "ready" => "complete",
            _ => "active",
        },
    );
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TrackedJob {
        category: &'static str,
        clones: Arc<AtomicUsize>,
    }

    impl Clone for TrackedJob {
        fn clone(&self) -> Self {
            self.clones.fetch_add(1, Ordering::Relaxed);
            Self {
                category: self.category,
                clones: Arc::clone(&self.clones),
            }
        }
    }

    #[test]
    fn bounded_snapshot_preserves_full_counts_order_and_only_clones_visible_jobs() {
        let clones = Arc::new(AtomicUsize::new(0));
        let mut jobs = HashMap::new();
        for i in 0..503 {
            jobs.insert(
                format!("{i:05}-1.mp4"),
                TrackedJob {
                    category: ["active", "issue", "complete"][i % 3],
                    clones: Arc::clone(&clones),
                },
            );
        }
        for filename in ["notes.txt", "../invalid.mp4"] {
            jobs.insert(
                filename.to_owned(),
                TrackedJob {
                    category: "active",
                    clones: Arc::clone(&clones),
                },
            );
        }
        let expected: Vec<_> = (0..3)
            .flat_map(|category| {
                (0..503)
                    .rev()
                    .filter(move |i| i % 3 == category)
                    .map(|i| format!("{i:05}-1.mp4"))
            })
            .collect();
        for limit in [0, 1, 29, 30, 59, 60, 503, 600] {
            clones.store(0, Ordering::Relaxed);
            let (rows, counts) = bounded_jobs(&jobs, limit, |job| job.category);
            assert_eq!(counts, [168, 168, 167]);
            assert_eq!(rows.len(), limit.min(503));
            assert_eq!(clones.load(Ordering::Relaxed), rows.len());
            assert_eq!(
                rows.iter()
                    .map(|(filename, _)| filename)
                    .collect::<Vec<_>>(),
                expected.iter().take(limit).collect::<Vec<_>>()
            );
        }
        let (rows, counts) = bounded_jobs(&HashMap::<String, TrackedJob>::new(), 60, |job| {
            job.category
        });
        assert!(rows.is_empty());
        assert_eq!(counts, [0; 3]);
    }
}
