use notedown_ir::DocumentGraph;

use crate::FormatError;

/// Import Markdown bytes into `notedown-ir` via Oak (planned).
pub fn import_markdown_bytes(_label: &str, _text: &str) -> Result<DocumentGraph, FormatError> {
    Err(FormatError::NotImplemented {
        format: "markdown".to_string(),
        direction: "import".to_string(),
    })
}
