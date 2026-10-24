//! Flowchart AST lowering into a DaVinci scene.

use std::collections::HashMap;

use davinci_scene::{
    Color, MarkerShape, NodeKind, PathData, Point, Rect, Scene, SceneNode, ShapeStyle, Size, TextAnchor, TextStyle, Transform,
};
use davinci_types::{Diagnostic, DiagnosticSeverity, codes, diagnostic};
use oak_mermaid::ast::{EdgeStyle, FlowElement, Flowchart, NodeShape};

use crate::{
    error::DiagramError,
    layout::{Direction, layered_rects},
};

const NODE_FILL: Color = Color::rgb(0xe8, 0xf0, 0xfe);
const NODE_STROKE: Color = Color::rgb(0x4a, 0x55, 0x68);
const EDGE_COLOR: Color = Color::rgb(0x2d, 0x37, 0x48);
const NODE_FONT_SIZE: f64 = 13.0;
const EDGE_FONT_SIZE: f64 = 12.0;
const NODE_BASELINE_OFFSET: f64 = 4.5;
const NODE_HEIGHT: f64 = 40.0;
const NODE_MIN_WIDTH: f64 = 80.0;
const NODE_CHARACTER_WIDTH: f64 = 8.0;
const NODE_HORIZONTAL_PADDING: f64 = 24.0;
const ARROW_SIZE: f64 = 10.0;

/// A lowered diagram: a scene plus non-fatal loss and approximation records.
#[derive(Clone, Debug, PartialEq)]
pub struct LoweredDiagram {
    /// The generic vector scene.
    pub scene: Scene,
    /// Non-fatal records for approximated or degraded constructs.
    pub diagnostics: Vec<Diagnostic>,
}

impl LoweredDiagram {
    /// Serialize the lowered scene to deterministic SVG.
    pub fn to_svg(&self) -> Result<String, DiagramError> {
        self.scene.to_svg().map_err(DiagramError::from)
    }
}

struct NodeSeed {
    label: String,
    shape: NodeShape,
}

struct EdgeSeed {
    from: usize,
    to: usize,
    label: Option<String>,
    style: EdgeStyle,
}

/// Lower one flowchart into a deterministic layered scene.
///
/// Node boxes are sized from the label length, placed in declaration-ordered
/// layers, and connected with straight edges. Subgraphs are unavailable in the
/// pinned Oak AST, so they cannot appear here.
pub fn lower_flowchart(flowchart: &Flowchart) -> Result<LoweredDiagram, DiagramError> {
    let direction = Direction::parse(&flowchart.orientation)
        .ok_or_else(|| DiagramError::unsupported(format!("unsupported flowchart orientation `{}`", flowchart.orientation)))?;
    let (nodes, edges) = collect_elements(flowchart)?;

    if nodes.is_empty() {
        return Err(DiagramError::empty_diagram("flowchart contains no nodes or edges"));
    }

    let sizes: Vec<Size> = nodes.iter().map(|node| node_size(&node.label)).collect();
    let layout_edges: Vec<(usize, usize)> = edges.iter().map(|edge| (edge.from, edge.to)).collect();
    let layout = layered_rects(&sizes, &layout_edges, direction);

    let mut diagnostics = Vec::new();
    if !layout.cycle_nodes.is_empty() {
        diagnostics.push(diagnostic(
            "diagram",
            codes::APPROXIMATED,
            DiagnosticSeverity::Warning,
            format!(
                "flowchart contains a cycle involving nodes {:?}. Layered placement breaks it in declaration order",
                layout.cycle_nodes.iter().map(|index| format!("node-{index}")).collect::<Vec<_>>()
            ),
        ));
    }

    let mut scene = Scene::new(layout.canvas);
    for (index, edge) in edges.iter().enumerate() {
        let from = layout.rects[edge.from];
        let to = layout.rects[edge.to];
        let start = boundary_point(from, to.center());
        let end = boundary_point(to, from.center());

        let stroke_width = match edge.style {
            EdgeStyle::ThickArrow => 2.5,
            _ => 1.5,
        };
        scene.push(
            SceneNode::new(NodeKind::Line { from: start, to: end, style: ShapeStyle::stroked(EDGE_COLOR, stroke_width) })
                .with_id(format!("edge-{index}")),
        );

        if has_arrow(edge.style) {
            let angle = (end.y - start.y).atan2(end.x - start.x).to_degrees();
            let transform = Transform::rotate_degrees(angle + 90.0).then(Transform::translate(end.x, end.y));
            scene.push(
                SceneNode::new(NodeKind::Marker {
                    at: Point::ZERO,
                    shape: MarkerShape::Triangle,
                    size: ARROW_SIZE,
                    style: ShapeStyle::filled(EDGE_COLOR),
                })
                .with_id(format!("edge-{index}-arrow"))
                .with_transform(transform),
            );
        }

        if let Some(label) = &edge.label {
            let midpoint = Point::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5 - 6.0);
            scene.push(
                SceneNode::new(NodeKind::Text { position: midpoint, text: label.clone(), style: edge_text_style() })
                    .with_id(format!("edge-{index}-label")),
            );
        }

        if matches!(edge.style, EdgeStyle::DottedArrow) {
            diagnostics.push(diagnostic(
                "diagram",
                codes::APPROXIMATED,
                DiagnosticSeverity::Warning,
                format!("dotted edge `edge-{index}` is rendered as a solid line because the scene style has no dash surface"),
            ));
        }
    }

    for (index, node) in nodes.iter().enumerate() {
        let rect = layout.rects[index];
        let shape = node_shape(index, node, rect, &mut diagnostics);
        scene.push(shape);
        scene.push(
            SceneNode::new(NodeKind::Text {
                position: Point::new(rect.center().x, rect.center().y + NODE_BASELINE_OFFSET),
                text: node.label.clone(),
                style: node_text_style(),
            })
            .with_id(format!("node-{index}-label")),
        );
    }

    Ok(LoweredDiagram { scene, diagnostics })
}

/// Lower a parsed Mermaid root into a scene.
///
/// Only a single flowchart is accepted. Other diagram families and
/// multi-diagram roots fail explicitly because they have no deterministic
/// lowering yet.
#[cfg(feature = "mermaid")]
pub fn lower_mermaid(root: &oak_mermaid::MermaidRoot) -> Result<LoweredDiagram, DiagramError> {
    use oak_mermaid::ast::MermaidDiagram;

    if root.diagrams.is_empty() {
        return Err(DiagramError::ast_unavailable("Mermaid root contains no diagrams"));
    }
    if root.diagrams.len() > 1 {
        return Err(DiagramError::unsupported(format!(
            "a Mermaid root with {} diagrams is not lowered yet, exactly one diagram is supported",
            root.diagrams.len()
        )));
    }
    match &root.diagrams[0] {
        MermaidDiagram::Flowchart(flowchart) => lower_flowchart(flowchart),
        MermaidDiagram::Sequence(_) => Err(DiagramError::unsupported("sequence diagrams are not lowered yet")),
        MermaidDiagram::ClassDiagram(_) => Err(DiagramError::unsupported("class diagrams are not lowered yet")),
    }
}

fn collect_elements(flowchart: &Flowchart) -> Result<(Vec<NodeSeed>, Vec<EdgeSeed>), DiagramError> {
    let mut nodes: Vec<NodeSeed> = Vec::new();
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut edges: Vec<EdgeSeed> = Vec::new();

    for element in &flowchart.elements {
        match element {
            FlowElement::Node { id, text, shape } => {
                let id = require_id(id)?;
                match ids.get(&id) {
                    Some(index) => {
                        let node = &mut nodes[*index];
                        if let Some(text) = text.as_deref().map(str::trim).filter(|text| !text.is_empty()) {
                            node.label = text.to_string();
                        }
                        node.shape = *shape;
                    }
                    None => {
                        let label = text.as_deref().map(str::trim).filter(|text| !text.is_empty()).unwrap_or(id.as_str()).to_string();
                        let index = nodes.len();
                        nodes.push(NodeSeed { label, shape: *shape });
                        ids.insert(id, index);
                    }
                }
            }
            FlowElement::Edge { from, to, text, style } => {
                let from_id = require_id(from)?;
                let to_id = require_id(to)?;
                let from = ensure_node(&mut nodes, &mut ids, &from_id);
                let to = ensure_node(&mut nodes, &mut ids, &to_id);
                if from == to {
                    return Err(DiagramError::unsupported(format!("self edge `{from_id}` is not lowered yet")));
                }
                let label = text.as_deref().map(str::trim).filter(|text| !text.is_empty()).map(str::to_string);
                edges.push(EdgeSeed { from, to, label, style: *style });
            }
        }
    }

    Ok((nodes, edges))
}

fn require_id(id: &str) -> Result<String, DiagramError> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err(DiagramError::unsupported("flowchart contains a node or edge endpoint with an empty id"));
    }
    Ok(trimmed.to_string())
}

fn ensure_node(nodes: &mut Vec<NodeSeed>, ids: &mut HashMap<String, usize>, id: &str) -> usize {
    if let Some(index) = ids.get(id) {
        return *index;
    }
    let index = nodes.len();
    nodes.push(NodeSeed { label: id.to_string(), shape: NodeShape::Box });
    ids.insert(id.to_string(), index);
    index
}

fn node_size(label: &str) -> Size {
    let width = (label.chars().count() as f64 * NODE_CHARACTER_WIDTH + NODE_HORIZONTAL_PADDING).max(NODE_MIN_WIDTH);
    Size::new(width, NODE_HEIGHT)
}

fn node_shape(index: usize, node: &NodeSeed, rect: Rect, diagnostics: &mut Vec<Diagnostic>) -> SceneNode {
    let style = ShapeStyle { fill: Some(NODE_FILL), stroke: Some(NODE_STROKE), stroke_width: 1.5, opacity: 1.0 };
    let kind = match node.shape {
        NodeShape::Box => NodeKind::Rect { rect, style },
        NodeShape::Round => NodeKind::RoundedRect { rect, radius: 8.0, style },
        NodeShape::Stadium => NodeKind::RoundedRect { rect, radius: rect.height * 0.5, style },
        NodeShape::Subroutine => {
            diagnostics
                .push(approximation(format!("subroutine node `node-{index}` is rendered as a plain rectangle without its double border")));
            NodeKind::Rect { rect, style }
        }
        NodeShape::Cylinder => {
            diagnostics.push(approximation(format!("cylinder node `node-{index}` is rendered as a rounded rectangle")));
            NodeKind::RoundedRect { rect, radius: 6.0, style }
        }
        NodeShape::Circle => {
            diagnostics.push(approximation(format!("circle node `node-{index}` is rendered as an ellipse matching its node box")));
            NodeKind::Ellipse { center: rect.center(), radii: Size::new(rect.width * 0.5, rect.height * 0.5), style }
        }
        NodeShape::Rhombus => NodeKind::Path { path: rhombus_path(rect), style },
    };
    SceneNode::new(kind).with_id(format!("node-{index}"))
}

fn rhombus_path(rect: Rect) -> PathData {
    let center = rect.center();
    PathData::new()
        .move_to(Point::new(center.x, rect.y))
        .line_to(Point::new(rect.right(), center.y))
        .line_to(Point::new(center.x, rect.bottom()))
        .line_to(Point::new(rect.x, center.y))
        .close()
}

fn boundary_point(rect: Rect, toward: Point) -> Point {
    let center = rect.center();
    let dx = toward.x - center.x;
    let dy = toward.y - center.y;
    if dx == 0.0 && dy == 0.0 {
        return center;
    }
    let scale_x = if dx != 0.0 { rect.width * 0.5 / dx.abs() } else { f64::INFINITY };
    let scale_y = if dy != 0.0 { rect.height * 0.5 / dy.abs() } else { f64::INFINITY };
    let scale = scale_x.min(scale_y);
    Point::new(center.x + dx * scale, center.y + dy * scale)
}

fn has_arrow(style: EdgeStyle) -> bool {
    matches!(style, EdgeStyle::Arrow | EdgeStyle::DottedArrow | EdgeStyle::ThickArrow)
}

fn node_text_style() -> TextStyle {
    TextStyle {
        fill: Some(Color::BLACK),
        opacity: 1.0,
        font_family: Some("sans-serif".to_string()),
        font_size: NODE_FONT_SIZE,
        font_weight: None,
        text_anchor: TextAnchor::Middle,
    }
}

fn edge_text_style() -> TextStyle {
    TextStyle {
        fill: Some(EDGE_COLOR),
        opacity: 1.0,
        font_family: Some("sans-serif".to_string()),
        font_size: EDGE_FONT_SIZE,
        font_weight: None,
        text_anchor: TextAnchor::Middle,
    }
}

fn approximation(message: impl Into<String>) -> Diagnostic {
    diagnostic("diagram", codes::APPROXIMATED, DiagnosticSeverity::Warning, message)
}
