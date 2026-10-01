use serde::{Deserialize, Serialize};

use crate::id::{AssetId, DocumentId, LinkId, NodeId};

/// Directed semantic relation between endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationKind {
    Contains,
    References,
    Bidirectional,
    Cites,
    Embeds,
    Backlink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationEndpoint {
    Node(NodeId),
    Document(DocumentId),
    Asset(AssetId),
    ExternalUri(String),
    Unresolved { label: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    pub id: LinkId,
    pub kind: RelationKind,
    pub source: RelationEndpoint,
    pub target: RelationEndpoint,
}

impl RelationEndpoint {
    /// Whether this endpoint refers to the same semantic target as `other`.
    pub fn matches(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Node(a), Self::Node(b)) => a == b,
            (Self::Document(a), Self::Document(b)) => a == b,
            (Self::Asset(a), Self::Asset(b)) => a == b,
            (Self::ExternalUri(a), Self::ExternalUri(b)) => a == b,
            (Self::Unresolved { label: a }, Self::Unresolved { label: b }) => a == b,
            _ => false,
        }
    }
}
