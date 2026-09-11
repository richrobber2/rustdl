//! Local gallery and playback state responses, including inspection fixtures.

use super::super::local;
use super::super::local::queue::DownloadPhase;
use std::error::Error;
use std::fs;
use std::path::Path;
use tiny_http::{Request, Response, StatusCode};

pub(in super::super) fn respond_app_state(
    request: Request,
    output_dir: &Path,
    requested_filename: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let value = if local::runtime::inspection_mode() {
        serde_json::json!({
            "inspection": true,
            "active": 1,
            "activityActive": 1,
            "activityIssues": 0,
            "downloaded": 27_262_976_u64,
            "total": 67_108_864_u64,
            "jobs": [{
                "filename": "synthetic-preview.mp4",
                "phase": "downloading",
                "downloaded": 27_262_976_u64,
                "total": 67_108_864_u64,
                "quality": "Synthetic 1080p",
                "height": 1080,
                "source": null,
            }],
            "current": requested_filename.map(|_| serde_json::json!({
                "filename": "synthetic-preview.mp4",
                "phase": "downloading",
                "downloaded": 27_262_976_u64,
                "total": 67_108_864_u64,
                "quality": "Synthetic 1080p",
                "height": 1080,
                "source": null,
            })),
        })
    } else {
        let mut jobs = local::queue::download_jobs()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .filter(|(filename, _)| local::media::valid_video_filename(filename))
            .map(|(filename, job)| (filename.clone(), job.clone()))
            .collect::<Vec<_>>();
        jobs.sort_unstable_by(|left, right| right.0.cmp(&left.0));
        let active = jobs
            .iter()
            .filter(|(_, job)| {
                matches!(
                    job.phase,
                    DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading
                )
            })
            .count();
        let downloaded = jobs.iter().fold(0_u64, |total, (_, job)| {
            total.saturating_add(job.downloaded)
        });
        let total = jobs.iter().fold(0_u64, |sum, (_, job)| {
            sum.saturating_add(job.total.unwrap_or(0))
        });
        let current = requested_filename
            .filter(|filename| local::media::valid_video_filename(filename))
            .and_then(|filename| {
                jobs.iter()
                    .find(|(candidate, _)| candidate == filename)
                    .map(|(_, job)| local::queue::app_state_job(filename, job))
                    .or_else(|| {
                        fs::metadata(output_dir.join(filename))
                            .ok()
                            .map(|metadata| {
                                serde_json::json!({
                                    "filename": filename,
                                    "phase": "ready",
                                    "downloaded": metadata.len(),
                                    "total": metadata.len(),
                                    "quality": null,
                                    "height": null,
                                    "source": null,
                                })
                            })
                    })
            });
        let activity = local::activity_state::snapshot();
        serde_json::json!({
            "inspection": false,
            "active": active,
            "activityActive": activity["counts"]["active"].clone(),
            "activityIssues": activity["counts"]["issues"].clone(),
            "downloaded": downloaded,
            "total": total,
            "jobs": jobs
                .iter()
                .map(|(filename, job)| local::queue::app_state_job(filename, job))
                .collect::<Vec<_>>(),
            "current": current,
        })
    };
    let response = Response::from_string(serde_json::to_string(&value)?)
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
