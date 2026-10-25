//! Source-independent diagram semantics.
//!
//! A [`DiagramSpec`] describes what a diagram *means*: nodes, edges, clusters,
//! and ordering constraints. It carries no positions, sizes, drawing commands,
//! or source-syntax details. Frontends such as Mermaid lower into this model,
//! and [`crate::compile`] turns it into a layout plan.

/// The primary reading direction of a diagram.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Successive layers stack downwards.
    TopDown,
    /// Successive layers stack upwards.
    BottomUp,
    /// Successive layers advance to the right.
    LeftToRight,
    /// Successive layers advance to the left.
    RightToLeft,
}

/// The outline requested for a diagram node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeShape {
    /// A plain rectangle.
    Box,
    /// A rectangle with softly rounded corners.
    Round,
    /// A pill whose corner radius is half the node height.
    Stadium,
    /// A rectangle with an inner border.
    Subroutine,
    /// A database cylinder.
    Cylinder,
    /// A circle.
    Circle,
    /// A decision diamond.
    Rhombus,
}

/// How an edge is stroked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeStyle {
    /// A solid line without an arrow head.
    Line,
    /// A solid line with an arrow head.
    Arrow,
    /// A dotted line with an arrow head.
    DottedArrow,
    /// A thick line with an arrow head.
    ThickArrow,
    /// A dotted line without an arrow head.
    ///
    /// The current scene surface has no dash attribute, so this style is
    /// lowered as a solid line and reported as an approximation.
    DottedLine,
    /// A solid line ending in a hollow arrow head.
    ///
    /// Class inheritance uses this style so the superclass end stays distinct
    /// from an ordinary association arrow.
    HollowArrow,
    /// A solid line ending in a filled diamond.
    ///
    /// Class composition uses this style.
    FilledDiamond,
    /// A solid line ending in a hollow diamond.
    ///
    /// Class aggregation uses this style.
    HollowDiamond,
}

/// One side of a node rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The top edge.
    Top,
    /// The right edge.
    Right,
    /// The bottom edge.
    Bottom,
    /// The left edge.
    Left,
}

/// A named attachment point on a node boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    /// Identity used by [`Endpoint::port`].
    pub id: String,
    /// The boundary side the port sits on.
    pub side: Side,
    /// Position along the side in the inclusive range `0.0..=1.0`.
    ///
    /// `0.0` is the start of the side in reading order (left for horizontal
    /// sides, top for vertical sides) and `1.0` is the end.
    pub offset: f64,
}

/// A semantic node.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagramNode {
    /// Stable node identity, unique within one diagram.
    pub id: String,
    /// Display label. A missing or blank label falls back to the id.
    pub label: Option<String>,
    /// Requested outline.
    pub shape: NodeShape,
    /// Explicit attachment points on the node boundary.
    pub ports: Vec<Port>,
    /// Optional cluster membership declared on the node itself.
    pub cluster: Option<String>,
}

/// One end of an edge.
#[derive(Clone, Debug, PartialEq)]
pub struct Endpoint {
    /// The node identity.
    pub node: String,
    /// Optional port identity declared by [`DiagramNode::ports`].
    pub port: Option<String>,
}

/// A directed semantic connection between two nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagramEdge {
    /// The source endpoint.
    pub from: Endpoint,
    /// The target endpoint.
    pub to: Endpoint,
    /// Optional edge label.
    pub label: Option<String>,
    /// Requested edge style.
    pub style: EdgeStyle,
}

/// A named group of nodes drawn with one bounding frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Cluster {
    /// Stable cluster identity.
    pub id: String,
    /// Optional frame label.
    pub label: Option<String>,
    /// Member node identities in declaration order.
    pub members: Vec<String>,
}

/// An explicit ordering requirement layered layout must honour.
#[derive(Clone, Debug, PartialEq)]
pub enum Constraint {
    /// All listed nodes share one layer.
    SameRank(Vec<String>),
    /// The `after` node must be placed on a strictly later layer than `before`.
    PlaceBefore {
        /// The node that must come first.
        before: String,
        /// The node that must come later.
        after: String,
    },
}

/// A complete, source-independent diagram description.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagramSpec {
    /// Layer advance direction.
    pub direction: Direction,
    /// Nodes in declaration order.
    pub nodes: Vec<DiagramNode>,
    /// Edges in declaration order.
    pub edges: Vec<DiagramEdge>,
    /// Clusters in declaration order.
    pub clusters: Vec<Cluster>,
    /// Ordering constraints in declaration order.
    pub constraints: Vec<Constraint>,
}

impl DiagramSpec {
    /// Create an empty diagram with the given direction.
    pub fn new(direction: Direction) -> Self {
        Self { direction, nodes: Vec::new(), edges: Vec::new(), clusters: Vec::new(), constraints: Vec::new() }
    }
}
