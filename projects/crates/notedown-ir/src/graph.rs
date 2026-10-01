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

/// Semantic document graph — import layers construct this, export layers consume it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentGraph {
    pub id: DocumentId,
    pub revision: u64,
    pub metadata: DocumentMetadata,
    pub blocks: Vec<BlockNode>,
    pub relations: Vec<Relation>,
    pub assets: Vec<Asset>,
    pub sources: Vec<SourceRef>,
    pub coverage: CoverageReport,
    #[serde(default)]
    next_node: u64,
}

impl DocumentGraph {
    /// Empty document with the given identity.
    pub fn new(id: DocumentId) -> Self {
        Self {
            id,
            revision: 0,
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
            next_node: 0,
        }
    }

    /// Bump the document revision after a semantic edit.
    pub fn bump_revision(&mut self) {
        self.revision += 1;
    }

    /// Replace display metadata without changing document identity.
    pub fn set_metadata(&mut self, metadata: DocumentMetadata) {
        self.metadata = metadata;
        self.bump_revision();
    }

    /// Append a block and return its semantic node id.
    pub fn push_block(&mut self, block: Block) -> NodeId {
        self.next_node += 1;
        let id = NodeId(self.next_node);
        self.blocks.push(BlockNode { id, block });
        id
    }

    /// Append a block with an explicit id (import adapters with stable foreign ids).
    pub fn push_block_with_id(&mut self, id: NodeId, block: Block) {
        if id.0 > self.next_node {
            self.next_node = id.0;
        }
        self.blocks.push(BlockNode { id, block });
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

    /// Update asset storage location while preserving logical identity.
    pub fn set_asset_source(&mut self, id: AssetId, source: Option<String>) -> bool {
        let Some(asset) = self.assets.iter_mut().find(|asset| asset.id == id) else {
            return false;
        };
        asset.source = source;
        self.bump_revision();
        true
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
