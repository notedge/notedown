use notedown_ir::DocumentGraph;

use crate::FormatError;

/// Import DOCX bytes into `notedown-ir` via Acorn and Oak (planned).
pub fn import_docx_bytes(_label: &str, _bytes: &[u8]) -> Result<DocumentGraph, FormatError> {
    Err(FormatError::NotImplemented {
        format: "docx".to_string(),
        direction: "import".to_string(),
    })
}
