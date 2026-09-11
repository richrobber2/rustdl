//! Bounded in-memory playlist preparation state and progress responses.
use super::super::local;
use super::gallery::PlaylistMembership;
use super::models::DiscoveryCandidate;
use super::youtube::YouTubePlaylistEntry;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tiny_http::{Request, Response, StatusCode};

pub(in super::super) const WORKERS: usize = 3;
pub(in super::super) type Selection = (YouTubePlaylistEntry, PlaylistMembership);
pub(in super::super) type Job = Arc<Mutex<Resolution>>;
static JOBS: OnceLock<Mutex<HashMap<String, Job>>> = OnceLock::new();

pub(in super::super) struct Resolution {
    pub selections: Vec<Selection>,
    pub next: usize,
    pub completed: usize,
    pub candidates: Vec<Option<DiscoveryCandidate>>,
    pub failures: Vec<String>,
    pub cancelled: bool,
    pub workers: usize,
    pub quality_token: Option<String>,
    pub started: Instant,
}
impl Resolution {
    pub(in super::super) fn new(selections: Vec<Selection>) -> Self {
        let total = selections.len();
        Self {
            selections,
            next: 0,
            completed: 0,
            candidates: vec![None; total],
            failures: Vec::new(),
            cancelled: false,
            workers: WORKERS.min(total),
            quality_token: None,
            started: Instant::now(),
        }
    }
}
pub(in super::super) fn register(selections: Vec<Selection>) -> Result<(String, Job), String> {
    let mut jobs = JOBS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if jobs
        .values()
        .any(|job| job.lock().unwrap_or_else(|e| e.into_inner()).workers > 0)
    {
        return Err("Another playlist is still being prepared. Finish or cancel it before starting another selection.".to_owned());
    }
    while jobs.len() >= 8 {
        let oldest = jobs
            .iter()
            .min_by_key(|(_, job)| job.lock().unwrap_or_else(|e| e.into_inner()).started)
            .map(|(id, _)| id.clone())
            .unwrap();
        jobs.remove(&oldest);
    }
    let token = local::security::random_token();
    let job = Arc::new(Mutex::new(Resolution::new(selections)));
    jobs.insert(token.clone(), job.clone());
    Ok((token, job))
}
pub(in super::super) fn get(token: &str) -> Option<Job> {
    JOBS.get()?
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(token)
        .cloned()
}
pub(in super::super) fn snapshot(job: &Job) -> serde_json::Value {
    let job = job.lock().unwrap_or_else(|e| e.into_inner());
    serde_json::json!({
        "total": job.selections.len(), "completed": job.completed,
        "ready": job.completed.saturating_sub(job.failures.len()),
        "failures": job.failures, "finished": job.workers == 0,
        "cancelled": job.cancelled, "elapsedSeconds": job.started.elapsed().as_secs(),
        "canContinue": job.quality_token.is_some(),
    })
}
pub(in super::super) fn respond_status(
    request: Request,
    token: &str,
) -> Result<(), Box<dyn Error>> {
    let Some(job) = local::playlist_resolution::get(token) else {
        return local::html::respond_text(
            request,
            404,
            "Playlist preparation expired. Open the playlist again.",
        );
    };
    request.respond(
        Response::from_string(local::playlist_resolution::snapshot(&job).to_string())
            .with_header(local::html::header(
                "Content-Type",
                "application/json; charset=utf-8",
            ))
            .with_header(local::html::header("Cache-Control", "no-store")),
    )?;
    Ok(())
}
pub(in super::super) fn respond_page(request: Request, token: &str) -> Result<(), Box<dyn Error>> {
    if local::playlist_resolution::get(token).is_none() {
        return local::html::respond_text(
            request,
            404,
            "Playlist preparation expired. Open the playlist again.",
        );
    }
    local::html::respond_html(
        request,
        format!(
            include_str!("../../assets/html/playlist-resolution.html"),
            token = local::html::escape_html(token),
            css = local::discovery::DISCOVERY_CSS,
            script = include_str!("../../assets/js/playlist-resolution.js"),
        ),
    )
}
pub(in super::super) fn respond_cancel(
    request: Request,
    token: &str,
) -> Result<(), Box<dyn Error>> {
    let Some(job) = local::playlist_resolution::get(token) else {
        return local::html::respond_text(request, 404, "Playlist preparation expired.");
    };
    {
        let mut job = job.lock().unwrap_or_else(|e| e.into_inner());
        if job.workers > 0 {
            job.cancelled = true;
        }
    }
    request.respond(
        Response::empty(StatusCode(303)).with_header(local::html::header(
            "Location",
            &format!("/playlist/resolution?job={token}"),
        )),
    )?;
    Ok(())
}
pub(in super::super) fn respond_formats(
    request: Request,
    token: &str,
) -> Result<(), Box<dyn Error>> {
    let Some(job) = local::playlist_resolution::get(token) else {
        return local::html::respond_text(
            request,
            404,
            "Playlist preparation expired. Open the playlist again.",
        );
    };
    let job = job.lock().unwrap_or_else(|e| e.into_inner());
    let Some(quality_token) = job.quality_token.as_deref() else {
        return local::html::respond_text(
            request,
            409,
            "Formats are not ready. Return to playlist preparation.",
        );
    };
    let picks = (0..job.completed.saturating_sub(job.failures.len()))
        .map(|index| format!("{quality_token}:{index}"))
        .collect::<Vec<_>>();
    drop(job);
    local::discovery::respond_quality_page(request, &picks)
}
