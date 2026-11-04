//! Authored byte ranges that tie semantic diagram items back to frontend tokens.

use std::ops::Range;

/// Frontend family name used when a mapped diagnostic has no family.
const DEFAULT_SOURCE: &str = "diagram";

/// Authored byte ranges for [`crate::spec::DiagramSpec`] items.
///
/// The map is a sidecar, not part of the semantic spec: [`crate::spec::DiagramSpec`]
/// keeps carrying no source syntax, and a frontend that cannot locate an item
/// records `None` for it. Entries are appended in the same order their spec
/// vectors grow, so index `n` names the token that produced `spec.nodes[n]`
/// or `spec.edges[n]`.
///
/// The Oak Mermaid AST exposes only the whole-diagram range, so the Mermaid
/// frontend leaves the map empty. The DOT and PlantUML ASTs carry per-item
/// ranges, so those frontends locate the token a diagnostic should point at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceMap {
    source: Option<&'static str>,
    nodes: Vec<Option<Range<usize>>>,
    edges: Vec<Option<Range<usize>>>,
}

impl SourceMap {
    /// Create an empty map that names its frontend family, such as `dot`.
    pub fn new(source: &'static str) -> Self {
        Self { source: Some(source), ..Self::default() }
    }

    /// The frontend family a mapped diagnostic is attributed to.
    pub fn family(&self) -> &'static str {
        self.source.unwrap_or(DEFAULT_SOURCE)
    }

    /// Record the token range that produced the next spec node.
    pub fn push_node(&mut self, span: Option<Range<usize>>) {
        self.nodes.push(span);
    }

    /// Record the statement range that produced the next spec edge.
    pub fn push_edge(&mut self, span: Option<Range<usize>>) {
        self.edges.push(span);
    }

    /// The declared token range of node `index`, when the frontend found one.
    pub fn node(&self, index: usize) -> Option<Range<usize>> {
        self.nodes.get(index).cloned().flatten()
    }

    /// The statement range of edge `index`, when the frontend found one.
    pub fn edge(&self, index: usize) -> Option<Range<usize>> {
        self.edges.get(index).cloned().flatten()
    }
}
