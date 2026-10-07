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
        let mut content = ui::body("native-updates", &self.scrolls[Screen::Updates as usize]);
        if let Some(page) = &self.updates {
            let mut status = ui::card(cx).child(ui::card_title(
                "updates-text-2",
                if privacy {
                    format!("Updates · {}", page.state)
                } else {
                    page.detail.clone()
                },
            ));
            if !privacy && !page.version.is_empty() {
                status = status.child(div().child(semantic_text(
                    "updates-text-3",
                    format!("Available version · {}", page.version),
                )));
            }
            if !page.configured {
                status = status.child(ui::muted_text(
                    "updates-text-4",
                    "No update source configured.",
                    cx,
                ));
            }
            if page.can_install {
                status = status.child(
                    self.update_button("update-install", "Install ready update", "install", cx)
                        .primary(),
                );
            }
            if page.can_check {
                status = status.child(self.update_button(
                    "update-check",
                    "Check for updates",
                    "check",
                    cx,
                ));
            }
            content = content.child(status);
        } else {
            content = content.child(ui::card(cx).child(ui::muted_text(
                "updates-text-5",
                "Loading update status…",
                cx,
            )));
        }
        content =
            content.child(self.update_button("update-refresh", "Refresh status", "snapshot", cx));
        if self.navigation_error {
            content = content.child(ui::error_text(
                "updates-text-6",
                "Action unavailable. Try again.",
                cx,
            ));
        }
        ui::page(cx)
            .child(ui::header(
                self.back_button("updates-back", cx),
                "updates-text-1",
                "Updates",
            ))
            .child(content)
    }
}
