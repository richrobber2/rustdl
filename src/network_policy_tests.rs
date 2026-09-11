use super::{external, local};
use local::queue::{DownloadJob, DownloadPhase};
use std::{fs, sync::mpsc, thread, time::Duration};

#[test]
#[ignore = "Changes the process-wide network policy; run this test separately"]
fn native_network_policy_waits_preserves_partials_and_resumes_with_range() {
    let name = "youtube-networktest.mp4";
    let directory =
        std::env::temp_dir().join(format!("rustdl-network-policy-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    struct Cleanup(String, std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            local::queue::download_jobs()
                .lock()
                .unwrap()
                .remove(&self.0);
            local::runtime::set_download_network_state(0);
            let _ = fs::remove_dir_all(&self.1);
        }
    }
    let _cleanup = Cleanup(name.to_owned(), directory.clone());
    let job = DownloadJob {
        phase: DownloadPhase::Queued,
        downloaded: 16,
        total: Some(128),
        error: None,
        source_url: None,
        media_url: None,
        audio_url: None,
        extract_audio: false,
        quality_label: None,
        quality_height: None,
    };
    local::queue::set_download_job(name, job.clone());
    local::runtime::set_download_network_state(3);
    assert_eq!(
        local::queue::app_state_job(name, &job)["phaseLabel"],
        "Waiting for mobile-data approval"
    );
    let (tx, rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        tx.send(local::queue::acquire_download_slot(name)).unwrap();
    });
    assert!(rx.recv_timeout(Duration::from_millis(150)).is_err());
    local::queue::download_jobs()
        .lock()
        .unwrap()
        .get_mut(name)
        .unwrap()
        .phase = DownloadPhase::Paused;
    local::queue::download_gate().1.notify_all();
    assert!(!rx.recv_timeout(Duration::from_secs(2)).unwrap());
    worker.join().unwrap();
    assert!(!local::queue::defer_for_network(name));
    assert_eq!(
        local::queue::download_job(name).unwrap().phase,
        DownloadPhase::Paused
    );

    local::queue::set_download_job(name, job);
    local::runtime::set_download_network_state(0);
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}/media", server.server_addr());
    let worker = thread::spawn(move || {
        for attempt in 0..2 {
            let request = server
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap();
            assert!(
                request
                    .headers()
                    .iter()
                    .any(|h| h.field.equiv("Range") && h.value.as_str() == "bytes=16-")
            );
            if attempt == 0 {
                local::runtime::set_download_network_state(2);
            }
            let response = tiny_http::Response::from_data((16..128).collect::<Vec<u8>>())
                .with_status_code(206)
                .with_header(
                    tiny_http::Header::from_bytes("Content-Range", "bytes 16-127/128").unwrap(),
                );
            let _ = request.respond(response);
        }
    });
    let path = directory.join("track.part");
    fs::write(&path, (0..16).collect::<Vec<u8>>()).unwrap();
    let client = external::http::build_client().unwrap();
    assert_eq!(
        external::http::download_adaptive_track(&client, &url, &path, name, 0).unwrap(),
        None
    );
    assert_eq!(fs::metadata(&path).unwrap().len(), 16);
    assert_eq!(
        local::queue::download_job(name).unwrap().phase,
        DownloadPhase::Queued
    );
    local::runtime::set_download_network_state(0);
    assert_eq!(
        external::http::download_adaptive_track(&client, &url, &path, name, 0).unwrap(),
        Some(128)
    );
    assert_eq!(fs::read(&path).unwrap(), (0..128).collect::<Vec<u8>>());
    worker.join().unwrap();
}
