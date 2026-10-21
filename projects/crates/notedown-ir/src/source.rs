use serde::{Deserialize, Serialize};

use crate::{
    id::NodeId,
    status::{LossMarker, SemanticStatus},
};

/// Provenance for a semantic node (zero, one, or many sources per node).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SourceKind {
    /// XML subtree located by package-relative path.
    XmlRange {
        /// Package-relative XML path.
        path: String,
    },
    /// OPC or archive package member path.
    PackageMember {
        /// Member path inside the package.
        path: String,
    },
    /// PDF object reference.
    PdfObject {
        /// PDF object identifier.
        object_id: String,
    },
    /// PDF glyph region on a page.
    GlyphRegion {
        /// One-based page number.
        page: u32,
    },
    /// Source text span in the original text format.
    TextSpan {
        /// Inclusive byte or character start offset.
        start: u32,
        /// Exclusive end offset.
        end: u32,
    },
    /// Synthetic provenance recorded when no precise source exists.
    Synthetic {
        /// Reason the synthetic record was created.
        reason: String,
    },
    /// Manually attached provenance without machine coordinates.
    Manual,
}

/// Provenance record attached to a semantic node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceRef {
    /// Semantic node this provenance describes.
    pub node: NodeId,
    /// Provenance coordinates or synthetic reason.
    pub kind: SourceKind,
    /// Confidence of the provenance mapping.
    pub precision: SemanticStatus,
}

/// Import/export completeness summary for the document graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageReport {
    /// Whether conversion completed without recorded loss.
    pub complete: bool,
    /// Recorded loss markers when conversion was partial.
    pub loss: Vec<LossMarker>,
}
