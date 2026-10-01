//! Conservative DOCX export from `notedown-ir`.

mod package;
mod numbering;
mod writer;

use notedown_ir::DocumentGraph;

use crate::FormatError;

/// Export a document graph to DOCX bytes.
pub fn export_docx_bytes(graph: &DocumentGraph) -> Result<Vec<u8>, FormatError> {
    let (document_xml, include_numbering) = writer::render_document_xml(graph)?;
    Ok(package::build_minimal_opc_package(&document_xml, include_numbering))
}

/// Export a document graph to DOCX bytes on disk.
pub fn export_docx(graph: &DocumentGraph, path: impl AsRef<std::path::Path>) -> Result<(), FormatError> {
    let bytes = export_docx_bytes(graph)?;
    std::fs::write(path.as_ref(), bytes).map_err(|error| {
        FormatError::invalid_input(format!(
            "failed to write {}: {error}",
            path.as_ref().display()
        ))
    })
}
