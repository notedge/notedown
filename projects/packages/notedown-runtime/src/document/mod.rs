#[cfg(feature = "lsp")]
pub(crate) mod lsp;
pub(crate) mod meta;

pub use self::meta::{
    author::{DocumentAuthor, DocumentAuthorIter},
    class::{DocumentClass, DocumentClassArticle},
    datetime::DocumentTime,
    title::DocumentTitle,
    toc::{TableOfContent, TocConfig, TocNode},
};
use crate::plugin_system::Parser;
use chrono::{NaiveDateTime, Utc};
use notedown_ast::{
    value::{LiteralPair, OrderedMap},
    NotedownNode, NotedownValue,
};
use notedown_error::{QError, Result};
use std::collections::BTreeMap;
use yggdrasil_shared::{TextAdaptor, TextIndex, Url};

#[cfg(feature = "native")]
pub(crate) mod native_wrap {
    pub use crate::VMFileSystem;
    pub use chrono::{DateTime, TimeZone};
    pub use std::{
        fs::read_to_string,
        time::{SystemTime, UNIX_EPOCH},
    };
}
#[cfg(feature = "wasm")]
pub(crate) mod wasm_wrap {}
#[cfg(feature = "native")]
use native_wrap::*;
#[cfg(feature = "wasm")]
use wasm_wrap::*;

#[derive(Debug)]
pub struct NoteDocument {
    url: Url,
    /// used to check weather the file needs re-parse
    fingerprint: u128,
    text: TextIndex,
    ast: NotedownNode,
    variable: OrderedMap,
    errors: Vec<QError>,
    meta: DocumentMeta,
}

#[derive(Debug, Clone)]
pub struct DocumentMeta {
    title: DocumentTitle,
    authors: BTreeMap<String, DocumentAuthor>,
    date: DocumentTime,
    toc: TocNode,
}

impl PartialEq for NoteDocument {
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint.eq(&other.fingerprint)
    }
}

impl NoteDocument {
    #[inline]
    pub fn set_value_raw(&mut self, pair: LiteralPair) -> Option<LiteralPair> {
        self.variable.insert_raw(pair)
    }
    #[inline]
    pub fn set_value(&mut self, name: String, value: NotedownValue) -> Option<NotedownValue> {
        self.variable.insert(name, value)
    }
    #[inline]
    pub fn get_text(&self) -> String {
        self.text.text()
    }

    #[inline]
    pub fn get_text_index(&self) -> &TextIndex {
        &self.text
    }

    #[inline]
    pub fn get_ast(&self) -> &NotedownNode {
        &self.ast
    }

    #[inline]
    pub fn extend_error_one(&mut self, e: QError) {
        self.errors.push(e)
    }
    #[inline]
    pub fn extend_error_iter(&mut self, e: impl Iterator<Item = QError>) {
        self.errors.extend(e.into_iter())
    }
    #[inline]
    pub fn can_gc(&self) -> bool {
        false
    }
}

impl NoteDocument {
    #[inline]
    pub async fn update_text(&mut self) -> Result<()> {
        let full = &Self::load_url(&self.url).await?;
        self.text.apply_change_full(full);
        Ok(())
    }
    #[inline]
    pub async fn update_document(&mut self, parse: &Parser) -> Result<()> {
        let mut errors = vec![];
        let parsed = parse(&self.text.text(), &mut errors)?;
        todo!()
    }
}

impl NoteDocument {
    pub async fn load_url(url: &Url) -> Result<String> {
        #[cfg(feature = "native")]
        match url.scheme() == "file" {
            true => Self::load_local_url(url).await,
            false => Self::load_remote_url_native(url).await,
        }
        #[cfg(feature = "wasm")]
        match url.scheme() == "file" {
            true => Err(QError::runtime_error("Can not load local file from wasm")),
            false => Self::load_remote_url_wasm(url).await,
        }
    }
    #[cfg(feature = "native")]
    #[inline(always)]
    async fn load_local_url(url: &Url) -> Result<String> {
        let path = url.to_file_path()?;
        Ok(read_to_string(path)?)
    }
    #[cfg(feature = "native")]
    #[inline(always)]
    async fn load_remote_url_native(url: &Url) -> Result<String> {
        todo!("Remote: {}", url)
    }
    #[cfg(feature = "wasm")]
    #[inline(always)]
    async fn load_remote_url_wasm(url: &Url) -> Result<String> {
        todo!("Remote: {}", url)
    }
}
