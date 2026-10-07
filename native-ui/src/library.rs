use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPage {
    pub ok: bool,
    pub detail: String,
    pub title: String,
    pub location: String,
    #[serde(default)]
    pub revision: u64,
    pub offset: usize,
    pub total: usize,
    pub next_offset: Option<i32>,
    pub previous_offset: Option<i32>,
    pub items: Vec<LibraryItem>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub kind: String,
    pub state: String,
    pub has_thumbnail: bool,
    pub can_share: bool,
    pub can_delete: bool,
    #[serde(default)]
    pub position_seconds: f64,
    #[serde(default)]
    pub duration_seconds: f64,
    #[serde(default)]
    pub watched: bool,
    #[serde(default)]
    pub queued: bool,
    #[serde(default)]
    pub can_source: bool,
}
