use crate::NoteDocument;
use std::ops::Range;

mod visit_ast;

/// Information about table of content
pub trait TableOfContent {
    /// Get table of content from element with config
    fn toc_configurable(&self, config: &TocConfig) -> TocNode;
    /// Get table of content from element
    #[inline]
    fn toc(&self) -> TocNode {
        let cfg = TocConfig::default();
        self.toc_configurable(&cfg)
    }
}

/// Config of table of content
#[derive(Debug, Clone)]
pub struct TocConfig {
    /// Calculate the level from the root, 0 means infinite, and root it self is the first level
    pub max_depth: u8,
}

/// Node of table of content
#[derive(Debug, Clone)]
pub struct TocNode {
    /// Depth of the node from root
    pub level: u8,
    /// Addition information of the node
    pub detail: String,
    /// Range of this node
    pub range: Range<usize>,
    /// Children elements of the node
    pub children: Vec<TocNode>,
}

impl Default for TocConfig {
    fn default() -> Self {
        Self { max_depth: u8::MAX }
    }
}

impl Default for TocNode {
    fn default() -> Self {
        Self { level: 0, detail: String::from("ROOT"), range: Default::default(), children: vec![] }
    }
}

impl NoteDocument {
    #[inline]
    pub fn get_toc(&self) -> &TocNode {
        &self.meta.toc
    }
}
