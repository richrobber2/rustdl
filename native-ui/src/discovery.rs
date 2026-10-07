use super::*;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Page {
    pub ok: bool,
    pub kind: String,
    pub detail: String,
    pub token: String,
    pub offset: usize,
    pub total: usize,
    pub completed: usize,
    pub ready: usize,
    pub issue_count: usize,
    pub finished: bool,
    pub cancelled: bool,
    pub can_continue: bool,
    pub next_offset: Option<i32>,
    pub previous_offset: Option<i32>,
    pub items: Vec<Item>,
    pub selection: Option<Vec<String>>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub qualities: Vec<Quality>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Quality {
    pub id: String,
    pub label: String,
}

fn command(action: &str, payload: serde_json::Value) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let action = env.new_string(action).map_err(|e| e.to_string())?;
        let payload = env
            .new_string(payload.to_string())
            .map_err(|e| e.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("requestNativeDiscovery"),
            jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&action), JValue::Object(&payload)],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}
impl Home {
    pub(super) fn render_discovery(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let privacy = self.settings.as_ref().is_none_or(|s| s.inspection_privacy);
        let mut content = ui::body(
            "native-discovery",
            &self.scrolls[Screen::Discovery as usize],
        )
        .child(
            Button::new("discovery-input")
                .label("Paste video links")
                .primary()
                .large()
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.navigation_error = mobile_jni::with_env(|env| {
                        let activity = mobile_jni::activity(env)?;
                        env.call_method(
                            &activity,
                            jni::jni_str!("inputNativeDiscovery"),
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
        if let Some(page) = &self.discovery {
            if page.kind == "loading" {
                content = content.child(ui::muted_text("discovery-text-2", "Working…", cx));
            }
            if !page.detail.is_empty() {
                content =
                    content.child(ui::muted_text("discovery-text-3", page.detail.clone(), cx));
            }
            if page.kind == "qualities" || page.kind == "playlist" {
                content = content.child(ui::card_title(
                    "discovery-text-4",
                    format!(
                        "{} candidates · {} selected",
                        page.total,
                        self.discovery_picks.len()
                    ),
                ));
                let token = page.token.clone();
                let offset = page.offset;
                let mut selection = ui::button_row();
                selection = selection.child(Button::new("discovery-all").label("Select all candidates")
                    .on_click(cx.listener(move |this,_,_,cx| {
                        this.navigation_error=command("bulk",serde_json::json!({"token":token,"offset":offset,"all":true,"format":"original"})).is_err();cx.notify();
                    })));
                content = content.child(
                    selection.child(
                        Button::new("discovery-clear")
                            .label("Clear selection")
                            .large()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.discovery_picks.clear();
                                cx.notify();
                            })),
                    ),
                );
                if page.kind == "qualities" {
                    let formats = ["original", "360p", "480p", "720p", "1080p", "audio"];
                    content = content.child(
                        Button::new("discovery-bulk-format")
                            .label(format!(
                                "Bulk format: {}",
                                formats[self.discovery_format as usize % formats.len()]
                            ))
                            .large()
                            .w_full()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.discovery_format = (this.discovery_format + 1) % 6;
                                cx.notify();
                            })),
                    );
                    let token = page.token.clone();
                    let offset = page.offset;
                    content=content.child(Button::new("discovery-apply-format").label("Apply format to selected candidates").large().w_full()
                        .on_click(cx.listener(move |this,_,_,cx| {
                            let formats=["original","360p","480p","720p","1080p","audio"];
                            let picks=this.discovery_picks.values().cloned().collect::<Vec<_>>();
                            this.navigation_error=command("bulk",serde_json::json!({"token":token,"offset":offset,"picks":picks,"format":formats[this.discovery_format as usize%6]})).is_err();cx.notify();
                        })));
                }
                for item in &page.items {
                    let picked = self.discovery_picks.get(&item.id);
                    let quality = picked
                        .and_then(|id| item.qualities.iter().find(|q| &q.id == id))
                        .or_else(|| item.qualities.first());
                    let id = item.id.clone();
                    let first = if page.kind == "playlist" {
                        Some(item.id.clone())
                    } else {
                        quality.map(|q| q.id.clone())
                    };
                    let mut card = ui::card(cx)
                        .id(format!("discovery-row-{}", item.id))
                        .gap_2()
                        .child(div().font_semibold().child(semantic_text(
                            "discovery-text-5",
                            if privacy {
                                "Media candidate".to_owned()
                            } else {
                                item.title.clone()
                            },
                        )));
                    if !privacy {
                        card =
                            card.child(ui::muted_text("discovery-text-6", item.detail.clone(), cx));
                    }
                    let mut actions = ui::button_row().child(
                        Button::new(SharedString::from(format!("pick-{id}")))
                            .label(if picked.is_some() {
                                "Deselect"
                            } else {
                                "Select"
                            })
                            .large()
                            .when(picked.is_none(), |button| button.primary())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if this.discovery_picks.remove(&id).is_none()
                                    && let Some(first) = &first
                                {
                                    this.discovery_picks.insert(id.clone(), first.clone());
                                }
                                cx.notify();
                            })),
                    );
                    if let Some(quality) = quality {
                        let id = item.id.clone();
                        let choices = item.qualities.clone();
                        let selected = quality.id.clone();
                        actions = actions.child(
                            Button::new(SharedString::from(format!("format-{id}")))
                                .label(format!(
                                    "Format: {}",
                                    if privacy {
                                        "Selected format"
                                    } else {
                                        &quality.label
                                    }
                                ))
                                .large()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let index =
                                        choices.iter().position(|q| q.id == selected).unwrap_or(0);
                                    if let Some(next) = choices.get((index + 1) % choices.len()) {
                                        this.discovery_picks.insert(id.clone(), next.id.clone());
                                    }
                                    cx.notify();
                                })),
                        );
                    }
                    content = content.child(card.child(actions));
                }
                let mut pages = ui::button_row();
                for (id, label, offset) in [
                    (
                        "discovery-previous",
                        "Previous candidates",
                        page.previous_offset,
                    ),
                    ("discovery-next", "Next candidates", page.next_offset),
                ] {
                    if let Some(offset) = offset {
                        let token = page.token.clone();
                        pages = pages.child(Button::new(id).label(label).on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.navigation_error = command(
                                    "page",
                                    serde_json::json!({"token":token,"offset":offset}),
                                )
                                .is_err();
                                cx.notify();
                            },
                        )));
                    }
                }
                content = content.child(pages);
                let playlist = page.kind == "playlist";
                content = content.child(
                    Button::new("discovery-import")
                        .primary()
                        .label(if playlist {
                            "Prepare selected playlist items"
                        } else {
                            "Download selected formats"
                        })
                        .large()
                        .w_full()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let picks = this.discovery_picks.values().cloned().collect::<Vec<_>>();
                            this.navigation_error = command(
                                if playlist { "prepare" } else { "import" },
                                serde_json::json!({"picks":picks}),
                            )
                            .is_err();
                            cx.notify();
                        })),
                );
            }
        }
        if let Some(page) = &self.discovery
            && page.kind == "preparation"
        {
            content = content.child(ui::card_title(
                "discovery-text-7",
                format!(
                    "{} of {} prepared · {} ready · {} issues",
                    page.completed, page.total, page.ready, page.issue_count
                ),
            ));
            if page.cancelled {
                content = content.child(ui::muted_text(
                    "discovery-text-8",
                    "Preparation cancelled",
                    cx,
                ));
            }
            if page.can_continue {
                let token = page.token.clone();
                content = content.child(
                    Button::new("discovery-formats")
                        .label("Continue to formats")
                        .large()
                        .w_full()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.navigation_error =
                                command("formats", serde_json::json!({"token":token})).is_err();
                            cx.notify();
                        })),
                );
            }
            if page.finished && (page.cancelled || page.issue_count > 0) {
                let token = page.token.clone();
                content = content.child(
                    Button::new("discovery-retry")
                        .label("Retry preparation")
                        .large()
                        .w_full()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.navigation_error =
                                command("retry", serde_json::json!({"token":token})).is_err();
                            cx.notify();
                        })),
                );
            }
            if !page.finished && !page.cancelled {
                let token = page.token.clone();
                content = content.child(
                    Button::new("discovery-cancel")
                        .label("Cancel preparation")
                        .large()
                        .w_full()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.navigation_error =
                                command("cancel", serde_json::json!({"token":token})).is_err();
                            cx.notify();
                        })),
                );
            }
        }
        if self.navigation_error {
            content = content.child(ui::error_text(
                "discovery-text-9",
                "Action unavailable. Try again.",
                cx,
            ));
        }
        ui::page(cx)
            .child(ui::header(
                self.back_button("discovery-back", cx),
                "discovery-text-1",
                "Discover downloads",
            ))
            .child(content)
    }
}
