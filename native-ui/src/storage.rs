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
            .on_click(cx.listener(move |this, _, _, cx| {
                this.navigation_error = request(action, &id, offset).is_err();
                cx.notify();
            }))
    }
    pub(super) fn render_storage(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let privacy = self.settings.as_ref().is_none_or(|s| s.inspection_privacy);
        let mut content = ui::body("native-storage", &self.scrolls[Screen::Storage as usize]);
        let mut offset = 0;
        let mut pages = ui::button_row();
        if let Some(page) = &self.storage {
            offset = page.offset as i32;
            if !page.detail.is_empty() {
                content = content.child(if page.ok {
                    ui::muted_text("storage-text-2", page.detail.clone(), cx)
                } else {
                    ui::error_text("storage-text-2", page.detail.clone(), cx)
                });
            }
            if page.ok {
                let mut usage = ui::card(cx).gap_2();
                for (label, bytes) in [
                    ("Saved videos", page.video_bytes),
                    ("Partial downloads", page.partial_bytes),
                    ("Thumbnail cache", page.thumbnail_bytes),
                    ("Metadata", page.metadata_bytes),
                ] {
                    usage = usage.child(div().child(semantic_text(
                        format!("storage-metric-{label}"),
                        format!(
                            "{label} · {}",
                            diagnostics::bytes(bytes.min(i64::MAX as u64) as i64)
                        ),
                    )));
                }
                usage = usage.child(ui::muted_text(
                    "storage-text-4",
                    format!(
                        "{} watched · {} duplicates · {} stale downloads",
                        page.watched_count, page.duplicate_count, page.stale_count
                    ),
                    cx,
                ));
                let mut cleanup = ui::button_row();
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
                        cleanup = cleanup.child(self.storage_button(
                            format!("cleanup-{action}"),
                            label.into(),
                            action,
                            "".into(),
                            offset,
                            cx,
                        ));
                    }
                }
                content = content.child(usage.child(cleanup));
                if page.items.is_empty() {
                    content = content.child(ui::card(cx).child(ui::muted_text(
                        "storage-text-5",
                        "No stored videos or partial downloads.",
                        cx,
                    )));
                }
                for item in &page.items {
                    let mut card = ui::card(cx)
                        .id(format!("storage-row-{}", item.id))
                        .gap_2()
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
                        .child(ui::muted_text(
                            "storage-text-7",
                            diagnostics::bytes(item.bytes.min(i64::MAX as u64) as i64),
                            cx,
                        ));
                    if item.watched {
                        card = card.child(ui::muted_text("storage-text-8", "Watched", cx));
                    }
                    if item.duplicate {
                        card =
                            card.child(ui::muted_text("storage-text-9", "Duplicate content", cx));
                    }
                    if item.stale {
                        card = card.child(ui::muted_text(
                            "storage-text-10",
                            "Stale partial download",
                            cx,
                        ));
                    }
                    if item.kind == "video" {
                        card = card.child(
                            ui::button_row().child(
                                self.storage_button(
                                    format!("storage-delete-{}", item.id),
                                    "Delete video".into(),
                                    "delete",
                                    item.id.clone(),
                                    offset,
                                    cx,
                                )
                                .danger(),
                            ),
                        );
                    }
                    content = content.child(card);
                }
                if page.offset > 0 {
                    pages = pages.child(self.storage_button(
                        "storage-prev".into(),
                        "Previous".into(),
                        "snapshot",
                        "".into(),
                        offset.saturating_sub(25),
                        cx,
                    ));
                }
                if page.offset + page.items.len() < page.total_items {
                    pages = pages.child(self.storage_button(
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
            content = content.child(ui::card(cx).child(ui::muted_text(
                "storage-text-11",
                "Loading storage…",
                cx,
            )));
        }
        content = content.child(pages.child(self.storage_button(
            "storage-refresh".into(),
            "Refresh".into(),
            "snapshot",
            "".into(),
            offset,
            cx,
        )));
        if self.navigation_error {
            content = content.child(ui::error_text(
                "storage-text-12",
                "Action unavailable. Try again.",
                cx,
            ));
        }
        ui::page(cx)
            .child(ui::header(
                self.back_button("storage-back", cx),
                "storage-text-1",
                "Storage",
            ))
            .child(content)
    }
}
