use serde::{Deserialize, Serialize};

/// Stable document identity (not a filesystem path).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(
    /// Opaque numeric identity.
    pub u64,
);

/// Semantic content node identity inside a document graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(
    /// Opaque numeric identity.
    pub u64,
);

/// Logical asset identity (not a URL or filesystem path).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetId(
    /// Opaque numeric identity.
    pub u64,
);

/// Directed relation edge identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LinkId(
    /// Opaque numeric identity.
    pub u64,
);
