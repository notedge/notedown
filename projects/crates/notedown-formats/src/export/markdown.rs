use notedown_ir::DocumentGraph;

use crate::FormatError;

/// Export `notedown-ir` to Markdown text (planned).
pub fn export_markdown(_graph: &DocumentGraph) -> Result<String, FormatError> {
    Err(FormatError::NotImplemented {
        format: "markdown".to_string(),
        direction: "export".to_string(),
    })
}
