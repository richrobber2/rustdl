use super::*;
use serde::Deserialize;
const FILTERS: [&str; 4] = ["all", "active", "issue", "complete"];
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub kind: String,
    pub category: String,
    pub phase: String,
    pub phase_label: String,
    pub title: String,
    pub done: u64,
    pub total: u64,
    pub issue: bool,
    pub issue_detail: Option<String>,
    pub can_play: bool,
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Page {
    pub ok: bool,
    pub detail: String,
    pub offset: usize,
    pub total_items: usize,
    pub items: Vec<Item>,
    pub active: u64,
    pub issues: u64,
    pub completed: u64,
    pub free_bytes: Option<u64>,
    pub unmetered: bool,
    pub charging: bool,
    pub power_save: bool,
    pub thermal_status: usize,
    pub storage_low: bool,
    pub update_detail: String,
}
pub(super) fn request(filter: &str, offset: i32) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let filter = env.new_string(filter).map_err(|e| e.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("requestNativeActivity"),
            jni::jni_sig!("(Ljava/lang/String;I)V"),
            &[JValue::Object(&filter), JValue::Int(offset)],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}
fn open(id: &str, action: &str) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let id = env.new_string(id).map_err(|e| e.to_string())?;
        let action = env.new_string(action).map_err(|e| e.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("openNativeActivityItem"),
            jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&id), JValue::Object(&action)],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}
impl Home {
    fn activity_page_button(
        &self,
        key: &'static str,
        label: &'static str,
        offset: i32,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(key)
            .label(label)
            .large()
            .w_full()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.navigation_error =
                    request(FILTERS[this.activity_filter as usize % 4], offset).is_err();
                cx.notify();
            }))
    }
    pub(super) fn render_activity(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let privacy = self.settings.as_ref().is_none_or(|s| s.inspection_privacy);
        let mut content = semantic_scroll("native-activity", &self.scrolls[10])
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
                    .child(semantic_text("activity-text-1", "Activity Center")),
            )
            .child(
                Button::new("activity-filter")
                    .label(format!(
                        "Filter · {}",
                        ["All", "Active", "Issues", "Completed"][self.activity_filter as usize % 4]
                    ))
                    .large()
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.activity_filter = (this.activity_filter + 1) % 4;
                        this.navigation_error =
                            request(FILTERS[this.activity_filter as usize], 0).is_err();
                        cx.notify();
                    })),
            );
        let mut offset = 0;
        if let Some(page) = &self.activity {
            offset = page.offset as i32;
            if !page.detail.is_empty() {
                content = content
                    .child(div().child(semantic_text("activity-text-2", page.detail.clone())));
            }
            if page.ok {
                content = content
                    .child(div().child(semantic_text(
                        "activity-text-3",
                        format!(
                            "{} active · {} issues · {} completed",
                            page.active, page.issues, page.completed
                        ),
                    )))
                    .child(div().child(semantic_text(
                        "activity-text-4",
                        format!(
                            "Free space · {}",
                            page.free_bytes
                                .map(|bytes| diagnostics::bytes(bytes.min(i64::MAX as u64) as i64))
                                .unwrap_or_else(|| "Unavailable".into())
                        ),
                    )))
                    .child(div().child(semantic_text(
                        "activity-text-5",
                        if page.power_save {
                            "Battery saver · reduced concurrency"
                        } else if page.charging {
                            "Charging · performance enabled"
                        } else {
                            "Balanced"
                        },
                    )))
                    .child(div().child(semantic_text(
                        "activity-text-6",
                        format!(
                            "Thermal · {}",
                            [
                                "Normal",
                                "Light",
                                "Moderate",
                                "Severe",
                                "Critical",
                                "Emergency",
                                "Shutdown"
                            ][page.thermal_status.min(6)]
                        ),
                    )))
                    .child(div().child(semantic_text(
                        "activity-text-7",
                        if page.unmetered {
                            "Unmetered network"
                        } else {
                            "Metered or unknown network"
                        },
                    )));
                if page.storage_low {
                    content = content
                        .child(div().child(semantic_text("activity-text-8", "Storage is low")));
                }
                if !page.update_detail.is_empty() {
                    content = content.child(
                        div().child(semantic_text("activity-text-9", page.update_detail.clone())),
                    );
                }
                if page.items.is_empty() {
                    content = content.child(div().child(semantic_text(
                        "activity-text-10",
                        "No activity matches this filter.",
                    )));
                }
                for item in &page.items {
                    let mut card = div()
                        .id(format!("activity-row-{}", item.id))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_3()
                        .border_1()
                        .rounded_md()
                        .child(div().font_semibold().child(semantic_text(
                            "activity-text-11",
                            if privacy {
                                "Downloaded media".to_owned()
                            } else {
                                item.title.clone()
                            },
                        )))
                        .child(div().child(semantic_text(
                            "activity-text-12",
                            format!(
                                "{} · {}",
                                item.phase_label,
                                progress::transfer_progress(item.done, item.total)
                            ),
                        )));
                    if item.issue {
                        card = card.child(div().child(semantic_text(
                            "activity-text-13",
                            if privacy {
                                "Action needs attention".to_owned()
                            } else {
                                item.issue_detail
                                    .clone()
                                    .unwrap_or_else(|| "Action needs attention".into())
                            },
                        )));
                    }
                    let action = if item.kind == "transfer" {
                        "transfer"
                    } else if item.can_play {
                        "play"
                    } else {
                        "queue"
                    };
                    let label = if action == "transfer" {
                        "Transfer"
                    } else if action == "play" {
                        "Play"
                    } else {
                        "Queue"
                    };
                    let id = item.id.clone();
                    card = card.child(
                        Button::new(format!("activity-item-{id}"))
                            .label(label)
                            .large()
                            .w_full()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.navigation_error = open(&id, action).is_err();
                                cx.notify();
                            })),
                    );
                    content = content.child(card);
                }
                if page.offset > 0 {
                    content = content.child(self.activity_page_button(
                        "activity-prev",
                        "Previous",
                        offset.saturating_sub(25),
                        cx,
                    ));
                }
                if page.offset + page.items.len() < page.total_items {
                    content = content.child(self.activity_page_button(
                        "activity-next",
                        "Next",
                        offset.saturating_add(25),
                        cx,
                    ));
                }
            }
        } else {
            content =
                content.child(div().child(semantic_text("activity-text-14", "Loading activity…")));
        }
        content =
            content.child(self.activity_page_button("activity-refresh", "Refresh", offset, cx));
        if self.navigation_error {
            content = content.child(div().child(semantic_text(
                "activity-text-15",
                "Action unavailable. Try again.",
            )));
        }
        content.child(
            Button::new("activity-back")
                .label("Back")
                .large()
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| {
                    set_screen(0, Ordering::Release);
                    this.screen = 0;
                    let _ = notify_screen("home");
                    cx.notify();
                })),
        )
    }
}
