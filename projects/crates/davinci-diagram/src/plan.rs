//! Layout results produced from a [`crate::spec::DiagramSpec`].
//!
//! A plan contains final geometry only: canvas size, placed node rectangles,
//! routed edge polylines, cluster frames, and non-fatal diagnostics. It never
//! carries SVG strings, drawing commands, scales, or renderer instructions.

use std::collections::BTreeMap;

use davinci_scene::{Point, Rect, Size};
use davinci_types::Diagnostic;

use crate::spec::{EdgeStyle, Endpoint, NodeShape};

/// A node with its final rectangle and resolved port anchors.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedNode {
    /// Stable node identity from the spec.
    pub id: String,
    /// Resolved display label; never blank, falling back to [`PlacedNode::id`].
    pub label: String,
    /// Requested node outline.
    pub shape: NodeShape,
    /// Final rectangle in scene coordinates.
    pub rect: Rect,
    /// The cluster frame that owns this node, if any.
    pub cluster: Option<String>,
    /// Declared port anchors in scene coordinates, keyed by port id.
    pub ports: BTreeMap<String, Point>,
}

/// An edge with its final orthogonal route and optional label box.
#[derive(Clone, Debug, PartialEq)]
pub struct RoutedEdge {
    /// The source endpoint as declared in the spec.
    pub from: Endpoint,
    /// The target endpoint as declared in the spec.
    pub to: Endpoint,
    /// Orthogonal polyline in scene coordinates, from source anchor to target
    /// anchor.
    pub points: Vec<Point>,
    /// Optional edge label.
    pub label: Option<String>,
    /// Reserved rectangle for the label, centered on the polyline midpoint.
    pub label_box: Option<Rect>,
    /// Requested edge style.
    pub style: EdgeStyle,
}

/// A cluster with its final bounding frame.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedCluster {
    /// Stable cluster identity.
    pub id: String,
    /// Optional frame label.
    pub label: Option<String>,
    /// The frame rectangle in scene coordinates.
    pub rect: Rect,
}

/// A deterministic layout plan for one diagram.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagramLayoutPlan {
    /// Canvas size that fits every node and cluster frame.
    pub canvas: Size,
    /// Nodes in spec declaration order.
    pub nodes: Vec<PlacedNode>,
    /// Routed edges in spec declaration order, excluding skipped edges.
    pub edges: Vec<RoutedEdge>,
    /// Cluster frames in spec declaration order.
    pub clusters: Vec<PlacedCluster>,
    /// Non-fatal loss, approximation, and conflict records.
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagramLayoutPlan {
    /// Find a placed node by its spec identity.
    pub fn node(&self, id: &str) -> Option<&PlacedNode> {
        self.nodes.iter().find(|node| node.id == id)
    }
}
