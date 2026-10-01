use super::*;
use yggdrasil_shared::{
    lsp_types::{Diagnostic, DocumentSymbol, DocumentSymbolResponse, SymbolKind},
    LspRange, LspTextAdaptor,
};

impl TocNode {
    #[inline]
    pub fn get_lsp_range(&self, text: &TextIndex) -> LspRange {
        text.offset_range_to_lsp_range(&self.range).unwrap_or_default()
    }
    /// Get table of content from element in lsp form
    #[inline]
    #[allow(deprecated)]
    pub fn as_document_symbol(&self, text: &TextIndex) -> DocumentSymbol {
        DocumentSymbol { name: "".to_string(), detail: Some(self.detail.to_owned()), kind: SymbolKind::NAMESPACE, tags: None, deprecated: None, range: self.get_lsp_range(text), selection_range: self.get_lsp_range(text), children: None }
    }
}

impl NoteDocument {
    #[inline]
    pub fn as_lsp_diagnostics(&self, index: &TextIndex) -> Vec<Diagnostic> {
        self.errors.iter().map(|f| f.as_lsp_diagnostic(index)).collect()
    }
    #[inline]
    pub fn get_lsp_toc(&self) -> DocumentSymbol {
        self.meta.toc.as_document_symbol(&self.text)
    }
}
