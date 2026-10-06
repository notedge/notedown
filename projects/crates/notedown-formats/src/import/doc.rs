use acorn_doc::extract_text_from_bytes;

use notedown_ir::{Block, DocumentGraph, DocumentId, Inline, LossMarker, SemanticStatus};

use crate::FormatError;

/// Import extractable text from a legacy OLE Word document.
pub fn import_doc_bytes(label: &str, bytes: &[u8]) -> Result<DocumentGraph, FormatError> {
    let extracted = extract_text_from_bytes(bytes).map_err(|error| match error {
        acorn_doc::DocError::NotCompoundFile => FormatError::invalid_input("input is not an OLE Compound File"),
        acorn_doc::DocError::Encrypted => FormatError::unsupported("doc", "encrypted or obfuscated Word documents"),
        acorn_doc::DocError::InvalidFib => FormatError::invalid_input("invalid Word FIB or text range"),
        other => FormatError::parse("doc", other.to_string()),
    })?;
    let mut graph = DocumentGraph::new(DocumentId(1));
    for paragraph in split_doc_paragraphs(&extracted.text) {
        graph.push_block(Block::Paragraph {
            content: vec![Inline::Text {
                text: paragraph.to_string(),
            }],
        });
    }
    graph.push_loss(LossMarker {
        code: "import.doc.partial_coverage".into(),
        message: format!(
            "{label}: extracted WordDocument text only. OLE layout, fields, revisions, tables, drawings, and embedded objects are not represented"
        ),
        status: SemanticStatus::Partial,
    });
    if extracted.complex && !extracted.piece_table_used {
        graph.push_loss(LossMarker {
            code: "import.doc.complex_piece_table_unresolved".into(),
            message: "The Word FIB marks this document as complex. Text was limited to the simple FIB range because CLX piece-table reconstruction is not available yet".into(),
            status: SemanticStatus::Partial,
        });
    }
    Ok(graph)
}

fn split_doc_paragraphs(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n').filter(|paragraph| !paragraph.is_empty())
}

/// Import a legacy `.doc` file from disk.
pub fn import_doc(path: impl AsRef<std::path::Path>) -> Result<DocumentGraph, FormatError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|error| {
        FormatError::invalid_input(format!("failed to read {}: {error}", path.display()))
    })?;
    import_doc_bytes(&path.display().to_string(), &bytes)
}

#[cfg(test)]
mod tests {
    use super::split_doc_paragraphs;

    #[test]
    fn preserves_leading_and_trailing_spaces_inside_doc_paragraphs() {
        let paragraphs = split_doc_paragraphs("  leading\ttext  \nnext  ").collect::<Vec<_>>();
        assert_eq!(paragraphs, ["  leading\ttext  ", "next  "]);
    }
}
