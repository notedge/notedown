use super::*;

use yggdrasil_shared::lsp_types::{Diagnostic, DocumentSymbolResponse, Position, TextDocumentContentChangeEvent};

impl NoteVM {
    #[inline]
    pub fn get_lsp_toc(&self, url: &Url) -> Option<DocumentSymbolResponse> {
        let toc = match self.fs.cache.get(url) {
            None => return None,
            Some(s) => s.get_lsp_toc(),
        };
        Some(DocumentSymbolResponse::Nested(vec![toc]))
    }
}

/// Asynchronous operations that take amount of time
impl NoteVM {
    #[inline]
    pub async fn update(&self, url: &Url) -> Vec<Diagnostic> {
        self.update_text(url).await;
        match self.ps.get_parser("note") {
            None => {}
            Some(parser) => {
                self.update_document(url, &parser).await;
            }
        }
        todo!()
    }

    pub fn get_completion_context(&self, url: &Url, p: &Position) -> ContextKind {
        let _ = (url, p);
        todo!()
    }

    #[inline]
    pub async fn update_increment(&self, url: &Url, edits: Vec<TextDocumentContentChangeEvent>) -> Vec<Diagnostic> {
        let _ = (url, edits);
        todo!()
    }
}
