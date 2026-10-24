//! Semantic scene: the unresolved domain identity tree.
//!
//! A [`SemanticScene`] keeps domain-level identity and unresolved display
//! intent. Transforms and clips are local to their node, groups keep their
//! nesting, and no geometry has been composed or flattened yet. Resolution into
//! a [`DisplayScene`](crate::DisplayScene) is the only consumer of this tree.

use davinci_types::{Point, Rect, Size, Transform};

use crate::{
    path::PathData,
    scene::{MarkerShape, ResourceRef, SemanticRole, ShapeStyle, SourceRef, TextStyle},
};

/// The unresolved geometry and paint intent of a semantic node.
#[derive(Clone, Debug, PartialEq)]
pub enum SemanticKind {
    /// A container with unresolved child transforms and its own opacity.
    Group {
        /// Child nodes in paint order.
        children: Vec<SemanticNode>,
        /// Group opacity in the inclusive range `0.0..=1.0`.
        opacity: f64,
    },
    /// An arbitrary path in the node's local coordinates.
    Path {
        /// Commands to draw before resolution.
        path: PathData,
        /// Fill and stroke attributes.
        style: ShapeStyle,
    },
    /// A straight segment between two local points.
    Line {
        /// Segment start.
        from: Point,
        /// Segment end.
        to: Point,
        /// Fill and stroke attributes.
        style: ShapeStyle,
    },
    /// An axis-aligned rectangle in the node's local coordinates.
    Rect {
        /// Rectangle before resolution.
        rect: Rect,
        /// Fill and stroke attributes.
        style: ShapeStyle,
    },
    /// An axis-aligned rounded rectangle in the node's local coordinates.
    RoundedRect {
        /// Rectangle before resolution.
        rect: Rect,
        /// Corner radius in scene units.
        radius: f64,
        /// Fill and stroke attributes.
        style: ShapeStyle,
    },
    /// An ellipse in the node's local coordinates.
    Ellipse {
        /// Ellipse center.
        center: Point,
        /// Horizontal (`width`) and vertical (`height`) radii.
        radii: Size,
        /// Fill and stroke attributes.
        style: ShapeStyle,
    },
    /// A single line of text.
    Text {
        /// Position on the text baseline in local coordinates.
        position: Point,
        /// Text content.
        text: String,
        /// Font and paint intent.
        style: TextStyle,
    },
    /// An embedded image placed into a local rectangle.
    Image {
        /// Placement rectangle in local coordinates.
        rect: Rect,
        /// Referenced image resource passed through to the renderer.
        resource: ResourceRef,
        /// Image opacity in the inclusive range `0.0..=1.0`.
        opacity: f64,
    },
    /// A small geometric marker used for diagram endpoints.
    Marker {
        /// Marker center in local coordinates.
        at: Point,
        /// Marker shape.
        shape: MarkerShape,
        /// Full marker size in scene units.
        size: f64,
        /// Fill and stroke attributes.
        style: ShapeStyle,
    },
}

/// A semantic tree node: identity plus unresolved local display intent.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticNode {
    /// Stable caller-provided identity.
    pub id: Option<String>,
    /// Domain meaning used for hit testing, accessibility, and motion matching.
    pub role: Option<SemanticRole>,
    /// Origin of this node in its source document or data snapshot.
    pub source: Option<SourceRef>,
    /// Hit region in this node's local coordinates.
    pub hit_region: Option<Rect>,
    /// Transform from this node's local coordinates into its parent.
    pub transform: Transform,
    /// Axis-aligned clip rectangle in this node's local coordinates.
    pub clip: Option<Rect>,
    /// Geometry and paint intent.
    pub kind: SemanticKind,
}

impl SemanticNode {
    /// Create a node from its unresolved geometry.
    pub fn new(kind: SemanticKind) -> Self {
        Self { id: None, role: None, source: None, hit_region: None, transform: Transform::IDENTITY, clip: None, kind }
    }

    /// Create a group node with default opacity.
    pub fn group(children: Vec<SemanticNode>) -> Self {
        Self::new(SemanticKind::Group { children, opacity: 1.0 })
    }

    /// Attach a stable identity.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Declare the node's domain meaning.
    pub fn with_role(mut self, role: SemanticRole) -> Self {
        self.role = Some(role);
        self
    }

    /// Declare where the node came from.
    pub fn with_source(mut self, source: SourceRef) -> Self {
        self.source = Some(source);
        self
    }

    /// Declare an explicit pointer target in local coordinates.
    pub fn with_hit_region(mut self, hit_region: Rect) -> Self {
        self.hit_region = Some(hit_region);
        self
    }

    /// Attach a local transform.
    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    /// Attach a local clip rectangle.
    pub fn with_clip(mut self, clip: Rect) -> Self {
        self.clip = Some(clip);
        self
    }
}

/// A semantic scene: canvas size plus ordered identity roots.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticScene {
    /// Natural canvas size carried through resolution.
    pub size: Size,
    /// Root nodes in paint order.
    pub nodes: Vec<SemanticNode>,
}

impl SemanticScene {
    /// Create an empty semantic scene with a canvas size.
    pub fn new(size: Size) -> Self {
        Self { size, nodes: Vec::new() }
    }

    /// Append a root node.
    pub fn push(&mut self, node: SemanticNode) {
        self.nodes.push(node);
    }
}
