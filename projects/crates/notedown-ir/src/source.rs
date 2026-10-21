use serde::{Deserialize, Serialize};

use crate::id::NodeId;
use crate::status::{LossMarker, SemanticStatus};

/// Provenance for a semantic node (zero, one, or many sources per node).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SourceKind {
    XmlRange { path: String },
    PackageMember { path: String },
    PdfObject { object_id: String },
    GlyphRegion { page: u32 },
    TextSpan { start: u32, end: u32 },
    Synthetic { reason: String },
    Manual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceRef {
    pub node: NodeId,
    pub kind: SourceKind,
    pub precision: SemanticStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageReport {
    pub complete: bool,
    pub loss: Vec<LossMarker>,
}
