//! Shared resolved-media and discovery-candidate data.

use super::super::local::gallery::PlaylistMembership;

#[derive(Clone, Debug)]
pub(in super::super) struct ResolvedVideo {
    pub(in super::super) filename: String,
    pub(in super::super) media_url: String,
    pub(in super::super) audio_url: Option<String>,
    pub(in super::super) extract_audio: bool,
    pub(in super::super) quality_label: Option<String>,
    pub(in super::super) quality_height: Option<u32>,
}

#[derive(Clone, Debug)]
pub(in super::super) struct DiscoveryCandidate {
    pub(in super::super) resolved: ResolvedVideo,
    pub(in super::super) qualities: Vec<ResolvedVideo>,
    pub(in super::super) source_url: String,
    pub(in super::super) author: String,
    pub(in super::super) text: String,
    pub(in super::super) playlist: Option<PlaylistMembership>,
}

impl ResolvedVideo {
    pub(in super::super) fn filename(&self) -> String {
        self.filename.clone()
    }
}
