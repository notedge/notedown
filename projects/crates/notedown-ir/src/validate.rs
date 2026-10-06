//! Structural validation for semantic graph invariants.

use crate::block::Block;
use crate::graph::DocumentGraph;
use crate::id::{AssetId, DocumentId, LinkId, NodeId};
use crate::inline::Inline;
use crate::relation::{RelationEndpoint, RelationKind};
use std::collections::{HashMap, HashSet};

/// Single invariant violation on the document graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationIssue {
    DuplicateNodeId { id: NodeId },
    DuplicateAssetId { id: AssetId },
    DuplicateLinkId { id: LinkId },
    UnknownNode { id: NodeId },
    UnknownSectionChild { section: NodeId, child: NodeId },
    UnknownListChild { list: NodeId, child: NodeId },
    UnknownInlineReference { from: NodeId, target: NodeId },
    UnknownRelationEndpoint { link: LinkId },
    ContainmentCycle { from: NodeId, to: NodeId },
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
        self.check_containment_cycles(&node_ids, &mut report);
        report
    }

    fn collect_node_ids(&self, report: &mut ValidationReport) -> HashSet<NodeId> {
        let mut seen = HashSet::new();
        for node in &self.blocks {
            if !seen.insert(node.id) {
                report.issues.push(ValidationIssue::DuplicateNodeId { id: node.id });
            }
        }
        seen
    }

    fn collect_asset_ids(&self, report: &mut ValidationReport) -> HashSet<AssetId> {
        let mut seen = HashSet::new();
        for asset in &self.assets {
            if !seen.insert(asset.id) {
                report.issues.push(ValidationIssue::DuplicateAssetId { id: asset.id });
            }
        }
        seen
    }

    fn check_relations(
        &self,
        node_ids: &HashSet<NodeId>,
        asset_ids: &HashSet<AssetId>,
        report: &mut ValidationReport,
    ) {
        let mut seen_links = HashSet::new();
        for relation in &self.relations {
            if !seen_links.insert(relation.id) {
                report
                    .issues
                    .push(ValidationIssue::DuplicateLinkId { id: relation.id });
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

    fn check_sources(&self, node_ids: &HashSet<NodeId>, report: &mut ValidationReport) {
        for source in &self.sources {
            if !node_ids.contains(&source.node) {
                report
                    .issues
                    .push(ValidationIssue::UnknownNode { id: source.node });
            }
        }
    }

    fn check_block_refs(&self, node_ids: &HashSet<NodeId>, report: &mut ValidationReport) {
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
                                report.issues.push(ValidationIssue::UnknownListChild {
                                    list: node.id,
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

    fn check_containment_cycles(&self, node_ids: &HashSet<NodeId>, report: &mut ValidationReport) {
        let mut children_by_node: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        for node in &self.blocks {
            let children = children_by_node.entry(node.id).or_default();
            match &node.block {
                Block::Section { children: nested, .. } => children.extend(nested.iter().copied()),
                Block::List { items, .. } => {
                    children.extend(items.iter().flat_map(|item| item.children.iter().copied()));
                }
                _ => {}
            }
        }
        for relation in &self.relations {
            if relation.kind == RelationKind::Contains {
                if let (RelationEndpoint::Node(parent), RelationEndpoint::Node(child)) =
                    (&relation.source, &relation.target)
                {
                    children_by_node.entry(*parent).or_default().push(*child);
                }
            }
        }

        let mut state = HashMap::<NodeId, u8>::new();
        let mut stack = Vec::<(NodeId, usize)>::new();
        for node in &self.blocks {
            if state.get(&node.id).copied().unwrap_or(0) != 0 {
                continue;
            }
            state.insert(node.id, 1);
            stack.push((node.id, 0));

            while let Some((current, child_index)) = stack.last().copied() {
                let children = children_by_node.get(&current).map_or(&[][..], Vec::as_slice);
                if child_index >= children.len() {
                    state.insert(current, 2);
                    stack.pop();
                    continue;
                }

                let child = children[child_index];
                stack.last_mut().expect("DFS stack is non-empty").1 += 1;
                if !node_ids.contains(&child) {
                    continue;
                }
                match state.get(&child).copied().unwrap_or(0) {
                    1 => report.issues.push(ValidationIssue::ContainmentCycle { from: current, to: child }),
                    2 => {}
                    _ => {
                        state.insert(child, 1);
                        stack.push((child, 0));
                    }
                }
            }
        }
    }
}

fn endpoint_exists(
    endpoint: &RelationEndpoint,
    document_id: DocumentId,
    node_ids: &HashSet<NodeId>,
    asset_ids: &HashSet<AssetId>,
) -> bool {
    match endpoint {
        RelationEndpoint::Node(id) => node_ids.contains(id),
        RelationEndpoint::Document(id) => *id == document_id,
        RelationEndpoint::Asset(id) => asset_ids.contains(id),
        RelationEndpoint::ExternalUri(_) | RelationEndpoint::Unresolved { .. } => true,
    }
}

fn check_inline_refs(from: NodeId, inlines: &[Inline], node_ids: &HashSet<NodeId>, report: &mut ValidationReport) {
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
