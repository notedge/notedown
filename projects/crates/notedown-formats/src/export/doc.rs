use notedown_ir::DocumentGraph;

use crate::FormatError;

/// Legacy binary Word output is intentionally rejected until a complete FIB/CLX writer exists.
pub fn export_doc_bytes(_graph: &DocumentGraph) -> Result<Vec<u8>, FormatError> {
    Err(FormatError::not_implemented("doc", "export"))
}
