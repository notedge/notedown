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
