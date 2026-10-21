use serde::{Deserialize, Serialize};

/// Explicit semantic or conversion state on IR objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticStatus {
    Resolved,
    Unresolved,
    Partial,
    Unsupported,
    Inferred,
    Lossy,
}

/// Writer-facing loss marker attached during import or export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LossMarker {
    pub code: String,
    pub message: String,
    pub status: SemanticStatus,
}
