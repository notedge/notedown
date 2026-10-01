use diagnostic_quick::{FileID, Span};

use crate::{NotedownKind, NotedownNode};

/// Convert element into [`NotedownNode`]
pub trait IntoNotedown {
    /// Convert element into [`NotedownNode`] with position
    fn into_node(self, span: &Span, file: &FileID) -> NotedownNode;
}

impl IntoNotedown for NotedownKind {
    #[inline]
    fn into_node(self, span: &Span, file: &FileID) -> NotedownNode {
        NotedownNode { value: self, range: span.clone(), file: file.clone() }
    }
}
