//! EPUB import via Acorn OCF and `oak-html` XHTML lowering.

mod assets;
pub mod ast_lowering;
pub mod html_extensions;
pub mod navigation;
mod reader;
pub mod xhtml;

pub use reader::{import_epub, import_epub_bytes};

pub(crate) fn map_ocf_error(error: acorn_epub::OcfError) -> crate::FormatError {
    crate::FormatError::parse("epub", error.to_string())
}
