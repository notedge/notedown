mod diagnostic;
#[cfg(feature = "lsp")]
mod lsp;

pub use notedown_ast::traits::ContextKind;

use crate::{
    plugin_system::{Parser, PluginSystem},
    VMFileSystem,
};
use std::path::Path;
use yggdrasil_shared::Url;

#[derive(Debug)]
pub struct NoteVM {
    fs: VMFileSystem,
    ps: PluginSystem,
}

impl Default for NoteVM {
    fn default() -> Self {
        Self { fs: Default::default(), ps: Default::default() }
    }
}

impl NoteVM {
    pub fn new(root: Url) -> NoteVM {
        Self { fs: VMFileSystem::new(root), ps: Default::default() }
    }

    pub fn run() {}
    #[inline]
    pub async fn load_cache(&mut self, dump: &Path) {
        let _ = dump;
        todo!()
    }
    #[inline]
    pub async fn dump_cache(&self, dump: &Path) {
        let _ = dump;
        todo!()
    }
}

/// Properties that can be obtained immediately
impl NoteVM {
    /// Remove cache that no longer using
    #[inline]
    pub fn gc(&self) {
        self.fs.cache.retain(|_, v| v.can_gc())
    }
    /// Mark some file is useless
    #[inline]
    pub fn gc_mark(&self, _: &Url) {
        // TODO: mark
    }
}

/// Asynchronous operations that take amount of time
impl NoteVM {
    #[inline]
    async fn update_text(&self, url: &Url) -> bool {
        match self.fs.update_text(url).await {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    #[inline]
    async fn update_document(&self, url: &Url, parser: &Parser) -> bool {
        match self.fs.update_ast(url, parser).await {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    pub async fn publish(&self) {}
}
