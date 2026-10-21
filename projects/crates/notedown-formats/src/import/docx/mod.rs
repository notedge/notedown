//! DOCX import via Acorn OPC and WordprocessingML projection.

pub mod footnotes;
pub mod numbering;
mod reader;
mod rels;
mod table;
pub mod xml;
pub mod xml_extensions;

pub use reader::{import_docx, import_docx_bytes};

pub(crate) fn map_opc_error(error: acorn_docx::OpcError) -> crate::FormatError {
    crate::FormatError::parse("docx", error.to_string())
}
