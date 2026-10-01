//! Structural validation for semantic graph invariants.

use crate::block::Block;
use crate::graph::DocumentGraph;
use crate::id::{AssetId, DocumentId, LinkId, NodeId};
use crate::inline::Inline;
use crate::relation::RelationEndpoint;

/// Single invariant violation on the document graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationIssue {
    DuplicateNodeId { id: NodeId },
    DuplicateAssetId { id: AssetId },
    DuplicateLinkId { id: LinkId },
    UnknownNode { id: NodeId },
    UnknownSectionChild { section: NodeId, child: NodeId },
    UnknownInlineReference { from: NodeId, target: NodeId },
    UnknownRelationEndpoint { link: LinkId },
}

/// Collected validation result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

impl DocumentGraph {
    /// Check identity uniqueness, endpoint existence, and local reference integrity.
    pub fn validate(&self) -> ValidationReport {
        let mut report = ValidationReport::default();
        let node_ids = self.collect_node_ids(&mut report);
        let asset_ids = self.collect_asset_ids(&mut report);
        self.check_relations(&node_ids, &asset_ids, &mut report);
        self.check_sources(&node_ids, &mut report);
        self.check_block_refs(&node_ids, &mut report);
        report
    }

    fn collect_node_ids(&self, report: &mut ValidationReport) -> Vec<NodeId> {
        let mut seen = Vec::new();
        for node in &self.blocks {
            if seen.contains(&node.id) {
                report.issues.push(ValidationIssue::DuplicateNodeId { id: node.id });
            } else {
                seen.push(node.id);
            }
        }
        seen
    }

    fn collect_asset_ids(&self, report: &mut ValidationReport) -> Vec<AssetId> {
        let mut seen = Vec::new();
        for asset in &self.assets {
            if seen.contains(&asset.id) {
                report.issues.push(ValidationIssue::DuplicateAssetId { id: asset.id });
            } else {
                seen.push(asset.id);
            }
        }
        seen
    }

    fn check_relations(
        &self,
        node_ids: &[NodeId],
        asset_ids: &[AssetId],
        report: &mut ValidationReport,
    ) {
        let mut seen_links = Vec::new();
        for relation in &self.relations {
            if seen_links.contains(&relation.id) {
                report
                    .issues
                    .push(ValidationIssue::DuplicateLinkId { id: relation.id });
            } else {
                seen_links.push(relation.id);
            }

            if !endpoint_exists(&relation.source, self.id, node_ids, asset_ids)
                || !endpoint_exists(&relation.target, self.id, node_ids, asset_ids)
            {
                report.issues.push(ValidationIssue::UnknownRelationEndpoint {
                    link: relation.id,
                });
            }
        }
    }

    fn check_sources(&self, node_ids: &[NodeId], report: &mut ValidationReport) {
        for source in &self.sources {
            if !node_ids.contains(&source.node) {
                report
                    .issues
                    .push(ValidationIssue::UnknownNode { id: source.node });
            }
        }
    }

    fn check_block_refs(&self, node_ids: &[NodeId], report: &mut ValidationReport) {
        for node in &self.blocks {
            match &node.block {
                Block::Section { title, children, .. } => {
                    check_inline_refs(node.id, title, node_ids, report);
                    for child in children {
                        if !node_ids.contains(child) {
                            report.issues.push(ValidationIssue::UnknownSectionChild {
                                section: node.id,
                                child: *child,
                            });
                        }
                    }
                }
                Block::Paragraph { content } | Block::Quote { content } => {
                    check_inline_refs(node.id, content, node_ids, report);
                }
                Block::List { items, .. } => {
                    for item in items {
                        check_inline_refs(node.id, &item.content, node_ids, report);
                        for child in &item.children {
                            if !node_ids.contains(child) {
                                report.issues.push(ValidationIssue::UnknownSectionChild {
                                    section: node.id,
                                    child: *child,
                                });
                            }
                        }
                    }
                }
                Block::Table { rows } => {
                    for row in rows {
                        for cell in &row.cells {
                            check_inline_refs(node.id, cell, node_ids, report);
                        }
                    }
                }
                Block::Code { .. } | Block::Math { .. } | Block::Opaque { .. } => {}
            }
        }
    }
}

fn endpoint_exists(
    endpoint: &RelationEndpoint,
    document_id: DocumentId,
    node_ids: &[NodeId],
    asset_ids: &[AssetId],
) -> bool {
    match endpoint {
        RelationEndpoint::Node(id) => node_ids.contains(id),
        RelationEndpoint::Document(id) => *id == document_id,
        RelationEndpoint::Asset(id) => asset_ids.contains(id),
        RelationEndpoint::ExternalUri(_) | RelationEndpoint::Unresolved { .. } => true,
    }
}

fn check_inline_refs(from: NodeId, inlines: &[Inline], node_ids: &[NodeId], report: &mut ValidationReport) {
    for inline in inlines {
        match inline {
            Inline::Reference { target, .. } if !node_ids.contains(target) => {
                report.issues.push(ValidationIssue::UnknownInlineReference {
                    from,
                    target: *target,
                });
            }
            Inline::Styled { children, .. } => {
                check_inline_refs(from, children, node_ids, report);
            }
            _ => {}
        }
    }
}
