//! Node-level resolution of an invalidation set against scene identity.
//!
//! A layer set says which kind of work is stale. This module narrows that set
//! to identified scene nodes using stable ids, roles, and source provenance.
//! The result is honest about what cannot be narrowed: layers that reach global
//! state appear in [`NodeInvalidation::whole_layers`] instead of pretending to
//! be precise.
//!
//! # Narrowing rules
//!
//! | Layer | Recompute | Whole layer |
//! |-------|-----------|-------------|
//! | `Overlay` | ids equal to a target, sharing the target node's source element, or prefixed by `target/` or `target:` | no targets, or a matching node has no id |
//! | `Style` | every identified node | any node has no id |
//! | `Measurement` | identified text nodes | an unidentified text node exists |
//! | `Data` | identified nodes with a data role or a source ref | an unidentified data-bearing node exists |
//! | `Layout` | every identified node | always, because placement and bounds are global |
//! | `Viewport` | none | always, because only the projection changes |
//! | `Motion` | none | always, because naming nodes needs a transition plan |
//!
//! Data roles are `Node`, `Edge`, `EdgeLabel`, `Cluster`, `Axis`, `Tick`,
//! `Legend`, and `Mark`. `SemanticRole::Label` alone is not data-bearing, but a
//! label whose node carries a [`SourceRef`] is.
//!
//! The set is used exactly as given. [`RuntimeUpdate`](crate::RuntimeUpdate)
//! already carries the closed set, so a parameter change reaches `Layout` and
//! therefore every identified node. Call [`InvalidationSet::closure`]
//! manually for hand-built sets that should behave like a reducer result.

use std::collections::{BTreeMap, BTreeSet};

use davinci_scene::{DisplayScene, NodeKind, SceneNode, SemanticKind, SemanticNode, SemanticRole, SemanticScene, SourceRef};

use crate::{InvalidationScope, InvalidationSet, RuntimeUpdate};

/// Node ids reached by an invalidation set.
///
/// `recompute` and `retained` are claims about node content only. Whenever
/// `whole_layers` is non-empty, the corresponding layer must still run
/// globally and may touch retained nodes. `Layout` is always whole, so
/// `retained` never claims that placement is reusable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NodeInvalidation {
    /// Identified nodes whose output must be recomputed for a narrowed layer.
    pub recompute: BTreeSet<String>,
    /// Identified nodes absent from `recompute`.
    pub retained: BTreeSet<String>,
    /// Layers that cannot be narrowed to individual ids.
    pub whole_layers: InvalidationSet,
}

impl NodeInvalidation {
    /// Returns `true` when nothing is named and no layer is global.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.recompute.is_empty() && self.whole_layers.is_empty()
    }
}

/// Read the identity facts shared by semantic and resolved scene nodes.
trait Identified: Sized {
    fn id(&self) -> Option<&str>;
    fn role(&self) -> Option<SemanticRole>;
    fn source(&self) -> Option<&SourceRef>;
    fn is_text(&self) -> bool;
    fn children(&self) -> &[Self];
}

impl Identified for SceneNode {
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    fn role(&self) -> Option<SemanticRole> {
        self.role
    }

    fn source(&self) -> Option<&SourceRef> {
        self.source.as_ref()
    }

    fn is_text(&self) -> bool {
        matches!(&self.kind, NodeKind::Text { .. })
    }

    fn children(&self) -> &[Self] {
        match &self.kind {
            NodeKind::Group { children, .. } => children,
            _ => &[],
        }
    }
}

impl Identified for SemanticNode {
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    fn role(&self) -> Option<SemanticRole> {
        self.role
    }

    fn source(&self) -> Option<&SourceRef> {
        self.source.as_ref()
    }

    fn is_text(&self) -> bool {
        matches!(&self.kind, SemanticKind::Text { .. })
    }

    fn children(&self) -> &[Self] {
        match &self.kind {
            SemanticKind::Group { children, .. } => children,
            _ => &[],
        }
    }
}

/// Flattened identity facts of one node, borrowed from its scene.
struct Fact<'a> {
    id: Option<&'a str>,
    role: Option<SemanticRole>,
    source: Option<&'a str>,
    is_text: bool,
}

/// Resolve an update against a resolved display scene.
#[must_use]
pub fn invalidated_nodes(update: &RuntimeUpdate, scene: &DisplayScene) -> NodeInvalidation {
    let facts = collect(&scene.nodes);
    resolve(&facts, &update.invalidation, &update.targets)
}

/// Resolve an update against the unresolved semantic scene.
///
/// Both scene stages carry the same identity fields, so narrowing behaves the
/// same before and after resolution.
#[must_use]
pub fn invalidated_semantic_nodes(update: &RuntimeUpdate, scene: &SemanticScene) -> NodeInvalidation {
    let facts = collect(&scene.nodes);
    resolve(&facts, &update.invalidation, &update.targets)
}

/// Flatten a scene tree into identity facts in document order.
fn collect<'a, N: Identified>(roots: &'a [N]) -> Vec<Fact<'a>> {
    let mut facts = Vec::new();
    for node in roots {
        push_facts(node, &mut facts);
    }
    facts
}

fn push_facts<'a, N: Identified>(node: &'a N, facts: &mut Vec<Fact<'a>>) {
    facts.push(Fact {
        id: node.id(),
        role: node.role(),
        source: node.source().and_then(|source| source.element.as_deref()),
        is_text: node.is_text(),
    });
    for child in node.children() {
        push_facts(child, facts);
    }
}

/// Apply the narrowing rules to a flat fact list.
fn resolve(facts: &[Fact<'_>], scopes: &InvalidationSet, targets: &BTreeSet<String>) -> NodeInvalidation {
    let mut result = NodeInvalidation::default();

    // The first occurrence of an id decides which source element a target
    // stands for, matching the document-order contract used elsewhere.
    let mut source_of: BTreeMap<&str, &str> = BTreeMap::new();
    for fact in facts {
        if let (Some(id), Some(source)) = (fact.id, fact.source) {
            source_of.entry(id).or_insert(source);
        }
    }
    let target_sources: BTreeSet<&str> = targets.iter().filter_map(|target| source_of.get(target.as_str()).copied()).collect();

    if scopes.contains(InvalidationScope::Overlay) {
        resolve_overlay(facts, targets, &target_sources, &mut result);
    }
    if scopes.contains(InvalidationScope::Style) {
        for fact in facts {
            insert_node(fact, InvalidationScope::Style, &mut result);
        }
    }
    if scopes.contains(InvalidationScope::Measurement) {
        for fact in facts.iter().filter(|fact| fact.is_text) {
            insert_node(fact, InvalidationScope::Measurement, &mut result);
        }
    }
    if scopes.contains(InvalidationScope::Data) {
        for fact in facts.iter().filter(|fact| is_data_bearing(fact)) {
            insert_node(fact, InvalidationScope::Data, &mut result);
        }
    }
    if scopes.contains(InvalidationScope::Layout) {
        result.whole_layers.insert(InvalidationScope::Layout);
        for fact in facts {
            if let Some(id) = fact.id {
                result.recompute.insert(id.to_owned());
            }
        }
    }
    if scopes.contains(InvalidationScope::Viewport) {
        result.whole_layers.insert(InvalidationScope::Viewport);
    }
    if scopes.contains(InvalidationScope::Motion) {
        result.whole_layers.insert(InvalidationScope::Motion);
    }

    result.retained = facts.iter().filter_map(|fact| fact.id).map(str::to_owned).filter(|id| !result.recompute.contains(id)).collect();
    result
}

/// Overlay narrowing: targeted nodes plus their source and id-prefix relatives.
fn resolve_overlay(facts: &[Fact<'_>], targets: &BTreeSet<String>, target_sources: &BTreeSet<&str>, result: &mut NodeInvalidation) {
    if targets.is_empty() {
        result.whole_layers.insert(InvalidationScope::Overlay);
        return;
    }
    for fact in facts {
        let matches_source = fact.source.is_some_and(|source| target_sources.contains(source));
        let matches_id = fact.id.is_some_and(|id| id_matches_target(id, targets));
        if matches_source || matches_id {
            insert_node(fact, InvalidationScope::Overlay, result);
        }
    }
}

/// An id matches a target exactly or as a highlight child such as
/// `target/outline` or `target:ring`.
fn id_matches_target(id: &str, targets: &BTreeSet<String>) -> bool {
    targets
        .iter()
        .any(|target| id == target || id.strip_prefix(target.as_str()).is_some_and(|rest| rest.starts_with('/') || rest.starts_with(':')))
}

/// Nodes that data snapshots and scales can reach.
fn is_data_bearing(fact: &Fact<'_>) -> bool {
    fact.source.is_some()
        || matches!(
            fact.role,
            Some(
                SemanticRole::Node
                    | SemanticRole::Edge
                    | SemanticRole::EdgeLabel
                    | SemanticRole::Cluster
                    | SemanticRole::Axis
                    | SemanticRole::Tick
                    | SemanticRole::Legend
                    | SemanticRole::Mark
            )
        )
}

/// Record a node id, or mark the layer global when the node has no id.
fn insert_node(fact: &Fact<'_>, scope: InvalidationScope, result: &mut NodeInvalidation) {
    match fact.id {
        Some(id) => {
            result.recompute.insert(id.to_owned());
        }
        None => {
            result.whole_layers.insert(scope);
        }
    }
}
