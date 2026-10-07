use super::*;
use serde::Deserialize;
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Page {
    pub state: String,
    pub detail: String,
    pub version: String,
    pub configured: bool,
    pub can_check: bool,
    pub can_install: bool,
}
pub(super) fn request(action: &str) -> Result<(), String> {
    mobile_jni::with_env(|env| {
        let activity = mobile_jni::activity(env)?;
        let action = env.new_string(action).map_err(|e| e.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("requestNativeUpdates"),
            jni::jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&action)],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}
impl Home {
    fn update_button(
        &self,
        id: &'static str,
        label: &'static str,
        action: &'static str,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .label(label)
            .large()
            .w_full()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.navigation_error = request(action).is_err();
                cx.notify();
            }))
    }
    pub(super) fn render_updates(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let privacy = self.settings.as_ref().is_none_or(|s| s.inspection_privacy);
        let mut content = semantic_scroll("native-updates", &self.scrolls[11])
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
                    .child(semantic_text("updates-text-1", "Updates")),
            );
        if let Some(page) = &self.updates {
            content = content.child(div().child(semantic_text(
                "updates-text-2",
                if privacy {
                    format!("Updates · {}", page.state)
                } else {
                    page.detail.clone()
                },
            )));
            if !privacy && !page.version.is_empty() {
                content = content.child(div().child(semantic_text(
                    "updates-text-3",
                    format!("Available version · {}", page.version),
                )));
            }
            if !page.configured {
                content = content.child(div().child(semantic_text(
                    "updates-text-4",
                    "No update source configured.",
                )));
            }
            if page.can_check {
                content = content.child(self.update_button(
                    "update-check",
                    "Check for updates",
                    "check",
                    cx,
                ));
            }
            if page.can_install {
                content = content.child(self.update_button(
                    "update-install",
                    "Install ready update",
                    "install",
                    cx,
                ));
            }
        } else {
            content = content
                .child(div().child(semantic_text("updates-text-5", "Loading update status…")));
        }
        content =
            content.child(self.update_button("update-refresh", "Refresh status", "snapshot", cx));
        if self.navigation_error {
            content = content.child(div().child(semantic_text(
                "updates-text-6",
                "Action unavailable. Try again.",
            )));
        }
        content.child(
            Button::new("updates-back")
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
