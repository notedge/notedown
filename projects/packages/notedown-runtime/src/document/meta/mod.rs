pub(crate) mod author;
pub(crate) mod class;
pub(crate) mod datetime;
pub(crate) mod title;
pub(crate) mod toc;
use super::*;

impl DocumentMeta {
    #[inline]
    pub fn build_toc(&mut self, node: &NotedownNode) {
        let cfg = TocConfig::default();
        self.toc = node.toc_configurable(&cfg);
    }
}
