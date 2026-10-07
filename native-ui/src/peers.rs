use super::*;
use serde::Deserialize;
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Page {
    pub ok: bool,
    pub detail: String,
    pub paired: bool,
    pub item_selected: bool,
    pub title: String,
    pub address: String,
    pub phase: String,
    pub sent: u64,
    pub total: u64,
    pub issue: bool,
    pub receive_enabled: bool,
    pub receive_address: String,
    pub receive_key: String,
    pub qr_size: usize,
    pub qr_cells: Vec<bool>,
}
fn request(action: &str) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let action = env.new_string(action).map_err(|e| e.to_string())?;
        let payload = env.new_string("{}").map_err(|e| e.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("requestNativePeer"),
            jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&action), JValue::Object(&payload)],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}
impl Home {
    pub(super) fn render_peers(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let privacy = self.settings.as_ref().is_none_or(|s| s.inspection_privacy);
        let mut content = semantic_scroll("native-peers", &self.scrolls[7])
            .flex()
            .flex_col()
            .gap_4()
            .p_4()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                div()
                    .font_semibold()
                    .child(semantic_text("peers-text-1", "Device transfers")),
            );
        if let Some(page) = &self.peers {
            if page.receive_enabled {
                content = content.child(div().child(semantic_text(
                    "peers-text-2",
                    "Receiving enabled until pairing expires",
                )));
                if privacy {
                    content = content.child(div().child(semantic_text(
                        "peers-text-3",
                        "Pairing details hidden during inspection",
                    )));
                } else {
                    content = content
                        .child(
                            div()
                                .child(semantic_text("peers-text-4", page.receive_address.clone())),
                        )
                        .child(
                            div().child(semantic_text("peers-text-5", page.receive_key.clone())),
                        );
                    let side = page.qr_size;
                    if (21..=185).contains(&side) && page.qr_cells.len() == side * side {
                        let cells = page.qr_cells.clone();
                        content = content.child(
                            canvas(
                                |_, _, _| (),
                                move |bounds, _, window, _| {
                                    window.paint_quad(fill(bounds, rgb(0xffffff)));
                                    let unit = bounds.size.width / side as f32;
                                    for (index, dark) in cells.iter().enumerate() {
                                        if *dark {
                                            let origin = bounds.origin
                                                + point(
                                                    unit * (index % side) as f32,
                                                    unit * (index / side) as f32,
                                                );
                                            window.paint_quad(fill(
                                                Bounds::new(origin, size(unit, unit)),
                                                rgb(0x000000),
                                            ));
                                        }
                                    }
                                },
                            )
                            .w(px(280.))
                            .h(px(280.)),
                        );
                    }
                }
                content = content.child(
                    Button::new("peer-copy")
                        .label("Copy receiver pairing")
                        .large()
                        .w_full()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.navigation_error = request("copy").is_err();
                            cx.notify();
                        })),
                );
            }
            if page.item_selected {
                content = content.child(div().child(semantic_text(
                    "peers-text-6",
                    if privacy {
                        "Downloaded media".to_owned()
                    } else {
                        page.title.clone()
                    },
                )));
            }
            if !page.detail.is_empty() {
                content =
                    content.child(div().child(semantic_text("peers-text-7", page.detail.clone())));
            }
            content = content.child(div().child(semantic_text(
                "peers-text-8",
                if page.paired {
                    "Receiver paired"
                } else {
                    "Pair with a receiving device"
                },
            )));
            if !privacy && !page.address.is_empty() {
                content =
                    content.child(div().child(semantic_text("peers-text-9", page.address.clone())));
            }
            if !page.phase.is_empty() {
                content =
                    content.child(div().child(semantic_text("peers-text-10", page.phase.clone())));
            }
            if page.total > 0 {
                content = content.child(div().child(semantic_text(
                    "peers-text-11",
                    progress::transfer_progress(page.sent, page.total),
                )));
            }
            if page.issue {
                content = content.child(div().child(semantic_text(
                    "peers-text-12",
                    "Transfer failed. Check pairing and try again.",
                )));
            }
            if page.paired
                && page.item_selected
                && matches!(
                    page.phase.as_str(),
                    "idle" | "ready" | "complete" | "failed"
                )
            {
                content = content.child(
                    Button::new("peer-send")
                        .label("Send encrypted item")
                        .large()
                        .w_full()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.navigation_error = request("send").is_err();
                            cx.notify();
                        })),
                );
            }
        } else {
            content = content
                .child(div().child(semantic_text("peers-text-13", "Loading transfer state…")));
        }
        content = content.child(
            Button::new("peer-receive")
                .label("Enable or renew receiving")
                .large()
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.navigation_error = request("receive").is_err();
                    cx.notify();
                })),
        );
        content = content.child(
            Button::new("peer-pair")
                .label("Enter receiver pairing details")
                .large()
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.navigation_error = mobile_jni::with_env(|env| {
                        let activity = mobile_jni::activity(env)?;
                        env.call_method(
                            &activity,
                            jni::jni_str!("pairNativePeer"),
                            jni::jni_sig!("()V"),
                            &[],
                        )
                        .map_err(|e| e.to_string())?;
                        Ok(())
                    })
                    .is_err();
                    cx.notify();
                })),
        );
        content = content.child(
            Button::new("peer-refresh")
                .label("Refresh")
                .large()
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.navigation_error = request("snapshot").is_err();
                    cx.notify();
                })),
        );
        if self.navigation_error {
            content = content.child(div().child(semantic_text(
                "peers-text-14",
                "Action unavailable. Try again.",
            )));
        }
        content.child(
            Button::new("peer-back")
                .label("Back to library")
                .large()
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| {
                    set_screen(5, Ordering::Release);
                    this.screen = 5;
                    let _ = notify_screen("library");
                    let _ = request_library(0, "");
                    cx.notify();
                })),
        )
    }
}
