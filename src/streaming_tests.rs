use super::*;

#[test]
fn progressive_seek_uses_real_browser_buffer_ranges() {
    assert!(PLAYBACK_SCRIPT.contains("video.buffered.start(index)"));
    assert!(PLAYBACK_SCRIPT.contains("video.buffered.end(index)"));
    assert!(PLAYBACK_SCRIPT.contains("const seekSafely="));
    assert!(PLAYBACK_SCRIPT.contains("seekbackward:event=>seekBy"));
    assert!(PLAYBACK_SCRIPT.contains("seekto:event=>{if(finite(event.seekTime))seekSafely"));
    assert!(PLAYBACK_SCRIPT.contains("['progress','canplay','durationchange']"));
    assert!(!PLAYBACK_SCRIPT.contains("requested>downloadFraction"));
    assert!(PLAYBACK_SCRIPT.contains("const queuePreview="));
    assert!(PLAYBACK_SCRIPT.contains("previewVideo.fastSeek"));
    assert!(PLAYBACK_SCRIPT.contains("previewTimer=setTimeout"));
    assert!(PLAYBACK_SCRIPT.contains("requested<=video.currentTime||isBuffered(requested)"));
    assert!(PLAYBACK_SCRIPT.contains("<video muted playsinline preload=\"metadata\""));
}

#[test]
fn growing_streams_wake_on_download_progress_without_busy_polling() {
    let source = include_str!("main.rs");
    assert!(source.contains("DOWNLOAD_PROGRESS_SIGNAL"));
    assert!(source.contains("wait_timeout_while"));
    assert!(source.contains("signal_growing_media();"));
    assert!(!source.contains("thread::sleep(Duration::from_millis(75))"));
}

#[test]
fn player_marks_complete_and_growing_media_explicitly() {
    let source = include_str!("main.rs");
    assert!(source.contains(r#"data-growing="{growing}""#));
    assert!(PLAYBACK_SCRIPT.contains("video.dataset.growing==='true'"));
    assert!(PLAYBACK_SCRIPT.contains("if(!growing||"));
}
