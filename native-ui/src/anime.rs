use super::*;
use serde::Deserialize;
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub title: String,
    pub poster: String,
    pub poster_width: u32,
    pub poster_height: u32,
    pub watchlisted: bool,
    pub sub: Option<String>,
    pub dub: Option<String>,
    pub total: Option<String>,
    pub media_type: Option<String>,
    pub day: String,
    pub latest_episode: String,
    pub display_release: Option<u64>,
    pub display_release_text: String,
    pub is_new: bool,
    pub unavailable: bool,
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Episode {
    pub id: String,
    pub title: String,
    pub number: String,
    pub released_at: Option<u64>,
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Page {
    pub ok: bool,
    pub detail: String,
    pub kind: String,
    pub page: u16,
    pub pages: u16,
    pub items: Vec<Item>,
    pub episodes: Vec<Episode>,
    pub selection: String,
    pub title: String,
    pub new_count: usize,
    pub first_visit: bool,
}
pub(super) fn request(action: &str, id: &str, page: i32) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let action = env.new_string(action).map_err(|e| e.to_string())?;
        let id = env.new_string(id).map_err(|e| e.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("requestNativeAnime"),
            jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;I)V"),
            &[
                JValue::Object(&action),
                JValue::Object(&id),
                JValue::Int(page),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}
impl Home {
    fn anime_button(
        &self,
        id: String,
        label: String,
        action: &'static str,
        handle: String,
        page: i32,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .label(label)
            .large()
            .w_full()
            .when(matches!(action, "episodes" | "play"), |button| {
                button.primary()
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.navigation_error = request(action, &handle, page).is_err();
                cx.notify();
            }))
    }
    pub(super) fn render_anime(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let privacy = self.settings.as_ref().is_none_or(|s| s.inspection_privacy);
        let mut content = ui::body("native-anime", &self.scrolls[Screen::Anime as usize])
            .pt_0()
            .child(ui::muted_text(
                "anime-subtitle",
                "Discover shows and keep your favorites close.",
                cx,
            ))
            .child(self.anime_button(
                "anime-search".into(),
                "Search and categories".into(),
                "search",
                "".into(),
                1,
                cx,
            ))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        self.anime_button(
                            "anime-watchlist".into(),
                            "Watchlist".into(),
                            "watchlist",
                            "".into(),
                            1,
                            cx,
                        )
                        .w_auto()
                        .flex_1()
                        .min_w_0(),
                    )
                    .child(
                        self.anime_button(
                            "anime-catalog".into(),
                            "Catalog".into(),
                            "catalog",
                            "".into(),
                            1,
                            cx,
                        )
                        .w_auto()
                        .flex_1()
                        .min_w_0(),
                    ),
            );
        if let Some(page) = &self.anime {
            if !page.detail.is_empty() {
                content =
                    content.child(div().child(semantic_text("anime-text-2", page.detail.clone())));
            }
            if page.pages > 0 {
                content = content.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(semantic_text(
                            "anime-text-3",
                            format!("Page {} of {}", page.page, page.pages),
                        )),
                );
                if page.items.is_empty() && page.kind != "episodes" {
                    content = content.child(div().child(semantic_text(
                        "anime-text-4",
                        "No anime items in this view.",
                    )));
                }
                if page.kind == "calendar" {
                    content = content
                        .child(div().child(semantic_text(
                            "anime-text-5",
                            format!("{} shows have new episodes", page.new_count),
                        )))
                        .child(div().child(semantic_text(
                            "anime-text-6",
                            if page.first_visit {
                                "This is your first calendar check."
                            } else {
                                "New badges compare against your previous calendar check."
                            },
                        )));
                }
                let column_count = if f32::from(window.viewport_size().width) >= 600. {
                    3
                } else {
                    2
                };
                let mut columns: Vec<_> = (0..column_count)
                    .map(|_| div().flex().flex_col().flex_1().min_w_0().gap_3())
                    .collect();
                // Stable membership: asynchronous dimensions must not move a tile
                // into another column while the user is scrolling.
                for (index, item) in page.items.iter().enumerate() {
                    let ratio = if item.poster_width > 0 && item.poster_height > 0 {
                        item.poster_width as f32 / item.poster_height as f32
                    } else {
                        2. / 3.
                    };
                    let mut details = div().flex().flex_col().min_w_0().gap_2().child(
                        div().font_semibold().child(semantic_text(
                            "anime-text-7",
                            if privacy {
                                "Anime item".to_owned()
                            } else {
                                item.title.clone()
                            },
                        )),
                    );
                    let mut preview = div()
                        .w_full()
                        .aspect_ratio(ratio)
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_lg()
                        .overflow_hidden()
                        .bg(cx.theme().background.opacity(0.75))
                        .border_1()
                        .border_color(cx.theme().border);
                    if !privacy && item.poster.starts_with("/") {
                        preview = preview.child(
                            gpui::img(std::path::PathBuf::from(item.poster.clone()))
                                .size_full()
                                .object_fit(gpui::ObjectFit::Contain),
                        );
                    } else {
                        preview = preview.child(
                            div()
                                .p_2()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(semantic_text(
                                    "anime-preview-placeholder",
                                    if privacy {
                                        "Preview hidden"
                                    } else {
                                        "No preview"
                                    },
                                )),
                        );
                    }
                    if !item.day.is_empty() {
                        details = details.child(
                            div()
                                .font_semibold()
                                .child(semantic_text("anime-text-8", item.day.clone())),
                        );
                    }
                    if item.is_new {
                        details = details
                            .child(div().child(semantic_text("anime-text-9", "New episode")));
                    }
                    if item.unavailable {
                        details = details.child(div().child(semantic_text(
                            "anime-text-10",
                            "Schedule unavailable. Open episodes to retry.",
                        )));
                    }
                    if !privacy && !item.latest_episode.is_empty() {
                        details = details.child(div().child(semantic_text(
                            "anime-text-11",
                            format!("Latest episode {}", item.latest_episode),
                        )));
                    }
                    if !privacy && !item.display_release_text.is_empty() {
                        details = details.child(div().child(semantic_text(
                            "anime-text-12",
                            item.display_release_text.clone(),
                        )));
                    }
                    if !privacy {
                        if let Some(kind) = &item.media_type {
                            details = details
                                .child(div().child(semantic_text("anime-text-13", kind.clone())));
                        }
                        for (label, value) in [
                            ("Sub", &item.sub),
                            ("Dub", &item.dub),
                            ("Episodes", &item.total),
                        ] {
                            if let Some(value) = value {
                                details = details.child(div().child(semantic_text(
                                    format!("anime-field-{label}"),
                                    format!("{label}: {value}"),
                                )));
                            }
                        }
                    }
                    let card = div()
                        .id(format!("anime-row-{}", item.id))
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_2()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().muted.opacity(0.65))
                        .rounded_lg()
                        .child(preview)
                        .child(details)
                        .child(self.anime_button(
                            format!("watch-{}", item.id),
                            "Episodes".into(),
                            "episodes",
                            item.id.clone(),
                            0,
                            cx,
                        ))
                        .child(
                            self.anime_button(
                                format!("save-{}", item.id),
                                if item.watchlisted {
                                    "Remove saved"
                                } else {
                                    "Save"
                                }
                                .into(),
                                if item.watchlisted { "remove" } else { "add" },
                                item.id.clone(),
                                0,
                                cx,
                            ),
                        );
                    let column = index % column_count;
                    let existing = std::mem::replace(&mut columns[column], div());
                    columns[column] = existing.child(card);
                }
                content = content.child(div().flex().gap_3().children(columns));
                if page.kind == "episodes" {
                    content = content.child(div().font_semibold().child(semantic_text(
                        "anime-text-15",
                        if privacy {
                            "Anime episodes".to_owned()
                        } else {
                            page.title.clone()
                        },
                    )));
                    for episode in &page.episodes {
                        let label = if privacy {
                            "Play episode".to_owned()
                        } else {
                            format!("{} · {}", episode.number, episode.title)
                        };
                        content = content.child(self.anime_button(
                            format!("episode-{}", episode.id),
                            label,
                            "play",
                            episode.id.clone(),
                            0,
                            cx,
                        ));
                    }
                }
                let action = if page.kind == "calendar" {
                    "calendar-page"
                } else if page.kind == "episodes" {
                    "episodes"
                } else if page.kind == "watchlist" {
                    "watchlist"
                } else {
                    "catalog"
                };
                if page.page > 1 {
                    content = content.child(self.anime_button(
                        "anime-prev".into(),
                        "Previous".into(),
                        action,
                        page.selection.clone(),
                        i32::from(page.page - 1),
                        cx,
                    ));
                }
                if page.page < page.pages {
                    content = content.child(self.anime_button(
                        "anime-next".into(),
                        "Next".into(),
                        action,
                        page.selection.clone(),
                        i32::from(page.page + 1),
                        cx,
                    ));
                }
            }
        } else {
            content = content
                .child(div().child(semantic_text("anime-text-16", "Loading anime catalog…")));
        }
        content = content.child(
            self.anime_button(
                "anime-refresh".into(),
                "Refresh / retry".into(),
                "refresh",
                "".into(),
                self.anime
                    .as_ref()
                    .map_or(1, |page| i32::from(page.page.max(1))),
                cx,
            ),
        );
        content = content.child(self.anime_button(
            "anime-calendar".into(),
            "Release calendar".into(),
            "calendar",
            "".into(),
            0,
            cx,
        ));
        if self.navigation_error {
            content = content.child(ui::error_text(
                "anime-text-17",
                "Action unavailable. Try again.",
                cx,
            ));
        }
        ui::page(cx)
            .child(ui::header(
                self.back_button("anime-back", cx),
                "anime-text-1",
                "Anime",
            ))
            .child(content)
    }
}
