//! Structural validation for semantic graph invariants.

use crate::{
    block::Block,
    graph::DocumentGraph,
    id::{AssetId, DocumentId, LinkId, NodeId},
    inline::Inline,
    relation::{RelationEndpoint, RelationKind},
};
use std::collections::{HashMap, HashSet};

/// Single invariant violation on the document graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationIssue {
    /// Duplicate semantic node id.
    DuplicateNodeId {
        /// Repeated node id.
        id: NodeId,
    },
    /// Duplicate asset id.
    DuplicateAssetId {
        /// Repeated asset id.
        id: AssetId,
    },
    /// Duplicate relation id.
    DuplicateLinkId {
        /// Repeated relation id.
        id: LinkId,
    },
    /// Reference to a missing node.
    UnknownNode {
        /// Missing node id.
        id: NodeId,
    },
    /// Section child id not present in the block list.
    UnknownSectionChild {
        /// Section node id.
        section: NodeId,
        /// Missing child id.
        child: NodeId,
    },
    /// List child id not present in the block list.
    UnknownListChild {
        /// List node id.
        list: NodeId,
        /// Missing child id.
        child: NodeId,
    },
    /// Inline reference targets a missing node.
    UnknownInlineReference {
        /// Source node id.
        from: NodeId,
        /// Missing target node id.
        target: NodeId,
    },
    /// Inline link targets a missing endpoint.
    UnknownInlineLinkTarget {
        /// Source node id.
        from: NodeId,
        /// Invalid link target.
        target: RelationEndpoint,
    },
    /// Inline image references an unregistered asset.
    UnknownInlineImageAsset {
        /// Source node id.
        from: NodeId,
        /// Missing asset id.
        asset: AssetId,
    },
    /// Note or citation inline is missing its label/key.
    EmptyInlineLabel {
        /// Source node id.
        from: NodeId,
        /// Inline kind name for diagnostics.
        kind: String,
    },
    /// Relation endpoint refers to a missing node or asset.
    UnknownRelationEndpoint {
        /// Relation id with the invalid endpoint.
        link: LinkId,
    },
    /// Structural containment cycle detected.
    ContainmentCycle {
        /// Cycle source node.
        from: NodeId,
        /// Cycle target node.
        to: NodeId,
    },
    /// Ordered list item is missing an explicit number.
    MissingListItemNumber {
        /// List node id.
        list: NodeId,
        /// Zero-based item index inside the list.
        item_index: usize,
    },
    /// List item number violates the ordering invariant (non-positive or
    /// not strictly greater than the previous item).
    OutOfOrderListItemNumber {
        /// List node id.
        list: NodeId,
        /// Zero-based item index inside the list.
        item_index: usize,
        /// Observed number, when present.
        found: Option<u32>,
        /// Previous item number that this one must exceed.
        previous: Option<u32>,
    },
    /// Unordered list item should not carry a number.
    UnexpectedListItemNumber {
        /// List node id.
        list: NodeId,
        /// Zero-based item index inside the list.
        item_index: usize,
        /// Observed number.
        found: u32,
    },
}

/// Collected validation result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    /// Recorded invariant violations.
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    /// Whether no invariant violations were recorded.
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

impl DocumentGraph {
    /// Check identity uniqueness, endpoint existence, and local reference
    /// integrity.
    pub fn validate(&self) -> ValidationReport {
        let mut report = ValidationReport::default();
        let node_ids = self.collect_node_ids(&mut report);
        let asset_ids = self.collect_asset_ids(&mut report);
        self.check_relations(&node_ids, &asset_ids, &mut report);
        self.check_sources(&node_ids, &mut report);
        self.check_block_refs(&node_ids, &asset_ids, &mut report);
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

    fn check_relations(&self, node_ids: &HashSet<NodeId>, asset_ids: &HashSet<AssetId>, report: &mut ValidationReport) {
        let mut seen_links = HashSet::new();
        for relation in &self.relations {
            if !seen_links.insert(relation.id) {
                report.issues.push(ValidationIssue::DuplicateLinkId { id: relation.id });
            }

            if !endpoint_exists(&relation.source, self.id, node_ids, asset_ids)
                || !endpoint_exists(&relation.target, self.id, node_ids, asset_ids)
            {
                report.issues.push(ValidationIssue::UnknownRelationEndpoint { link: relation.id });
            }
        }
    }

    fn check_sources(&self, node_ids: &HashSet<NodeId>, report: &mut ValidationReport) {
        for source in &self.sources {
            if !node_ids.contains(&source.node) {
                report.issues.push(ValidationIssue::UnknownNode { id: source.node });
            }
        }
    }

    fn check_block_refs(&self, node_ids: &HashSet<NodeId>, asset_ids: &HashSet<AssetId>, report: &mut ValidationReport) {
        let context = InlineContext { document_id: self.id, node_ids, asset_ids };
        for node in &self.blocks {
            match &node.block {
                Block::Section { title, children, .. } => {
                    check_inline_refs(node.id, title, &context, report);
                    for child in children {
                        if !node_ids.contains(child) {
                            report.issues.push(ValidationIssue::UnknownSectionChild { section: node.id, child: *child });
                        }
                    }
                }
                Block::Paragraph { content } | Block::Quote { content } => {
                    check_inline_refs(node.id, content, &context, report);
                }
                Block::List { ordered, items, .. } => {
                    check_list_numbering(node.id, *ordered, items, report);
                    for item in items {
                        check_inline_refs(node.id, &item.content, &context, report);
                        for child in &item.children {
                            if !node_ids.contains(child) {
                                report.issues.push(ValidationIssue::UnknownListChild { list: node.id, child: *child });
                            }
                        }
                    }
                }
                Block::Table { rows } => {
                    for row in rows {
                        for cell in &row.cells {
                            check_inline_refs(node.id, cell, &context, report);
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
                if let (RelationEndpoint::Node(parent), RelationEndpoint::Node(child)) = (&relation.source, &relation.target) {
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

fn endpoint_exists(endpoint: &RelationEndpoint, document_id: DocumentId, node_ids: &HashSet<NodeId>, asset_ids: &HashSet<AssetId>) -> bool {
    match endpoint {
        RelationEndpoint::Node(id) => node_ids.contains(id),
        RelationEndpoint::Document(id) => *id == document_id,
        RelationEndpoint::Asset(id) => asset_ids.contains(id),
        RelationEndpoint::ExternalUri(_) | RelationEndpoint::Unresolved { .. } => true,
    }
}

/// Resolution environment shared by every inline reference check.
struct InlineContext<'a> {
    document_id: DocumentId,
    node_ids: &'a HashSet<NodeId>,
    asset_ids: &'a HashSet<AssetId>,
}

fn check_inline_refs(from: NodeId, inlines: &[Inline], context: &InlineContext<'_>, report: &mut ValidationReport) {
    for inline in inlines {
        match inline {
            Inline::Reference { target, .. } if !context.node_ids.contains(target) => {
                report.issues.push(ValidationIssue::UnknownInlineReference { from, target: *target });
            }
            Inline::Link { target, display, .. } => {
                if !endpoint_exists(target, context.document_id, context.node_ids, context.asset_ids) {
                    report.issues.push(ValidationIssue::UnknownInlineLinkTarget { from, target: target.clone() });
                }
                check_inline_refs(from, display, context, report);
            }
            Inline::Image { alt, asset, .. } => {
                if let Some(asset) = asset {
                    if !context.asset_ids.contains(asset) {
                        report.issues.push(ValidationIssue::UnknownInlineImageAsset { from, asset: *asset });
                    }
                }
                check_inline_refs(from, alt, context, report);
            }
            Inline::Note { label, display } => {
                check_label(from, "note", label, report);
                if let Some(display) = display {
                    check_inline_refs(from, display, context, report);
                }
            }
            Inline::Citation { key, display } => {
                check_label(from, "citation", key, report);
                if let Some(display) = display {
                    check_inline_refs(from, display, context, report);
                }
            }
            Inline::Styled { children, .. } | Inline::Attributed { children, .. } => {
                check_inline_refs(from, children, context, report);
            }
            _ => {}
        }
    }
}

fn check_label(from: NodeId, kind: &str, label: &str, report: &mut ValidationReport) {
    if label.trim().is_empty() {
        report.issues.push(ValidationIssue::EmptyInlineLabel { from, kind: kind.to_string() });
    }
}

/// Enforce list numbering invariants per [`Living 00`].
///
/// - Ordered list items must all carry a non-zero, strictly increasing number.
/// - Unordered list items must not carry a number.
fn check_list_numbering(list_id: NodeId, ordered: bool, items: &[crate::block::ListItem], report: &mut ValidationReport) {
    if !ordered {
        for (index, item) in items.iter().enumerate() {
            if let Some(n) = item.number {
                report.issues.push(ValidationIssue::UnexpectedListItemNumber { list: list_id, item_index: index, found: n });
            }
        }
        return;
    }

    let mut previous: Option<u32> = None;
    for (index, item) in items.iter().enumerate() {
        match item.number {
            None => report.issues.push(ValidationIssue::MissingListItemNumber { list: list_id, item_index: index }),
            Some(n) => {
                if n == 0 {
                    report.issues.push(ValidationIssue::OutOfOrderListItemNumber {
                        list: list_id,
                        item_index: index,
                        found: Some(n),
                        previous,
                    });
                }
                else if let Some(prev) = previous {
                    if n <= prev {
                        report.issues.push(ValidationIssue::OutOfOrderListItemNumber {
                            list: list_id,
                            item_index: index,
                            found: Some(n),
                            previous: Some(prev),
                        });
                    }
                }
                previous = Some(n);
            }
        }
    }
}
