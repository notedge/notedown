//! Conservative DOCX export from `notedown-ir`.

mod footnotes;
mod package;
mod numbering;
mod rels;
mod writer;

use notedown_ir::DocumentGraph;

use crate::FormatError;

/// Export a document graph to DOCX bytes.
pub fn export_docx_bytes(graph: &DocumentGraph) -> Result<Vec<u8>, FormatError> {
    let rendered = writer::render_document_xml(graph)?;
    Ok(package::build_minimal_opc_package(
        &rendered.document_xml,
        rendered.include_numbering,
        &rendered.document_rels_xml,
        &rendered.media_parts,
        &rendered.image_extensions,
        rendered.footnotes_xml.as_deref(),
    ))
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
