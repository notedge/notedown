use serde::{Deserialize, Serialize};

use crate::asset::Asset;
use crate::block::{Block, BlockNode};
use crate::id::{AssetId, DocumentId, LinkId, NodeId};
use crate::relation::{Relation, RelationEndpoint, RelationKind};
use crate::source::{CoverageReport, SourceRef};
use crate::status::LossMarker;

/// Document-level metadata (title is not document identity).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub title: Option<String>,
    pub language: Option<String>,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
}

/// Semantic document graph — Panduck readers construct this, writers consume it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentGraph {
    pub id: DocumentId,
    pub metadata: DocumentMetadata,
    pub blocks: Vec<BlockNode>,
    pub relations: Vec<Relation>,
    pub assets: Vec<Asset>,
    pub sources: Vec<SourceRef>,
    pub coverage: CoverageReport,
}

impl DocumentGraph {
    /// Empty document with the given identity.
    pub fn new(id: DocumentId) -> Self {
        Self {
            id,
            metadata: DocumentMetadata {
                title: None,
                language: None,
                authors: Vec::new(),
                tags: Vec::new(),
            },
            blocks: Vec::new(),
            relations: Vec::new(),
            assets: Vec::new(),
            sources: Vec::new(),
            coverage: CoverageReport {
                complete: true,
                loss: Vec::new(),
            },
        }
    }

    /// Append a block and return its semantic node id.
    pub fn push_block(&mut self, block: Block) -> NodeId {
        let id = NodeId(self.blocks.len() as u64 + 1);
        self.blocks.push(BlockNode { id, block });
        id
    }

    /// Record import/export loss without mutating content.
    pub fn push_loss(&mut self, marker: LossMarker) {
        self.coverage.complete = false;
        self.coverage.loss.push(marker);
    }

    /// Append a relation edge.
    pub fn push_relation(&mut self, relation: Relation) {
        self.relations.push(relation);
    }

    /// Register an asset and return its id.
    pub fn push_asset(&mut self, asset: Asset) -> AssetId {
        let id = asset.id;
        self.assets.push(asset);
        id
    }

    /// Attach provenance to a semantic node.
    pub fn attach_source(&mut self, source: SourceRef) {
        self.sources.push(source);
    }

    /// Look up a block by semantic node id.
    pub fn block(&self, id: NodeId) -> Option<&BlockNode> {
        self.blocks.iter().find(|node| node.id == id)
    }

    /// Provenance records attached to a semantic node.
    pub fn sources_for(&self, node: NodeId) -> Vec<&SourceRef> {
        self.sources.iter().filter(|source| source.node == node).collect()
    }

    /// Relations whose source endpoint matches `endpoint`.
    pub fn relations_from(&self, endpoint: &RelationEndpoint) -> Vec<&Relation> {
        self.relations
            .iter()
            .filter(|relation| relation.source.matches(endpoint))
            .collect()
    }

    /// Relations whose target endpoint matches `endpoint`.
    pub fn relations_to(&self, endpoint: &RelationEndpoint) -> Vec<&Relation> {
        self.relations
            .iter()
            .filter(|relation| relation.target.matches(endpoint))
            .collect()
    }

    /// Indexed backlink edges pointing at `target`.
    pub fn backlinks_to(&self, target: &RelationEndpoint) -> Vec<&Relation> {
        self.relations
            .iter()
            .filter(|relation| {
                relation.kind == RelationKind::Backlink && relation.target.matches(target)
            })
            .collect()
    }
}

/// Monotonic id allocator for a conversion session.
#[derive(Debug, Default, Clone, Copy)]
pub struct IdAllocator {
    next_document: u64,
    next_node: u64,
    next_asset: u64,
    next_link: u64,
}

impl IdAllocator {
    pub fn document_id(&mut self) -> DocumentId {
        self.next_document += 1;
        DocumentId(self.next_document)
    }

    pub fn node_id(&mut self) -> NodeId {
        self.next_node += 1;
        NodeId(self.next_node)
    }

    pub fn asset_id(&mut self) -> AssetId {
        self.next_asset += 1;
        AssetId(self.next_asset)
    }

    pub fn link_id(&mut self) -> LinkId {
        self.next_link += 1;
        LinkId(self.next_link)
    }
}
