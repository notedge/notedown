use serde::{Deserialize, Serialize};

/// Explicit semantic or conversion state on IR objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticStatus {
    /// Fully resolved semantics.
    Resolved,
    /// Intentionally unresolved target or value.
    Unresolved,
    /// Partially mapped semantics.
    Partial,
    /// Unsupported source construct retained as loss.
    Unsupported,
    /// Semantics inferred rather than authored.
    Inferred,
    /// Semantics preserved with known loss.
    Lossy,
}

/// Writer-facing loss marker attached during import or export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LossMarker {
    /// Stable loss code for tooling and diagnostics.
    pub code: String,
    /// Human-readable loss explanation.
    pub message: String,
    /// Severity or resolution state of the loss.
    pub status: SemanticStatus,
}
