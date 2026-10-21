use serde::{Deserialize, Serialize};

use crate::id::{AssetId, DocumentId, LinkId, NodeId};

/// Directed semantic relation between endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationKind {
    /// Structural parent/child containment.
    Contains,
    /// One-way reference from source to target.
    References,
    /// Explicit bidirectional association.
    Bidirectional,
    /// Bibliographic or scholarly citation.
    Cites,
    /// Embedded or included resource.
    Embeds,
    /// Indexed backlink edge pointing at a target.
    Backlink,
}

/// Semantic target of a relation edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationEndpoint {
    /// In-document block node.
    Node(NodeId),
    /// Another document identity.
    Document(DocumentId),
    /// Registered asset identity.
    Asset(AssetId),
    /// External URI outside the document graph.
    ExternalUri(String),
    /// Unresolved label kept as a first-class endpoint.
    Unresolved {
        /// Authored or imported label text.
        label: String,
    },
}

/// Directed relation edge between semantic endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    /// Stable relation identity.
    pub id: LinkId,
    /// Relation semantics.
    pub kind: RelationKind,
    /// Source endpoint.
    pub source: RelationEndpoint,
    /// Target endpoint.
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
