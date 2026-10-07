use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueuePage {
    pub ok: bool,
    pub detail: String,
    pub offset: usize,
    pub total: usize,
    pub next_offset: Option<i32>,
    pub previous_offset: Option<i32>,
    #[serde(default)]
    pub network_state: u8,
    pub items: Vec<QueueItem>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub id: String,
    pub title: String,
    pub phase_label: String,
    pub downloaded: u64,
    pub total: u64,
    pub actions: Vec<String>,
    pub can_play: bool,
    pub issue: bool,
    pub issue_detail: Option<String>,
    pub quality: Option<String>,
}
