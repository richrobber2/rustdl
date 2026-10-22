//! Decode playlist sections without retaining the response's full JSON tree.
use super::youtube::{self, YouTubePlaylistEntry};
use serde::Deserialize;
use serde::de::{self, SeqAccess, Visitor};
use serde_json::Value;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;

pub(in super::super) struct Page {
    pub title: Option<String>,
    pub entries: Vec<YouTubePlaylistEntry>,
    pub continuation: Option<String>,
}

#[derive(Default)]
struct Items {
    entries: Vec<YouTubePlaylistEntry>,
    continuation: Option<String>,
}

impl<'de> Deserialize<'de> for Items {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ItemsVisitor;
        impl<'de> Visitor<'de> for ItemsVisitor {
            type Value = Items;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("playlist items")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Items, A::Error> {
                let mut result = Items::default();
                while let Some(mut item) = seq.next_element::<Value>()? {
                    // Older desktop pages wrap the list in a renderer. Current
                    // lockup items are parsed one at a time and immediately dropped.
                    if let Some(contents) = item.pointer_mut("/playlistVideoListRenderer/contents")
                    {
                        if let Some(children) = contents.as_array_mut() {
                            for child in children.drain(..) {
                                result.push(&child).map_err(de::Error::custom)?;
                            }
                        }
                    } else {
                        result.push(&item).map_err(de::Error::custom)?;
                    }
                }
                Ok(result)
            }
        }
        deserializer.deserialize_seq(ItemsVisitor)
    }
}

impl Items {
    fn push(&mut self, item: &Value) -> Result<(), &'static str> {
        if let Some(entry) = youtube::playlist_entry(item) {
            if self.entries.len() >= 5_000 {
                return Err("playlist page exceeds 5000 entries");
            }
            self.entries.push(entry);
        }
        if let Some(token) = youtube::playlist_continuation_token(item) {
            self.continuation = Some(token);
        }
        Ok(())
    }
    fn finish(mut self, title: Option<String>) -> Page {
        let mut seen = HashSet::new();
        self.entries
            .retain(|entry| seen.insert(entry.video_id.clone()));
        Page {
            title,
            entries: self.entries,
            continuation: self.continuation,
        }
    }
    fn extend(&mut self, other: Self) -> Result<(), &'static str> {
        if self.entries.len().saturating_add(other.entries.len()) > 5_000 {
            return Err("playlist page exceeds 5000 entries");
        }
        self.entries.extend(other.entries);
        self.continuation = other.continuation.or(self.continuation.take());
        Ok(())
    }
}

#[derive(Deserialize, Default)]
struct Initial {
    #[serde(default)]
    contents: Option<Contents>,
    #[serde(default)]
    metadata: Value,
    #[serde(default)]
    header: Value,
}
#[derive(Deserialize, Default)]
struct Contents {
    #[serde(default, rename = "twoColumnBrowseResultsRenderer")]
    desktop: Option<Browse>,
    #[serde(default, rename = "singleColumnBrowseResultsRenderer")]
    mobile: Option<Browse>,
}
#[derive(Deserialize, Default)]
struct Browse {
    #[serde(default)]
    tabs: Vec<Tab>,
}
#[derive(Deserialize, Default)]
struct Tab {
    #[serde(default, rename = "tabRenderer")]
    renderer: Option<TabRenderer>,
}
#[derive(Deserialize, Default)]
struct TabRenderer {
    #[serde(default)]
    content: Option<Content>,
}
#[derive(Deserialize, Default)]
struct Content {
    #[serde(default, rename = "sectionListRenderer")]
    sections: Option<Sections>,
}
#[derive(Deserialize, Default)]
struct Sections {
    #[serde(default)]
    contents: Vec<Section>,
}
#[derive(Deserialize, Default)]
struct Section {
    #[serde(default, rename = "itemSectionRenderer")]
    items: Option<ItemSection>,
}
#[derive(Deserialize, Default)]
struct ItemSection {
    #[serde(default)]
    contents: Items,
}

pub(in super::super) fn initial(json: &str) -> Result<Page, Box<dyn Error>> {
    let data: Initial = serde_json::from_str(json)?;
    let title = youtube::playlist_title(
        &serde_json::json!({"metadata":data.metadata,"header":data.header}),
    );
    let contents = data
        .contents
        .ok_or("YouTube playlist tabs were not found")?;
    if contents.desktop.is_none() && contents.mobile.is_none() {
        return Err("YouTube playlist tabs were not found".into());
    }
    let mut result = Items::default();
    for browse in contents.desktop.into_iter().chain(contents.mobile) {
        for tab in browse.tabs {
            if let Some(sections) = tab
                .renderer
                .and_then(|r| r.content)
                .and_then(|c| c.sections)
            {
                for section in sections.contents {
                    if let Some(items) = section.items {
                        result.extend(items.contents)?;
                    }
                }
            }
        }
    }
    Ok(result.finish(title))
}

#[derive(Deserialize, Default)]
struct Continuation {
    #[serde(default, rename = "onResponseReceivedActions")]
    actions: Vec<Action>,
    #[serde(default, rename = "onResponseReceivedEndpoints")]
    endpoints: Vec<Action>,
}
#[derive(Deserialize, Default)]
struct Action {
    #[serde(default, rename = "appendContinuationItemsAction")]
    append: Option<ContinuationItems>,
    #[serde(default, rename = "reloadContinuationItemsCommand")]
    reload: Option<ContinuationItems>,
}
#[derive(Deserialize, Default)]
struct ContinuationItems {
    #[serde(default, rename = "continuationItems")]
    items: Items,
}

pub(in super::super) fn continuation(json: &str) -> Result<Page, Box<dyn Error>> {
    let data: Continuation = serde_json::from_str(json)?;
    if data.actions.is_empty() && data.endpoints.is_empty() {
        return Err("YouTube playlist continuation actions were not found".into());
    }
    let mut result = Items::default();
    for action in data.actions.into_iter().chain(data.endpoints) {
        if let Some(items) = action.append.or(action.reload) {
            result.extend(items.items)?;
        }
    }
    Ok(result.finish(None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scoped_streaming_parser_handles_desktop_mobile_and_old_lists() {
        for layout in [
            "twoColumnBrowseResultsRenderer",
            "singleColumnBrowseResultsRenderer",
        ] {
            let data = json!({
                "metadata":{"playlistMetadataRenderer":{"title":"Playlist & 雪"}},
                "contents":{layout:{"tabs":[{"tabRenderer":{"content":{"sectionListRenderer":{"contents":[
                    {"otherRenderer":{"contents":[{"lockupViewModel":{"contentId":"XXXXXXXXXXX"}}]}},
                    {"itemSectionRenderer":{"contents":[
                        {"lockupViewModel":{"contentId":"AAAAAAAAAAA","metadata":{"lockupMetadataViewModel":{"title":{"content":"A"}}}}},
                        {"playlistVideoListRenderer":{"contents":[
                            {"playlistVideoRenderer":{"videoId":"BBBBBBBBBBB","title":{"simpleText":"B"}}},
                            {"playlistVideoRenderer":{"videoId":"AAAAAAAAAAA"}}
                        ]}},
                        {"continuationItemRenderer":{"continuationEndpoint":{"continuationCommand":{"token":"next"}}}}
                    ]}}
                ]}}}}]}},
                "irrelevant":{"payload":"ignored".repeat(100_000)}
            });
            let page = initial(&data.to_string()).unwrap();
            assert_eq!(page.title.as_deref(), Some("Playlist & 雪"));
            assert_eq!(
                page.entries
                    .iter()
                    .map(|e| e.video_id.as_str())
                    .collect::<Vec<_>>(),
                ["AAAAAAAAAAA", "BBBBBBBBBBB"]
            );
            assert_eq!(page.continuation.as_deref(), Some("next"));
        }
    }

    #[test]
    fn escaped_initial_data_is_decoded_without_evaluating_scripts() {
        let html = r#"<script>var ytInitialData='\x7b\x22header\x22:{\x22pageHeaderRenderer\x22:{\x22pageTitle\x22:\x22Snow \u96ea \ud83e\udd80\x22}}}';window.bad();</script>"#;
        let decoded = youtube::initial_data(html).unwrap();
        let value: Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(
            youtube::playlist_title(&value).as_deref(),
            Some("Snow 雪 🦀")
        );
        assert!(matches!(
            youtube::initial_data("var ytInitialData={\"a\":1};").unwrap(),
            std::borrow::Cow::Borrowed(_)
        ));
        for html in [
            r#"var ytInitialData='\xZZ';"#,
            r#"var ytInitialData='\uD800';"#,
            "var ytInitialData='unterminated",
        ] {
            assert!(youtube::initial_data(html).is_err());
        }
    }

    #[test]
    fn continuation_accepts_both_action_shapes_and_deduplicates() {
        let data = json!({"onResponseReceivedEndpoints":[{"reloadContinuationItemsCommand":{"continuationItems":[
            {"playlistVideoRenderer":{"videoId":"AAAAAAAAAAA"}},
            {"playlistVideoRenderer":{"videoId":"AAAAAAAAAAA"}},
            {"continuationItemViewModel":{"continuationCommand":{"innertubeCommand":{"continuationCommand":{"token":"more"}}}}}
        ]}}]});
        let page = continuation(&data.to_string()).unwrap();
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.continuation.as_deref(), Some("more"));
        assert!(continuation("{}").is_err());
    }
}
