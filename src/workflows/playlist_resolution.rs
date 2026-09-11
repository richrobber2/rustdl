//! Concurrent metadata-only preparation; no media transfers begin here.
use super::super::{external, local, workflows};
use local::models::DiscoveryCandidate;
use local::playlist_resolution::{Job, Selection};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::thread;

pub(in super::super) fn start(selections: Vec<Selection>) -> Result<String, String> {
    let (token, job) = local::playlist_resolution::register(selections)?;
    let workers = job.lock().unwrap_or_else(|e| e.into_inner()).workers;
    for _ in 0..workers {
        let job = job.clone();
        let fallback = job.clone();
        if let Err(error) = thread::Builder::new()
            .name("playlist-formats".to_owned())
            .spawn(move || match external::youtube::Resolver::new() {
                Ok(resolver) => workflows::playlist_resolution::run_worker(&job, |url| {
                    resolver.resolve_candidate(url).map_err(|e| e.to_string())
                }),
                Err(error) => {
                    let message = error.to_string();
                    workflows::playlist_resolution::run_worker(&job, |_| Err(message.clone()));
                }
            })
        {
            fallback.lock().unwrap_or_else(|e| e.into_inner()).cancelled = true;
            workflows::playlist_resolution::worker_finished(&fallback);
            eprintln!("Could not start playlist worker: {error}");
        }
    }
    Ok(token)
}
fn run_worker(job: &Job, mut resolve: impl FnMut(&str) -> Result<DiscoveryCandidate, String>) {
    loop {
        let selection = {
            let mut job = job.lock().unwrap_or_else(|e| e.into_inner());
            if job.cancelled || job.next >= job.selections.len() {
                break;
            }
            let index = job.next;
            job.next += 1;
            (index, job.selections[index].clone())
        };
        let (index, (entry, membership)) = selection;
        let url = format!("https://www.youtube.com/watch?v={}", entry.video_id);
        let result = catch_unwind(AssertUnwindSafe(|| resolve(&url)))
            .unwrap_or_else(|_| Err("The extractor stopped unexpectedly".to_owned()));
        let mut job = job.lock().unwrap_or_else(|e| e.into_inner());
        if job.cancelled {
            break;
        }
        job.completed += 1;
        match result {
            Ok(mut candidate) => {
                candidate.playlist = Some(membership);
                job.candidates[index] = Some(candidate);
            }
            Err(error) => job.failures.push(format!(
                "{}: {}",
                entry.title,
                local::format::truncate_text(&error, 240)
            )),
        }
    }
    workflows::playlist_resolution::worker_finished(job);
}
fn worker_finished(job: &Job) {
    let mut job = job.lock().unwrap_or_else(|e| e.into_inner());
    job.workers = job.workers.saturating_sub(1);
    if job.workers == 0 && !job.cancelled {
        let candidates = job
            .candidates
            .iter_mut()
            .filter_map(Option::take)
            .collect::<Vec<_>>();
        if !candidates.is_empty() {
            job.quality_token = Some(local::discovery::store_discovery_session(candidates));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use local::gallery::PlaylistMembership;
    use local::models::ResolvedVideo;
    use local::playlist_resolution::{Resolution, WORKERS};
    use local::youtube::YouTubePlaylistEntry;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier, Mutex};

    fn job(count: usize) -> Job {
        Arc::new(Mutex::new(Resolution::new(
            (0..count)
                .map(|index| {
                    (
                        YouTubePlaylistEntry {
                            video_id: format!("worker{index:05}"),
                            title: format!("Item {index}"),
                            author: "Test".to_owned(),
                        },
                        PlaylistMembership {
                            playlist_id: "PL1234567890_test".to_owned(),
                            title: "Test".to_owned(),
                            position: index + 1,
                            total: count,
                        },
                    )
                })
                .collect(),
        )))
    }
    fn candidate(url: &str) -> DiscoveryCandidate {
        let resolved = ResolvedVideo {
            filename: format!("youtube-{}.mp4", local::youtube::video_id(url).unwrap()),
            media_url: "https://example.invalid/private-stream".to_owned(),
            audio_url: None,
            extract_audio: false,
            quality_label: None,
            quality_height: None,
        };
        DiscoveryCandidate {
            qualities: vec![resolved.clone()],
            resolved,
            source_url: url.to_owned(),
            author: "Test".to_owned(),
            text: "Test".to_owned(),
            playlist: None,
        }
    }
    #[test]
    fn five_hundred_resolutions_are_bounded_observable_and_keep_playlist_order() {
        let job = job(500);
        let entered = Barrier::new(WORKERS + 1);
        let release = Barrier::new(WORKERS + 1);
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        thread::scope(|scope| {
            for _ in 0..WORKERS {
                scope.spawn(|| {
                    let mut first = true;
                    workflows::playlist_resolution::run_worker(&job, |url| {
                        let running = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(running, Ordering::SeqCst);
                        if first {
                            first = false;
                            entered.wait();
                            release.wait();
                        }
                        thread::yield_now();
                        active.fetch_sub(1, Ordering::SeqCst);
                        if url.ends_with("worker00001") {
                            Err("Unavailable <video>".to_owned())
                        } else {
                            Ok(candidate(url))
                        }
                    });
                });
            }
            entered.wait();
            let progress = local::playlist_resolution::snapshot(&job);
            assert_eq!(progress["completed"], 0);
            assert_eq!(progress["finished"], false);
            assert_eq!(job.lock().unwrap().next, WORKERS);
            release.wait();
        });
        assert_eq!(peak.load(Ordering::SeqCst), WORKERS);
        let progress = local::playlist_resolution::snapshot(&job);
        assert_eq!(progress["completed"], 500);
        assert_eq!(progress["ready"], 499);
        assert_eq!(progress["finished"], true);
        assert_eq!(progress["failures"][0], "Item 1: Unavailable <video>");
        assert!(!progress.to_string().contains("private-stream"));
        let token = job.lock().unwrap().quality_token.clone().unwrap();
        let mut sessions = local::discovery::DISCOVERY_SESSIONS
            .get()
            .unwrap()
            .lock()
            .unwrap();
        let candidates = &sessions.get(&token).unwrap().candidates;
        let positions = candidates
            .iter()
            .map(|c| c.playlist.as_ref().unwrap().position)
            .collect::<Vec<_>>();
        assert_eq!(
            positions,
            (1..=500)
                .filter(|position| *position != 2)
                .collect::<Vec<_>>()
        );
        sessions.remove(&token);
    }
    #[test]
    fn cancellation_stops_new_requests_and_does_not_publish_partial_formats() {
        let job = job(500);
        let entered = Barrier::new(WORKERS + 1);
        let release = Barrier::new(WORKERS + 1);
        thread::scope(|scope| {
            for _ in 0..WORKERS {
                scope.spawn(|| {
                    workflows::playlist_resolution::run_worker(&job, |url| {
                        entered.wait();
                        release.wait();
                        Ok(candidate(url))
                    })
                });
            }
            entered.wait();
            job.lock().unwrap().cancelled = true;
            release.wait();
        });
        let state = job.lock().unwrap();
        assert_eq!(state.next, WORKERS);
        assert_eq!(state.workers, 0);
        assert!(state.quality_token.is_none());
        assert_eq!(state.completed, 0);
    }
}
