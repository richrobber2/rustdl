use super::*;
use serde::Deserialize;
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub title: String,
    pub bytes: u64,
    pub watched: bool,
    pub duplicate: bool,
    pub stale: bool,
    pub kind: String,
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Page {
    pub ok: bool,
    pub detail: String,
    pub offset: usize,
    pub total_items: usize,
    pub items: Vec<Item>,
    pub video_bytes: u64,
    pub partial_bytes: u64,
    pub thumbnail_bytes: u64,
    pub metadata_bytes: u64,
    pub watched_count: usize,
    pub duplicate_count: usize,
    pub stale_count: usize,
}
pub(super) fn request(action: &str, id: &str, offset: i32) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let action = env.new_string(action).map_err(|e| e.to_string())?;
        let id = env.new_string(id).map_err(|e| e.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("requestNativeStorage"),
            jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;I)V"),
            &[
                JValue::Object(&action),
                JValue::Object(&id),
                JValue::Int(offset),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}
impl Home {
    fn storage_button(
        &self,
        key: String,
        label: String,
        action: &'static str,
        id: String,
        offset: i32,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(key)
            .label(label)
            .large()
            .w_full()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.navigation_error = request(action, &id, offset).is_err();
                cx.notify();
            }))
    }
    pub(super) fn render_storage(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let privacy = self.settings.as_ref().is_none_or(|s| s.inspection_privacy);
        let mut content = semantic_scroll("native-storage", &self.scrolls[9])
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
                    .child(semantic_text("storage-text-1", "Storage")),
            );
        let mut offset = 0;
        if let Some(page) = &self.storage {
            offset = page.offset as i32;
            if !page.detail.is_empty() {
                content = content
                    .child(div().child(semantic_text("storage-text-2", page.detail.clone())));
            }
            if page.ok {
                for (label, bytes) in [
                    ("Saved videos", page.video_bytes),
                    ("Partial downloads", page.partial_bytes),
                    ("Thumbnail cache", page.thumbnail_bytes),
                    ("Metadata", page.metadata_bytes),
                ] {
                    content = content.child(div().child(semantic_text(
                        format!("storage-metric-{label}"),
                        format!(
                            "{label} · {}",
                            diagnostics::bytes(bytes.min(i64::MAX as u64) as i64)
                        ),
                    )));
                }
                content = content.child(div().child(semantic_text(
                    "storage-text-4",
                    format!(
                        "{} watched · {} duplicates · {} stale downloads",
                        page.watched_count, page.duplicate_count, page.stale_count
                    ),
                )));
                for (action, label, count) in [
                    ("watched", "Remove watched videos", page.watched_count),
                    ("stale", "Clear stale downloads", page.stale_count),
                    (
                        "thumbnails",
                        "Clear thumbnail cache",
                        usize::from(page.thumbnail_bytes > 0),
                    ),
                ] {
                    if count > 0 {
                        content = content.child(self.storage_button(
                            format!("cleanup-{action}"),
                            label.into(),
                            action,
                            "".into(),
                            offset,
                            cx,
                        ));
                    }
                }
                if page.items.is_empty() {
                    content = content.child(div().child(semantic_text(
                        "storage-text-5",
                        "No stored videos or partial downloads.",
                    )));
                }
                for item in &page.items {
                    let mut card = div()
                        .id(format!("storage-row-{}", item.id))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_3()
                        .border_1()
                        .rounded_md()
                        .child(div().font_semibold().child(semantic_text(
                            "storage-text-6",
                            if privacy {
                                if item.kind == "partial" {
                                    "Partial download"
                                } else {
                                    "Downloaded media"
                                }
                                .to_owned()
                            } else {
                                item.title.clone()
                            },
                        )))
                        .child(div().child(semantic_text(
                            "storage-text-7",
                            diagnostics::bytes(item.bytes.min(i64::MAX as u64) as i64),
                        )));
                    if item.watched {
                        card = card.child(div().child(semantic_text("storage-text-8", "Watched")));
                    }
                    if item.duplicate {
                        card = card.child(
                            div().child(semantic_text("storage-text-9", "Duplicate content")),
                        );
                    }
                    if item.stale {
                        card = card.child(
                            div().child(semantic_text("storage-text-10", "Stale partial download")),
                        );
                    }
                    if item.kind == "video" {
                        card = card.child(self.storage_button(
                            format!("storage-delete-{}", item.id),
                            "Delete video".into(),
                            "delete",
                            item.id.clone(),
                            offset,
                            cx,
                        ));
                    }
                    content = content.child(card);
                }
                if page.offset > 0 {
                    content = content.child(self.storage_button(
                        "storage-prev".into(),
                        "Previous".into(),
                        "snapshot",
                        "".into(),
                        offset.saturating_sub(25),
                        cx,
                    ));
                }
                if page.offset + page.items.len() < page.total_items {
                    content = content.child(self.storage_button(
                        "storage-next".into(),
                        "Next".into(),
                        "snapshot",
                        "".into(),
                        offset.saturating_add(25),
                        cx,
                    ));
                }
            }
        } else {
            content =
                content.child(div().child(semantic_text("storage-text-11", "Loading storage…")));
        }
        content = content.child(self.storage_button(
            "storage-refresh".into(),
            "Refresh".into(),
            "snapshot",
            "".into(),
            offset,
            cx,
        ));
        if self.navigation_error {
            content = content.child(div().child(semantic_text(
                "storage-text-12",
                "Action unavailable. Try again.",
            )));
        }
        content.child(
            Button::new("storage-back")
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
