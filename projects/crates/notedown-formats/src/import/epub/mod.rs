//! EPUB import via Acorn OCF and thin XHTML projection.

mod reader;
mod xhtml;

pub use reader::{import_epub, import_epub_bytes};

pub(crate) fn map_ocf_error(error: acorn_epub::OcfError) -> crate::FormatError {
    crate::FormatError::parse("epub", error.to_string())
}
