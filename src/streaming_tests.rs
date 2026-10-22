// These checks inspect source structure; UI tests separately execute generated scripts.
const SOURCE_PLAYBACK_SCRIPT: &str = include_str!("../assets/js/playback.js");

#[test]
fn progressive_seek_uses_real_browser_buffer_ranges() {
    assert!(SOURCE_PLAYBACK_SCRIPT.contains("video.buffered.start(index)"));
    assert!(SOURCE_PLAYBACK_SCRIPT.contains("video.buffered.end(index)"));
    assert!(SOURCE_PLAYBACK_SCRIPT.contains("const seekSafely="));
    assert!(SOURCE_PLAYBACK_SCRIPT.contains("seekbackward:event=>seekBy"));
    assert!(SOURCE_PLAYBACK_SCRIPT.contains("seekto:event=>{if(finite(event.seekTime))seekSafely"));
    assert!(SOURCE_PLAYBACK_SCRIPT.contains("['progress','canplay','durationchange']"));
    assert!(!SOURCE_PLAYBACK_SCRIPT.contains("requested>downloadFraction"));
    assert!(SOURCE_PLAYBACK_SCRIPT.contains("const queuePreview="));
    assert!(SOURCE_PLAYBACK_SCRIPT.contains("previewVideo.fastSeek"));
    assert!(SOURCE_PLAYBACK_SCRIPT.contains("previewTimer=setTimeout"));
    assert!(SOURCE_PLAYBACK_SCRIPT.contains("requested<=video.currentTime||isBuffered(requested)"));
    assert!(SOURCE_PLAYBACK_SCRIPT.contains("<video muted playsinline preload=\"metadata\""));
}

#[test]
fn growing_streams_wake_on_download_progress_without_busy_polling() {
    let source = concat!(
        include_str!("local/media.rs"),
        include_str!("local/queue.rs")
    );
    assert!(source.contains("DOWNLOAD_PROGRESS_SIGNAL"));
    assert!(source.contains("wait_timeout_while"));
    assert!(source.contains("signal_growing_media();"));
    assert!(!source.contains("thread::sleep(Duration::from_millis(75))"));
}

#[test]
fn player_marks_complete_and_growing_media_explicitly() {
    let source = include_str!("../assets/html/download-result.html");
    assert!(source.contains(r#"data-growing="{growing}""#));
    assert!(SOURCE_PLAYBACK_SCRIPT.contains("video.dataset.growing==='true'"));
    assert!(SOURCE_PLAYBACK_SCRIPT.contains("if(!growing||"));
}
