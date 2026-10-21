//! DOCX import via Acorn OPC and WordprocessingML projection.

/// WordprocessingML footnote import helpers.
pub mod footnotes;
/// List numbering definition import.
pub mod numbering;
mod reader;
mod rels;
mod table;
/// WordprocessingML body import.
pub mod xml;
/// Shared XML traversal helpers for DOCX import.
pub mod xml_extensions;

pub use reader::{import_docx, import_docx_bytes};

pub(crate) fn map_opc_error(error: acorn_docx::OpcError) -> crate::FormatError {
    crate::FormatError::parse("docx", error.to_string())
}
