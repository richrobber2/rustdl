use super::*;
use serde::Deserialize;
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Page {
    pub growing: bool,
    pub downloaded: u64,
    pub total: u64,
    pub quality: String,
    pub audio_tracks: u32,
    pub caption_tracks: u32,
    pub buffered_percent: u32,
    pub buffering: bool,
    pub can_pip: bool,
    pub state: String,
    pub title: String,
    pub detail: String,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub playing: bool,
    pub speed: f64,
    pub muted: bool,
    pub can_next: bool,
    pub up_next_count: u32,
    pub sleep_seconds: u64,
    pub rotation_locked: bool,
    pub fullscreen: bool,
    pub can_source: bool,
    pub can_send: bool,
    pub can_clear_queue: bool,
    pub audio_only: bool,
    pub volume: f64,
    pub restore_volume: f64,
    pub frames_hidden: bool,
}
pub(super) fn request(action: &str, value: f64) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let action = env.new_string(action).map_err(|e| e.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("requestNativePlayer"),
            jni::jni_sig!("(Ljava/lang/String;D)V"),
            &[JValue::Object(&action), JValue::Double(value)],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}
fn surface(bounds: Bounds<Pixels>) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        env.call_method(
            &activity,
            jni::jni_str!("positionNativePlayerSurface"),
            jni::jni_sig!("(FFFF)V"),
            &[
                JValue::Float(f32::from(bounds.origin.x)),
                JValue::Float(f32::from(bounds.origin.y)),
                JValue::Float(f32::from(bounds.size.width)),
                JValue::Float(f32::from(bounds.size.height)),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}
impl Home {
    fn player_key(&mut self, event: &KeyDownEvent, window: &Window, cx: &mut Context<Self>) {
        let Some(action) = progress::player_shortcut(
            event.keystroke.key.as_str(),
            event.keystroke.modifiers.modified(),
            event.is_held,
            window.focused(cx).is_some(),
        ) else {
            return;
        };
        self.navigation_error = request(action.0, action.1).is_err();
        cx.stop_propagation();
        cx.notify();
    }
    fn player_button(
        &self,
        id: &'static str,
        label: &'static str,
        action: &'static str,
        value: f64,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .label(label)
            .large()
            .w_full()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.navigation_error = request(action, value).is_err();
                cx.notify();
            }))
    }
    pub(super) fn render_mini_player(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let privacy = self.settings.as_ref().is_none_or(|s| s.inspection_privacy);
        let page = self.player.as_ref();
        let audio_only = page.is_some_and(|p| p.audio_only);
        let mut frame = div()
            .h(px(if audio_only { 40. } else { 120. }))
            .w_full()
            .bg(rgb(0x000000));
        if audio_only || privacy {
            frame = frame.child(div().p_2().text_color(rgb(0xffffff)).child(semantic_text(
                "player-remaining-text-1",
                if audio_only {
                    "Audio playback"
                } else {
                    "Video frames hidden during inspection"
                },
            )));
        } else {
            frame = frame.child(
                canvas(
                    |_, _, _| (),
                    |bounds, _, _, _| {
                        let _ = surface(bounds);
                    },
                )
                .size_full(),
            );
        }
        let title = if privacy {
            "Downloaded media"
        } else {
            page.map(|p| p.title.as_str()).unwrap_or("Player")
        };
        let playing = page.is_some_and(|p| p.playing);
        let mut controls = div()
            .flex()
            .gap_2()
            .child(self.player_button(
                "mini-play",
                if playing { "Pause" } else { "Play" },
                if playing { "pause" } else { "play" },
                0.,
                cx,
            ))
            .child(self.player_button("mini-expand", "Expand", "expand", 0., cx))
            .child(self.player_button("mini-close", "Close", "close-mini", 0., cx));
        if page.is_some_and(|p| p.can_next) {
            controls = controls.child(self.player_button("mini-next", "Next", "next", 0., cx));
        }
        if page.is_some_and(|p| p.can_pip) {
            controls = controls.child(self.player_button("mini-pip", "PiP", "pip", 0., cx));
        }
        div()
            .id("native-player-layout")
            .on_key_down(cx.listener(|this, event, window, cx| this.player_key(event, window, cx)))
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background.opacity(0.88))
            .text_color(cx.theme().foreground)
            .child(div().flex_1().min_h_0().child(self.render_library(cx)))
            .child(
                div()
                    .flex_none()
                    .p_2()
                    .child(div().child(semantic_text("player-text-1", title.to_owned())))
                    .child(frame)
                    .child(controls),
            )
    }
    pub(super) fn render_player(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let audio_only = self.player.as_ref().is_some_and(|p| p.audio_only);
        let fullscreen = self.player.as_ref().is_some_and(|p| p.fullscreen);
        let privacy = self.settings.as_ref().is_none_or(|s| s.inspection_privacy);
        let mut controls = semantic_scroll(
            "native-player-controls",
            &self.scrolls[Screen::Player as usize],
        )
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll();
        let title = if privacy {
            "Downloaded media".to_owned()
        } else {
            self.player
                .as_ref()
                .map(|p| p.title.clone())
                .unwrap_or_else(|| "Player".into())
        };
        if let Some(page) = &self.player {
            if page.volume.is_finite() {
                let value = (page.volume.clamp(0., 1.) * 100.) as f32;
                if !self.player_volume_dragging
                    && self.player_volume.read(cx).value()
                        != gpui_kit::component::slider::SliderValue::Single(value)
                {
                    self.player_volume
                        .update(cx, |state, cx| state.set_value(value, window, cx));
                }
                controls = controls
                    .child(div().child(semantic_text(
                        "player-text-2",
                        format!("Volume · {:.0}%", value),
                    )))
                    .child(gpui_kit::component::slider::Slider::new(&self.player_volume).w_full());
            }
            if !audio_only && (!privacy || fullscreen) {
                controls = controls.child(self.player_button(
                    "native-player-fullscreen",
                    if fullscreen {
                        "Exit fullscreen"
                    } else {
                        "Fullscreen"
                    },
                    "fullscreen",
                    0.,
                    cx,
                ));
            }
            if page.duration_seconds.is_finite()
                && page.duration_seconds > 0.
                && page.position_seconds.is_finite()
            {
                if !self.player_dragging {
                    let value = ((page.position_seconds / page.duration_seconds).clamp(0., 1.)
                        * 100.) as f32;
                    if self.player_slider.read(cx).value()
                        != gpui_kit::component::slider::SliderValue::Single(value)
                    {
                        self.player_slider
                            .update(cx, |state, cx| state.set_value(value, window, cx));
                    }
                }
                controls = controls
                    .child(gpui_kit::component::slider::Slider::new(&self.player_slider).w_full());
            }
            controls =
                controls.child(div().child(semantic_text("player-text-3", page.state.clone())));
            if page.growing {
                controls = controls.child(div().child(semantic_text(
                    "player-remaining-text-2",
                    if page.total > 0 {
                        format!("{} / {} bytes downloaded", page.downloaded, page.total)
                    } else {
                        format!("{} bytes downloaded", page.downloaded)
                    },
                )));
            } else {
                controls =
                    controls.child(div().child(semantic_text("player-text-4", "Saved locally")));
            }
            controls = controls.child(div().child(semantic_text(
                "player-format",
                if page.audio_only {
                    "Saved audio · M4A"
                } else {
                    "Saved video · MP4"
                },
            )));
            controls = controls.child(div().child(semantic_text(
                "player-text-5",
                format!(
                    "Audio tracks · {} · Caption tracks · {}",
                    page.audio_tracks, page.caption_tracks
                ),
            )));
            if !privacy && !page.quality.is_empty() {
                controls = controls.child(div().child(semantic_text(
                    "player-text-6",
                    format!("Quality · {}", page.quality),
                )));
            }

            if page.can_source {
                controls = controls
                    .child(self.player_button(
                        "native-player-source",
                        "Open source",
                        "source",
                        0.,
                        cx,
                    ))
                    .child(self.player_button(
                        "native-player-quality",
                        "Choose another quality",
                        "quality",
                        0.,
                        cx,
                    ));
            }
            if page.can_send {
                controls = controls.child(self.player_button(
                    "native-player-send",
                    "Send to device",
                    "send",
                    0.,
                    cx,
                ));
            }
            if page.state == "error" {
                controls = controls.child(self.player_button(
                    "native-playback-retry",
                    "Retry playback",
                    "retry",
                    0.,
                    cx,
                ));
            }
            if page.can_pip {
                controls = controls.child(self.player_button(
                    "native-player-pip",
                    "Picture in picture",
                    "pip",
                    0.,
                    cx,
                ));
            }
            if page.buffering {
                controls = controls.child(div().child(semantic_text(
                    "player-text-7",
                    "Buffering · waiting for playable data",
                )));
            }
            if page.growing {
                controls = controls.child(div().child(semantic_text(
                    "player-text-8",
                    format!("Decoder buffer · {}%", page.buffered_percent.min(100)),
                )));
                controls = controls.child(div().child(semantic_text(
                    "player-text-9",
                    "Download in progress · playback updates when complete",
                )));
            }
            if !page.detail.is_empty() {
                controls = controls
                    .child(div().child(semantic_text("player-text-10", page.detail.clone())));
            }
            if page.duration_seconds.is_finite() && page.position_seconds.is_finite() {
                controls = controls.child(div().child(semantic_text(
                    "player-text-11",
                    format!(
                        "{:.0} / {:.0} seconds",
                        page.position_seconds.max(0.),
                        page.duration_seconds.max(0.)
                    ),
                )));
            }
            controls = controls.child(self.player_button(
                "native-player-rotation",
                if page.rotation_locked {
                    "Unlock rotation"
                } else {
                    "Lock rotation"
                },
                "rotation",
                0.,
                cx,
            ));
            if page.sleep_seconds > 0 {
                controls = controls.child(div().child(semantic_text(
                    "player-text-12",
                    format!(
                        "Sleep timer · {} minutes remaining",
                        page.sleep_seconds.div_ceil(60)
                    ),
                )));
            }
            for (id, label, minutes) in [
                ("native-sleep15", "Sleep 15 minutes", 15.),
                ("native-sleep30", "Sleep 30 minutes", 30.),
                ("native-sleep60", "Sleep 60 minutes", 60.),
                ("native-sleep-clear", "Clear sleep timer", 0.),
            ] {
                controls = controls.child(self.player_button(id, label, "sleep", minutes, cx));
            }
            controls = controls.child(div().child(semantic_text(
                "player-up-next-status",
                if page.up_next_count > 0 {
                    format!("Up Next · {} queued", page.up_next_count)
                } else if page.can_next {
                    "Up Next · gallery fallback ready".to_owned()
                } else {
                    "Up Next · queue empty".to_owned()
                },
            )));
            if page.can_clear_queue {
                controls = controls.child(self.player_button(
                    "native-player-clear-queue",
                    "Clear Up Next",
                    "clear-queue",
                    0.,
                    cx,
                ));
            }
            if page.can_next {
                controls = controls.child(self.player_button(
                    "native-player-next",
                    "Play next",
                    "next",
                    0.,
                    cx,
                ));
            }
            if page.state != "loading" && page.state != "error" {
                controls = controls
                    .child(self.player_button(
                        "native-play",
                        if page.playing { "Pause" } else { "Play" },
                        if page.playing { "pause" } else { "play" },
                        0.,
                        cx,
                    ))
                    .child(self.player_button(
                        "native-player-seek",
                        "Seek to position",
                        "seek-dialog",
                        0.,
                        cx,
                    ))
                    .child(self.player_button(
                        "native-rewind",
                        "Back 10 seconds",
                        "seek",
                        (page.position_seconds - 10.).max(0.),
                        cx,
                    ))
                    .child(self.player_button(
                        "native-forward",
                        "Forward 10 seconds",
                        "seek",
                        page.position_seconds + 10.,
                        cx,
                    ));
                controls = controls.child(
                    Button::new("native-speed")
                        .label(format!("Speed · {}×", page.speed))
                        .large()
                        .w_full()
                        .on_click(cx.listener(|this, _, _, cx| {
                            let current = this.player.as_ref().map(|p| p.speed).unwrap_or(1.0);
                            let next = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0]
                                .into_iter()
                                .find(|s| *s > current + 0.01)
                                .unwrap_or(0.5);
                            this.navigation_error = request("speed", next).is_err();
                            cx.notify();
                        })),
                );
                controls = controls.child(
                    Button::new("native-volume")
                        .label(if page.muted { "Unmute" } else { "Mute" })
                        .large()
                        .w_full()
                        .on_click(cx.listener(|this, _, _, cx| {
                            let muted = this.player.as_ref().is_some_and(|p| p.muted);
                            this.navigation_error = request(
                                "volume",
                                if muted {
                                    this.player
                                        .as_ref()
                                        .map(|p| p.restore_volume)
                                        .filter(|v| v.is_finite() && *v > 0.)
                                        .unwrap_or(1.)
                                } else {
                                    0.
                                },
                            )
                            .is_err();
                            cx.notify();
                        })),
                );
            }
        }
        controls = controls
            .child(self.player_button(
                "native-player-share",
                "Share downloaded media",
                "share",
                0.,
                cx,
            ))
            .child(self.player_button(
                "native-player-refresh",
                "Refresh status",
                "snapshot",
                0.,
                cx,
            ))
            .child(self.player_button(
                "native-player-mini",
                "Browse with mini-player",
                "mini",
                0.,
                cx,
            ))
            .child(
                Button::new("native-player-back")
                    .label("Back to library")
                    .large()
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| {
                        let _ = request("pause", 0.);
                        set_screen(Screen::Library, Ordering::Release);
                        this.screen = Screen::Library;
                        let _ = notify_screen("library");
                        let _ = request_library(0, "");
                        cx.notify();
                    })),
            );
        if self.navigation_error {
            controls = controls.child(div().child(semantic_text(
                "player-text-13",
                "Playback action unavailable. Try again.",
            )));
        }
        let mut frame = div().h(px(220.)).w_full().bg(rgb(0x000000));
        if fullscreen {
            frame = frame.h((window.viewport_size().height - px(180.)).max(px(100.)));
            controls = controls.flex_none().h(px(140.));
        }
        if audio_only {
            frame = frame.h(px(60.)).child(
                div()
                    .p_4()
                    .text_color(rgb(0xffffff))
                    .child(semantic_text("player-text-14", "Audio playback")),
            );
        } else if privacy {
            frame = frame.child(div().p_4().text_color(rgb(0xffffff)).child(semantic_text(
                "player-text-15",
                "Video frames hidden during inspection",
            )));
        } else {
            frame = frame.child(
                canvas(
                    |_, _, _| (),
                    |bounds, _, _, _| {
                        let _ = surface(bounds);
                    },
                )
                .size_full(),
            );
        }
        div()
            .id("native-player-layout")
            .on_key_down(cx.listener(|this, event, window, cx| this.player_key(event, window, cx)))
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background.opacity(0.88))
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .p_4()
                    .font_semibold()
                    .child(semantic_text("player-remaining-text-3", title)),
            )
            .child(frame)
            .child(controls)
    }
}
