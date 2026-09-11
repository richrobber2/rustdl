//! Local page features. Renderers return HTML; responders handle HTTP.

pub(crate) mod changelog;
pub(crate) mod diagnostics;
pub(crate) mod download_result;
pub(crate) mod gallery;
pub(crate) mod player;
pub(crate) mod settings;

#[cfg(test)]
mod tests;
