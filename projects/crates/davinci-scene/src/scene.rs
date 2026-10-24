//! Scene graph, node shapes, and drawing styles.

use davinci_types::{Color, Point, Rect, Size, Transform};

use crate::{error::SceneError, path::PathData};

/// How a text element aligns its content horizontally.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAnchor {
    /// The position is the left edge of the text.
    #[default]
    Start,
    /// The position is the horizontal center of the text.
    Middle,
    /// The position is the right edge of the text.
    End,
}

impl TextAnchor {
    /// The SVG `text-anchor` keyword.
    pub fn as_svg(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Middle => "middle",
            Self::End => "end",
        }
    }
}

/// Fill and stroke attributes for a shape.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeStyle {
    /// Interior paint, or `None` for an unfilled shape.
    pub fill: Option<Color>,
    /// Outline paint, or `None` for no outline.
    pub stroke: Option<Color>,
    /// Outline width in scene units. Only meaningful when `stroke` is set.
    pub stroke_width: f64,
    /// Group and shape opacity in the inclusive range `0.0..=1.0`.
    pub opacity: f64,
}

impl ShapeStyle {
    /// A shape filled with a color and no outline.
    pub fn filled(fill: Color) -> Self {
        Self { fill: Some(fill), ..Self::default() }
    }

    /// An unfilled shape with an outline.
    pub fn stroked(stroke: Color, stroke_width: f64) -> Self {
        Self { stroke: Some(stroke), stroke_width, ..Self::default() }
    }
}

impl Default for ShapeStyle {
    fn default() -> Self {
        Self { fill: None, stroke: None, stroke_width: 1.0, opacity: 1.0 }
    }
}

/// Font and paint attributes for a text element.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// Text paint, or `None` to inherit the host default.
    pub fill: Option<Color>,
    /// Opacity in the inclusive range `0.0..=1.0`.
    pub opacity: f64,
    /// Font family passed through to the renderer.
    pub font_family: Option<String>,
    /// Font size in scene units. Must be positive.
    pub font_size: f64,
    /// Optional numeric font weight.
    pub font_weight: Option<u16>,
    /// Horizontal alignment relative to the position.
    pub text_anchor: TextAnchor,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self { fill: Some(Color::BLACK), opacity: 1.0, font_family: None, font_size: 16.0, font_weight: None, text_anchor: TextAnchor::Start }
    }
}

/// The shape drawn for a [`NodeKind::Marker`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkerShape {
    /// A filled or stroked circle.
    Circle,
    /// A triangle whose default orientation points up.
    ///
    /// Rotate the owning node to point the triangle along an edge.
    Triangle,
    /// A diamond with points on the four axis extremes.
    Diamond,
}

/// The geometry carried by a scene node.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    /// A container that inherits and composes child transforms.
    Group {
        /// Child nodes in paint order.
        children: Vec<SceneNode>,
        /// Group opacity in the inclusive range `0.0..=1.0`.
        opacity: f64,
    },
    /// An arbitrary path in local coordinates.
    Path {
        /// Commands to draw.
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
        /// Fill and stroke attributes. Fill is ignored by the writer.
        style: ShapeStyle,
    },
    /// An axis-aligned rectangle.
    Rect {
        /// Rectangle in local coordinates.
        rect: Rect,
        /// Fill and stroke attributes.
        style: ShapeStyle,
    },
    /// An axis-aligned rectangle with rounded corners.
    RoundedRect {
        /// Rectangle in local coordinates.
        rect: Rect,
        /// Corner radius in scene units.
        radius: f64,
        /// Fill and stroke attributes.
        style: ShapeStyle,
    },
    /// An ellipse defined by its center and horizontal and vertical radii.
    Ellipse {
        /// Ellipse center in local coordinates.
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
        /// Font and paint attributes.
        style: TextStyle,
    },
    /// An embedded image placed into a local rectangle.
    Image {
        /// Placement rectangle in local coordinates.
        rect: Rect,
        /// Image reference passed through to the renderer.
        href: String,
        /// Image opacity in the inclusive range `0.0..=1.0`.
        opacity: f64,
    },
    /// A small geometric marker used for diagram endpoints.
    Marker {
        /// Marker center in local coordinates.
        at: Point,
        /// Marker shape.
        shape: MarkerShape,
        /// Full marker size in scene units. The shape is centered on `at`.
        size: f64,
        /// Fill and stroke attributes.
        style: ShapeStyle,
    },
}

/// A scene tree node with its own transform, optional clip, and identity.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneNode {
    /// Stable caller-provided identity. The SVG writer emits it as `id`.
    pub id: Option<String>,
    /// Transform applied to this node and all of its children.
    pub transform: Transform,
    /// Axis-aligned clip rectangle in this node's local coordinates.
    pub clip: Option<Rect>,
    /// Geometry and paint attributes.
    pub kind: NodeKind,
}

impl SceneNode {
    /// Create a node from its geometry.
    pub fn new(kind: NodeKind) -> Self {
        Self { id: None, transform: Transform::IDENTITY, clip: None, kind }
    }

    /// Create a group node with default opacity.
    pub fn group(children: Vec<SceneNode>) -> Self {
        Self::new(NodeKind::Group { children, opacity: 1.0 })
    }

    /// Attach a stable identity.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
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

/// A generic vector scene: a canvas size plus ordered root nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// Natural canvas size. `to_svg` emits `viewBox="0 0 width height"`.
    pub size: Size,
    /// Root nodes in paint order.
    pub nodes: Vec<SceneNode>,
}

impl Scene {
    /// Create an empty scene with a canvas size.
    pub fn new(size: Size) -> Self {
        Self { size, nodes: Vec::new() }
    }

    /// Append a root node.
    pub fn push(&mut self, node: SceneNode) {
        self.nodes.push(node);
    }

    /// Validate every node and style before serialization.
    pub fn validate(&self) -> Result<(), SceneError> {
        crate::svg::validate_scene(self)
    }

    /// Serialize the scene to a deterministic SVG document.
    ///
    /// Attribute order and number formatting are fixed, so equal scenes
    /// produce byte-identical output.
    pub fn to_svg(&self) -> Result<String, SceneError> {
        crate::svg::write_svg(self)
    }
}
