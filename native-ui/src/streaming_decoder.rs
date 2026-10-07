//! Isolated streaming decoder controls; no decoder addresses enter this model.
use super::*;
use serde::Deserialize;
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Page {
    pub privacy: bool,
    pub title: String,
    pub ready: bool,
    pub media_ready: bool,
    pub playing: bool,
    pub playback_state: String,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub generation: i32,
    pub progress: u8,
    pub state: String,
    pub can_download: bool,
    pub watchlisted: bool,
    pub has_watchlist: bool,
    pub filter: String,
    pub server_offset: usize,
    pub server_total: usize,
    pub episode_offset: usize,
    pub episode_total: usize,
    pub servers: Vec<Row>,
    pub episodes: Vec<Row>,
}
impl Default for Page {
    fn default() -> Self {
        Self {
            privacy: true,
            title: String::new(),
            ready: false,
            media_ready: false,
            playing: false,
            playback_state: String::new(),
            position_seconds: 0.,
            duration_seconds: 0.,
            generation: 0,
            progress: 0,
            state: String::new(),
            can_download: false,
            watchlisted: false,
            has_watchlist: false,
            filter: String::new(),
            server_offset: 0,
            server_total: 0,
            episode_offset: 0,
            episode_total: 0,
            servers: vec![],
            episodes: vec![],
        }
    }
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Row {
    pub index: usize,
    pub label: String,
    pub selected: bool,
}
fn request(action: &str, index: usize, generation: i32) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let action = env.new_string(action).map_err(|e| e.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("requestNativeDecoder"),
            jni::jni_sig!("(Ljava/lang/String;II)V"),
            &[
                JValue::Object(&action),
                JValue::Int(index as i32),
                JValue::Int(generation),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}
impl Page {
    pub(super) fn view(&self, cx: &mut Context<Home>, scroll: &ScrollHandle) -> impl IntoElement {
        let generation = self.generation;
        let mut content = semantic_scroll("native-stream-decoder", scroll)
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .child(div().text_xl().child(semantic_text(
                "streaming_decoder-text-1",
                if self.privacy {
                    "Anime playback".to_owned()
                } else {
                    self.title.clone()
                },
            )));
        content = content.child(div().child(semantic_text(
            "streaming_decoder-text-2",
            match self.state.as_str() {
                "ready" => "Native anime player ready",
                "error" => "Could not open streaming playback. Refresh servers to retry.",
                _ => "Checking streaming playback…",
            },
        )));
        if self.privacy {
            content = content.child(div().child(semantic_text("streaming-private-video", "Inspection privacy hides video frames. Change it in Settings when you want to watch.")));
        }
        content = content.child(div().child(semantic_text(
            "anime-owned-player-state",
            match self.playback_state.as_str() {
                "playing" => "Playing anime in RustDL",
                "paused" => "Anime paused",
                "completed" => "Episode completed",
                "preparing" => "Preparing anime stream…",
                "error" => "Native stream could not play. Try another server.",
                "focus-unavailable" => "Audio focus unavailable. Try Play again.",
                _ => "Resolving anime media…",
            },
        )));
        if self.media_ready {
            if !self.privacy {
                content = content.child(
                    Button::new("anime-owned-fullscreen")
                        .label("Fullscreen")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.navigation_error = request("fullscreen", 0, generation).is_err();
                            cx.notify();
                        })),
                );
            }
            for (id, label, action) in [
                (
                    "anime-owned-toggle",
                    if self.playing { "Pause" } else { "Play" },
                    if self.playing { "pause" } else { "play" },
                ),
                ("anime-owned-back", "Back 10 seconds", "seek-back"),
                ("anime-owned-forward", "Forward 10 seconds", "seek-forward"),
            ] {
                content = content.child(Button::new(id).label(label).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.navigation_error = request(action, 0, generation).is_err();
                        cx.notify();
                    },
                )));
            }
        }
        if self.state == "loading" {
            content = content.child(div().child(semantic_text(
                "streaming_decoder-text-3",
                format!("Loading · {}%", self.progress.min(100)),
            )));
        }
        for (id, label, action) in [
            ("decoder-close", "Back to RustDL", "close"),
            ("decoder-refresh", "Refresh servers", "refresh"),
        ] {
            content = content.child(Button::new(id).label(label).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.navigation_error = request(action, 0, generation).is_err();
                    cx.notify();
                },
            )));
        }
        if self.has_watchlist {
            content = content.child(
                Button::new("decoder-watchlist")
                    .label(if self.watchlisted {
                        "Remove from watchlist"
                    } else {
                        "Add to watchlist"
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.navigation_error = request("watchlist", 0, generation).is_err();
                        cx.notify();
                    })),
            );
        }
        for (index, label) in ["All", "Sub", "Dub", "Ready", "Issues"].iter().enumerate() {
            let selected = self.filter == ["all", "sub", "dub", "ready", "issues"][index];
            content = content.child(
                Button::new(SharedString::from(format!("decoder-filter-{index}")))
                    .label(if selected {
                        format!("✓ {label}")
                    } else {
                        (*label).to_owned()
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.navigation_error = request("filter", index, generation).is_err();
                        cx.notify();
                    })),
            );
        }
        if self.can_download {
            content = content.child(
                Button::new("decoder-download")
                    .label("Download episode")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.navigation_error = request("download", 0, generation).is_err();
                        cx.notify();
                    })),
            );
        }
        for (action, rows) in [("server", &self.servers), ("episode", &self.episodes)] {
            for row in rows {
                let index = row.index;
                let label = if self.privacy {
                    format!(
                        "{} {}{}",
                        if action == "server" {
                            "Server"
                        } else {
                            "Episode"
                        },
                        index + 1,
                        if row.selected { " · selected" } else { "" }
                    )
                } else {
                    row.label.clone()
                };
                content = content.child(
                    Button::new(SharedString::from(format!("decoder-{action}-{index}")))
                        .label(label)
                        .w_full()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.navigation_error = request(action, index, generation).is_err();
                            cx.notify();
                        })),
                );
            }
        }
        for (id, label, action, offset, visible) in [
            (
                "decoder-prev-server",
                "Previous servers",
                "servers-page",
                self.server_offset.saturating_sub(25),
                self.server_offset > 0,
            ),
            (
                "decoder-next-server",
                "Next servers",
                "servers-page",
                self.server_offset.saturating_add(25),
                self.server_offset + 25 < self.server_total,
            ),
            (
                "decoder-prev-episode",
                "Previous episodes",
                "episodes-page",
                self.episode_offset.saturating_sub(25),
                self.episode_offset > 0,
            ),
            (
                "decoder-next-episode",
                "Next episodes",
                "episodes-page",
                self.episode_offset.saturating_add(25),
                self.episode_offset + 25 < self.episode_total,
            ),
        ] {
            if visible {
                content = content.child(Button::new(id).label(label).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.navigation_error = request(action, offset, generation).is_err();
                        cx.notify();
                    },
                )));
            }
        }
        content
    }
}
