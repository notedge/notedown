use serde::{Deserialize, Serialize};

use crate::{
    asset::Asset,
    block::{Block, BlockNode},
    id::{AssetId, DocumentId, LinkId, NodeId},
    relation::{Relation, RelationEndpoint, RelationKind},
    source::{CoverageReport, SourceRef},
    status::LossMarker,
    transform_errors::TransformError,
};

/// Document-level metadata (title is not document identity).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// Display title (not document identity).
    pub title: Option<String>,
    /// Primary language tag when known.
    pub language: Option<String>,
    /// Document authors in display order.
    pub authors: Vec<String>,
    /// Free-form tags or keywords.
    pub tags: Vec<String>,
}

/// Semantic document graph — import layers construct this, export layers
/// consume it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentGraph {
    /// Stable document identity.
    pub id: DocumentId,
    /// Monotonic revision bumped on semantic edits.
    pub revision: u64,
    /// Display metadata separate from identity.
    pub metadata: DocumentMetadata,
    /// Top-level and nested block nodes.
    pub blocks: Vec<BlockNode>,
    /// Directed semantic relations.
    pub relations: Vec<Relation>,
    /// Registered assets referenced by the document.
    pub assets: Vec<Asset>,
    /// Provenance sidecar records.
    pub sources: Vec<SourceRef>,
    /// Import/export completeness summary.
    pub coverage: CoverageReport,
    /// Internal allocator for auto-assigned node ids.
    #[serde(default)]
    next_node: u64,
}

impl DocumentGraph {
    /// Empty document with the given identity.
    pub fn new(id: DocumentId) -> Self {
        Self {
            id,
            revision: 0,
            metadata: DocumentMetadata { title: None, language: None, authors: Vec::new(), tags: Vec::new() },
            blocks: Vec::new(),
            relations: Vec::new(),
            assets: Vec::new(),
            sources: Vec::new(),
            coverage: CoverageReport { complete: true, loss: Vec::new() },
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

    /// Append a block with an explicit id (import adapters with stable foreign
    /// ids).
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
        let Some(asset) = self.assets.iter_mut().find(|asset| asset.id == id)
        else {
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
        self.relations.iter().filter(|relation| relation.source.matches(endpoint)).collect()
    }

    /// Relations whose target endpoint matches `endpoint`.
    pub fn relations_to(&self, endpoint: &RelationEndpoint) -> Vec<&Relation> {
        self.relations.iter().filter(|relation| relation.target.matches(endpoint)).collect()
    }

    /// Indexed backlink edges pointing at `target`.
    pub fn backlinks_to(&self, target: &RelationEndpoint) -> Vec<&Relation> {
        self.relations.iter().filter(|relation| relation.kind == RelationKind::Backlink && relation.target.matches(target)).collect()
    }
}

// ---------------------------------------------------------------------------
// Mutation API — the write counterpart to push_*
// ---------------------------------------------------------------------------

impl DocumentGraph {
    /// 删除一个 block 节点。
    ///
    /// 行为：
    /// 1. 从 `blocks` Vec 中移除该节点
    /// 2. 从所有 Section / ListItem 的 children 列表中清除对它的引用
    /// 3. relations / sources 中对它的引用**不会**自动清理 ——
    ///    [`validate`] 会把它们报告为 dangling。由调用方决定是否同步清理。
    ///
    /// [`validate`]: DocumentGraph::validate
    pub fn delete_block(&mut self, id: NodeId) -> Result<(), TransformError> {
        let pos = self.blocks.iter().position(|n| n.id == id).ok_or(TransformError::NotFound(id))?;
        self.blocks.remove(pos);

        // 从所有 Section children / ListItem children 中移除引用
        for node in &mut self.blocks {
            match &mut node.block {
                Block::Section { children, .. } => {
                    children.retain(|c| *c != id);
                }
                Block::List { items, .. } => {
                    for item in items {
                        item.children.retain(|c| *c != id);
                    }
                }
                _ => {}
            }
        }

        self.bump_revision();
        Ok(())
    }

    /// 用新 Block 替换指定节点的 payload。NodeId 保持不变（语义身份稳定）。
    pub fn replace_block(&mut self, id: NodeId, new_block: Block) -> Result<(), TransformError> {
        let node = self
            .blocks
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or(TransformError::NotFound(id))?;
        node.block = new_block;
        self.bump_revision();
        Ok(())
    }

    /// 替换可容纳子节点的 block（目前仅 [`Block::Section`]）的 children 列表。
    ///
    /// 调用方必须保证 `new_children` 里的每个 NodeId 都已在图中存在；
    /// 本函数只做最基本的结构合法性判断（parent 能否持 children）。
    pub fn set_children(&mut self, parent_id: NodeId, new_children: Vec<NodeId>) -> Result<(), TransformError> {
        let node = self
            .blocks
            .iter_mut()
            .find(|n| n.id == parent_id)
            .ok_or(TransformError::NotFound(parent_id))?;
        match &mut node.block {
            Block::Section { children, .. } => {
                *children = new_children;
            }
            _ => return Err(TransformError::InvalidParent(parent_id)),
        }
        self.bump_revision();
        Ok(())
    }

    /// 向 Section 的 children 列表追加一个子节点。
    pub fn append_child(&mut self, parent_id: NodeId, child: NodeId) -> Result<(), TransformError> {
        let node = self
            .blocks
            .iter_mut()
            .find(|n| n.id == parent_id)
            .ok_or(TransformError::NotFound(parent_id))?;
        match &mut node.block {
            Block::Section { children, .. } => {
                children.push(child);
            }
            _ => return Err(TransformError::InvalidParent(parent_id)),
        }
        self.bump_revision();
        Ok(())
    }

    /// 从 Section 的 children 列表中移除指定子节点（不删除 block 本身）。
    pub fn remove_child(&mut self, parent_id: NodeId, child: NodeId) -> Result<(), TransformError> {
        let node = self
            .blocks
            .iter_mut()
            .find(|n| n.id == parent_id)
            .ok_or(TransformError::NotFound(parent_id))?;
        match &mut node.block {
            Block::Section { children, .. } => {
                let pos = children.iter().position(|c| *c == child).ok_or(
                    TransformError::ChildNotFound { parent: parent_id, child },
                )?;
                children.remove(pos);
            }
            _ => return Err(TransformError::InvalidParent(parent_id)),
        }
        self.bump_revision();
        Ok(())
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
    /// Allocate the next document id.
    pub fn document_id(&mut self) -> DocumentId {
        self.next_document += 1;
        DocumentId(self.next_document)
    }

    /// Allocate the next node id.
    pub fn node_id(&mut self) -> NodeId {
        self.next_node += 1;
        NodeId(self.next_node)
    }

    /// Allocate the next asset id.
    pub fn asset_id(&mut self) -> AssetId {
        self.next_asset += 1;
        AssetId(self.next_asset)
    }

    /// Allocate the next relation id.
    pub fn link_id(&mut self) -> LinkId {
        self.next_link += 1;
        LinkId(self.next_link)
    }
}
