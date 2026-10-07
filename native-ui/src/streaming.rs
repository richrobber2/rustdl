//! Streaming controls use presentation-only manifests, never decoder URLs.
use super::*;
use gpui_kit::base::Disableable;
use serde::Deserialize;
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Page {
    pub ok: bool,
    pub detail: String,
    pub title: String,
    pub token: String,
    pub episode: String,
    pub episode_offset: usize,
    pub episode_total: usize,
    pub source_offset: usize,
    pub source_total: usize,
    pub source_filter: String,
    pub watchlisted: bool,
    pub sources: Vec<Source>,
    pub episodes: Vec<Episode>,
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Source {
    pub index: usize,
    pub label: String,
    pub language: String,
    pub available: bool,
    pub issue: bool,
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Episode {
    pub index: usize,
    pub number: String,
    pub title: String,
    pub selected: bool,
}
fn request(action: &str, token: &str, index: usize) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let action = env.new_string(action).map_err(|e| e.to_string())?;
        let token = env.new_string(token).map_err(|e| e.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("requestNativeStreaming"),
            jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;I)V"),
            &[
                JValue::Object(&action),
                JValue::Object(&token),
                JValue::Int(index as i32),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}
impl Page {
    pub(super) fn view(
        &self,
        privacy: bool,
        cx: &mut Context<Home>,
        scroll: &ScrollHandle,
    ) -> impl IntoElement {
        let mut content = semantic_scroll("native-streaming-controls", scroll)
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .p_4();
        content = content.child(div().text_2xl().child(semantic_text(
            "streaming-text-1",
            if privacy {
                "Anime playback".to_owned()
            } else {
                self.title.clone()
            },
        )));
        if !self.detail.is_empty() {
            content =
                content.child(div().child(semantic_text("streaming-text-2", self.detail.clone())));
        }
        if self.ok {
            let token = self.token.clone();
            let saved = self.watchlisted;
            content = content.child(
                Button::new("native-stream-watchlist")
                    .label(if saved {
                        "Remove from watchlist"
                    } else {
                        "Add to watchlist"
                    })
                    .large()
                    .w_full()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.navigation_error = request(
                            if saved {
                                "watchlist-remove"
                            } else {
                                "watchlist-add"
                            },
                            &token,
                            0,
                        )
                        .is_err();
                        cx.notify();
                    })),
            );
        }
        for (index, label) in ["All", "Sub", "Dub", "Ready", "Issues"].iter().enumerate() {
            let token = self.token.clone();
            content = content.child(
                Button::new(SharedString::from(format!("stream-filter-{index}")))
                    .label(
                        if self.source_filter == ["all", "sub", "dub", "ready", "issues"][index] {
                            format!("✓ {label}")
                        } else {
                            (*label).to_owned()
                        },
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.navigation_error = request("filter", &token, index).is_err();
                        cx.notify();
                    })),
            );
        }
        for source in &self.sources {
            let token = self.token.clone();
            let index = source.index;
            content = content.child(
                Button::new(SharedString::from(format!("stream-server-{index}")))
                    .label(if privacy {
                        format!("Server {}", index + 1)
                    } else {
                        source.label.clone()
                    })
                    .large()
                    .w_full()
                    .disabled(!source.available)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.navigation_error = request("play", &token, index).is_err();
                        cx.notify();
                    })),
            );
        }
        for (id, label, offset, visible) in [
            (
                "stream-servers-previous",
                "Previous servers",
                self.source_offset.saturating_sub(64),
                self.source_offset > 0,
            ),
            (
                "stream-servers-next",
                "Next servers",
                self.source_offset.saturating_add(64),
                self.source_offset + 64 < self.source_total,
            ),
        ] {
            if visible {
                let token = self.token.clone();
                content = content.child(Button::new(id).label(label).large().w_full().on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.navigation_error = request("sources", &token, offset).is_err();
                        cx.notify();
                    }),
                ));
            }
        }
        for episode in &self.episodes {
            let token = self.token.clone();
            let index = episode.index;
            content = content.child(
                Button::new(SharedString::from(format!("stream-episode-{index}")))
                    .label(if privacy {
                        if episode.selected {
                            "Selected episode".to_owned()
                        } else {
                            "Episode".to_owned()
                        }
                    } else {
                        format!("{} · {}", episode.number, episode.title)
                    })
                    .large()
                    .w_full()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.navigation_error = request("episode", &token, index).is_err();
                        cx.notify();
                    })),
            );
        }
        for (id, label, offset, visible) in [
            (
                "stream-previous",
                "Previous episodes",
                self.episode_offset.saturating_sub(25),
                self.episode_offset > 0,
            ),
            (
                "stream-next",
                "Next episodes",
                self.episode_offset.saturating_add(25),
                self.episode_offset + 25 < self.episode_total,
            ),
        ] {
            if visible {
                let token = self.token.clone();
                content = content.child(Button::new(id).label(label).large().w_full().on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.navigation_error = request("page", &token, offset).is_err();
                        cx.notify();
                    }),
                ));
            }
        }
        let token = self.token.clone();
        content = content
            .child(
                Button::new("native-stream-refresh")
                    .label("Refresh streaming controls")
                    .large()
                    .w_full()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.navigation_error = request("refresh", &token, 0).is_err();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("native-stream-back")
                    .label("Back to anime")
                    .large()
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| {
                        set_screen(Screen::Anime, Ordering::Release);
                        this.screen = Screen::Anime;
                        let _ = notify_screen("anime");
                        this.navigation_error = anime::request("catalog", "{}", 0).is_err();
                        cx.notify();
                    })),
            );
        content
    }
}
