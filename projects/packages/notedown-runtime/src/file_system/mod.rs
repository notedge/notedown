pub(crate) mod get_env;
pub(crate) mod get_git;

use crate::{plugin_system::Parser, NoteDocument};

use dashmap::DashMap;
use notedown_error::{QError, Result};
use std::{
    env::VarError,
    path::{Path, PathBuf},
};
use yggdrasil_shared::Url;

#[cfg(feature = "native")]
pub(crate) mod native_wrap {
    pub use async_std::{fs::File, io::ReadExt};
    pub use notedown_error::{
        git2::Repository,
        globset::{Glob, GlobSet, GlobSetBuilder},
    };
}
#[cfg(feature = "wasm")]
pub(crate) mod wasm_wrap {}
#[cfg(feature = "native")]
use native_wrap::*;
#[cfg(feature = "wasm")]
use wasm_wrap::*;

#[derive(Debug)]
pub struct VMFileSystem {
    pub workspace_root: Option<Url>,
    pub cache: DashMap<Url, NoteDocument>,
}

impl Default for VMFileSystem {
    fn default() -> Self {
        Self { workspace_root: None, cache: Default::default() }
    }
}

impl VMFileSystem {
    #[inline]
    pub fn new(url: Url) -> VMFileSystem {
        Self { workspace_root: Some(url), cache: Default::default() }
    }
    #[inline]
    pub fn set_workspace(&mut self, url: Url) {
        self.workspace_root = Some(url);
    }
    #[inline]
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

impl VMFileSystem {}

impl VMFileSystem {
    #[inline]
    pub async fn update_text(&self, url: &Url) -> Result<()> {
        match self.cache.get_mut(url) {
            None => {
                todo!()
            }
            Some(mut s) => s.value_mut().update_text().await,
        }
    }
    #[inline]
    pub async fn update_ast(&self, url: &Url, parser: &Parser) -> Result<()> {
        match self.cache.get_mut(&url) {
            None => Err(QError::runtime_error("TODO")),
            Some(mut s) => s.value_mut().update_document(parser).await,
        }
    }
}

#[cfg(feature = "native")]
impl VMFileSystem {
    #[inline]
    /// add a file url to resolve
    pub async fn load_local_url(&mut self, url: &Url) -> Result<()> {
        let mut file = File::open(url.to_file_path()?).await?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).await?;

        todo!()
    }
    /// add a local file path to resolve
    #[inline]
    pub async fn load_path(&mut self, path: &Path) -> Result<()> {
        let mut file = File::open(path).await?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).await?;
        let _ = Url::from_file_path(path)?;
        todo!()
    }

    #[inline]
    pub async fn load_pattern_text(&mut self, patterns: &str) -> Result<()> {
        let mut builder = GlobSetBuilder::new();
        for row in patterns.lines() {
            builder.add(Glob::new(row)?);
        }
        Ok(self.load_pattern(&builder.build()?).await)
    }

    #[inline]
    pub async fn load_pattern(&mut self, patterns: &GlobSet) {
        let _ = patterns;
        todo!()
    }
}
