use serde::{Deserialize, Serialize};

use crate::id::AssetId;
use crate::status::SemanticStatus;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetKind {
    Image,
    Audio,
    Video,
    Font,
    Attachment,
    GeneratedView,
}

/// Logical asset record — identity is separate from storage location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub id: AssetId,
    pub kind: AssetKind,
    pub content_identity: Option<String>,
    pub source: Option<String>,
    pub media_type: Option<String>,
    pub status: SemanticStatus,
}
