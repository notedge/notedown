//! DOCX import via Acorn OPC and WordprocessingML projection.

mod footnotes;
mod numbering;
mod oak_xml_util;
mod reader;
mod rels;
mod table;
mod xml;

pub use reader::{import_docx, import_docx_bytes};

pub(crate) fn map_opc_error(error: acorn_docx::OpcError) -> crate::FormatError {
    crate::FormatError::parse("docx", error.to_string())
}
