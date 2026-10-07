use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub ok: bool,
    pub detail: String,
    pub download_folder: String,
    pub keep_screen_awake: bool,
    pub allow_screenshots: bool,
    pub inspection_privacy: bool,
    pub diagnostics_refresh_seconds: u32,
    pub appearance: String,
    pub background_theme: String,
    pub space_effect_enabled: bool,
    pub reduce_motion: bool,
    pub mobile_download_policy: String,
}

pub fn next_choice<'a>(current: &str, choices: &'a [&'a str]) -> &'a str {
    choices
        .iter()
        .position(|choice| *choice == current)
        .map(|index| choices[(index + 1) % choices.len()])
        .unwrap_or(choices[0])
}
