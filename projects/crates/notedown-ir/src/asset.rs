use serde::{Deserialize, Serialize};

use crate::{id::AssetId, status::SemanticStatus};

/// Logical asset category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetKind {
    /// Raster or vector image.
    Image,
    /// Audio clip or stream.
    Audio,
    /// Video clip or stream.
    Video,
    /// Font resource.
    Font,
    /// Generic binary attachment.
    Attachment,
    /// Derived or rendered view materialized during import.
    GeneratedView,
}

/// Logical asset record — identity is separate from storage location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    /// Stable asset identity inside the document graph.
    pub id: AssetId,
    /// Asset category.
    pub kind: AssetKind,
    /// Content hash or other stable identity independent of path.
    pub content_identity: Option<String>,
    /// Current storage location such as a package-relative path or URI.
    pub source: Option<String>,
    /// MIME type when known.
    pub media_type: Option<String>,
    /// Resolution state for this asset record.
    pub status: SemanticStatus,
    /// Embedded payload when the import layer materialized bytes (e.g. OPC part
    /// body).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
}
