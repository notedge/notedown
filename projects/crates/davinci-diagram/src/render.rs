//! Deterministic semantic scene rendering for a diagram layout plan.
//!
//! Rendering only translates final plan geometry into [`SemanticScene`] nodes.
//! It never re-runs layout, measures text, chooses scales, or emits backend
//! commands. Resolution into display geometry is a separate step.

use davinci_scene::{
    Color, MarkerShape, PathData, Point, Rect, SemanticKind, SemanticNode, SemanticRole, SemanticScene, ShapeStyle, Size, SourceRef,
    TextAnchor, TextStyle, Transform,
};

use crate::{
    plan::{DiagramLayoutPlan, PlacedCluster, PlacedNode, RoutedEdge},
    spec::{EdgeStyle, NodeShape},
};

const NODE_FILL: Color = Color::rgb(0xe8, 0xf0, 0xfe);
const NODE_STROKE: Color = Color::rgb(0x4a, 0x55, 0x68);
const EDGE_COLOR: Color = Color::rgb(0x2d, 0x37, 0x48);
const CLUSTER_FILL: Color = Color::rgb(0xf5, 0xf7, 0xfa);
const CLUSTER_STROKE: Color = Color::rgb(0x9a, 0xa6, 0xb8);
const ROUND_RADIUS: f64 = 8.0;
const CLUSTER_RADIUS: f64 = 10.0;
const LINE_HEIGHT: f64 = 1.4;
const BASELINE_SHIFT: f64 = 0.35;
const ARROW_SIZE: f64 = 10.0;
const CLUSTER_LABEL_X: f64 = 10.0;
const CLUSTER_LABEL_Y: f64 = 14.0;
const CLUSTER_FONT_SIZE: f64 = 12.0;
const LIFELINE_COLOR: Color = Color::rgb(0x9a, 0xa6, 0xb8);
const GRAPH_EDGE_PREFIX: &str = "diagram-edge";
const SEQUENCE_MESSAGE_PREFIX: &str = "diagram-sequence-message";

/// Tuning for deterministic scene rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderOptions {
    /// Font size used for node labels. Edge and cluster labels use a size
    /// derived from this value.
    pub font_size: f64,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self { font_size: 13.0 }
    }
}

/// Render a layout plan into a deterministic semantic scene.
///
/// This uses [`RenderOptions::default`]. Equal plans always produce equal
/// semantics and byte-identical SVG after resolution.
pub fn render(plan: &DiagramLayoutPlan) -> SemanticScene {
    render_with_options(plan, &RenderOptions::default())
}

/// Render a layout plan with explicit text options.
///
/// The output keeps one shape node per placed node carrying
/// [`SemanticRole::Node`], a [`SourceRef`] for the spec node id, and a hit
/// region equal to the node rectangle. Edges are groups with
/// [`SemanticRole::Edge`], edge labels use [`SemanticRole::EdgeLabel`], and
/// cluster frames use [`SemanticRole::Cluster`].
pub fn render_with_options(plan: &DiagramLayoutPlan, options: &RenderOptions) -> SemanticScene {
    let mut scene = SemanticScene::new(plan.canvas);

    for cluster in &plan.clusters {
        push_cluster(&mut scene, cluster, options);
    }
    for (index, edge) in plan.edges.iter().enumerate() {
        push_edge(&mut scene, GRAPH_EDGE_PREFIX, index, edge);
    }
    for node in &plan.nodes {
        push_node(&mut scene, node);
    }
    for node in &plan.nodes {
        push_node_label(&mut scene, node, options);
    }
    for (index, edge) in plan.edges.iter().enumerate() {
        push_edge_label(&mut scene, GRAPH_EDGE_PREFIX, index, edge, options);
    }

    scene
}

/// Render a sequence layout plan into a deterministic semantic scene.
///
/// Participants use the same box and label surface as graph nodes, messages
/// reuse routed edges with [`SemanticRole::Edge`], and every participant gets a
/// [`SemanticRole::Decoration`] lifeline from its header bottom to the canvas
/// bottom. Equal plans always produce equal scenes and byte-identical SVG after
/// resolution.
pub fn render_sequence(plan: &DiagramLayoutPlan) -> SemanticScene {
    render_sequence_with_options(plan, &RenderOptions::default())
}

/// Render a sequence layout plan with explicit text options.
pub fn render_sequence_with_options(plan: &DiagramLayoutPlan, options: &RenderOptions) -> SemanticScene {
    let mut scene = SemanticScene::new(plan.canvas);

    for participant in &plan.nodes {
        let center = participant.rect.center();
        scene.push(
            SemanticNode::new(SemanticKind::Line {
                from: Point::new(center.x, participant.rect.bottom()),
                to: Point::new(center.x, plan.canvas.height),
                style: ShapeStyle::stroked(LIFELINE_COLOR, 1.0),
            })
            .with_id(format!("diagram-sequence-lifeline-{}", participant.id))
            .with_role(SemanticRole::Decoration)
            .with_source(SourceRef::element(participant.id.clone())),
        );
    }
    for (index, message) in plan.edges.iter().enumerate() {
        push_edge(&mut scene, SEQUENCE_MESSAGE_PREFIX, index, message);
    }
    for participant in &plan.nodes {
        push_node(&mut scene, participant);
    }
    for participant in &plan.nodes {
        push_node_label(&mut scene, participant, options);
    }
    for (index, message) in plan.edges.iter().enumerate() {
        push_edge_label(&mut scene, SEQUENCE_MESSAGE_PREFIX, index, message, options);
    }

    scene
}

fn push_cluster(scene: &mut SemanticScene, cluster: &PlacedCluster, options: &RenderOptions) {
    let source = SourceRef::element(cluster.id.clone());
    scene.push(
        SemanticNode::new(SemanticKind::RoundedRect { rect: cluster.rect, radius: CLUSTER_RADIUS, style: cluster_style() })
            .with_id(format!("diagram-cluster-{}", cluster.id))
            .with_role(SemanticRole::Cluster)
            .with_source(source.clone()),
    );

    if let Some(label) = &cluster.label {
        let font_size = CLUSTER_FONT_SIZE.min(options.font_size);
        for (index, line) in label.split('\n').enumerate() {
            let position =
                Point::new(cluster.rect.x + CLUSTER_LABEL_X, cluster.rect.y + CLUSTER_LABEL_Y + index as f64 * font_size * LINE_HEIGHT);
            scene.push(
                SemanticNode::new(SemanticKind::Text { position, text: line.to_string(), style: label_text_style(font_size, CLUSTER_STROKE) })
                    .with_id(cluster_label_id(&cluster.id, index, label.split('\n').count()))
                    .with_role(SemanticRole::Label)
                    .with_source(source.clone()),
            );
        }
    }
}

fn push_edge(scene: &mut SemanticScene, id_prefix: &str, index: usize, edge: &RoutedEdge) {
    let source = SourceRef::element(format!("{}->{}", edge.from.node, edge.to.node));
    let mut path = PathData::new().move_to(edge.points[0]);
    for &point in &edge.points[1..] {
        path = path.line_to(point);
    }

    let mut children =
        vec![SemanticNode::new(SemanticKind::Path { path, style: edge_style(edge.style) }).with_id(format!("{id_prefix}-{index}-line"))];

    if has_arrow(edge.style) {
        let end = edge.points[edge.points.len() - 1];
        let previous = edge.points[edge.points.len() - 2];
        let angle = (end.y - previous.y).atan2(end.x - previous.x).to_degrees();
        let transform = Transform::rotate_degrees(angle + 90.0).then(Transform::translate(end.x, end.y));
        children.push(
            SemanticNode::new(SemanticKind::Marker {
                at: Point::ZERO,
                shape: MarkerShape::Triangle,
                size: ARROW_SIZE,
                style: ShapeStyle::filled(EDGE_COLOR),
            })
            .with_id(format!("{id_prefix}-{index}-arrow"))
            .with_transform(transform),
        );
    }

    scene.push(SemanticNode::group(children).with_id(format!("{id_prefix}-{index}")).with_role(SemanticRole::Edge).with_source(source));
}

fn push_node(scene: &mut SemanticScene, node: &PlacedNode) {
    let source = SourceRef::element(node.id.clone());
    let id = format!("diagram-node-{}", node.id);
    scene.push(node_shape(node).with_id(id.clone()).with_role(SemanticRole::Node).with_source(source).with_hit_region(node.rect));
}

fn push_node_label(scene: &mut SemanticScene, node: &PlacedNode, options: &RenderOptions) {
    let source = SourceRef::element(node.id.clone());
    push_centered_text(
        scene,
        &format!("diagram-node-{}-label", node.id),
        &node.label,
        node.rect.center(),
        options.font_size,
        LabelKind::Node,
        source,
    );
}

fn push_edge_label(scene: &mut SemanticScene, id_prefix: &str, index: usize, edge: &RoutedEdge, options: &RenderOptions) {
    let (Some(label), Some(label_box)) = (edge.label.as_ref(), edge.label_box)
    else {
        return;
    };
    let source = SourceRef::element(format!("{}->{}", edge.from.node, edge.to.node));
    let font_size = (options.font_size * 12.0 / 13.0).max(1.0);
    push_centered_text(scene, &format!("{id_prefix}-{index}-label"), label, label_box.center(), font_size, LabelKind::Edge, source);
}

#[derive(Clone, Copy)]
enum LabelKind {
    Node,
    Edge,
}

impl LabelKind {
    fn paint(self) -> (Color, SemanticRole) {
        match self {
            Self::Node => (Color::BLACK, SemanticRole::Label),
            Self::Edge => (EDGE_COLOR, SemanticRole::EdgeLabel),
        }
    }
}

fn push_centered_text(
    scene: &mut SemanticScene,
    id_prefix: &str,
    text: &str,
    center: Point,
    font_size: f64,
    kind: LabelKind,
    source: SourceRef,
) {
    let (fill, role) = kind.paint();
    let lines: Vec<&str> = text.split('\n').collect();
    for (index, line) in lines.iter().enumerate() {
        let offset = (index as f64 - (lines.len() - 1) as f64 * 0.5) * font_size * LINE_HEIGHT;
        let position = Point::new(center.x, center.y + offset + font_size * BASELINE_SHIFT);
        let id = if lines.len() == 1 { id_prefix.to_string() } else { format!("{id_prefix}-{index}") };
        scene.push(
            SemanticNode::new(SemanticKind::Text { position, text: (*line).to_string(), style: label_text_style(font_size, fill) })
                .with_id(id)
                .with_role(role)
                .with_source(source.clone()),
        );
    }
}

fn cluster_label_id(cluster: &str, index: usize, line_count: usize) -> String {
    if line_count == 1 { format!("diagram-cluster-{cluster}-label") } else { format!("diagram-cluster-{cluster}-label-{index}") }
}

fn node_shape(node: &PlacedNode) -> SemanticNode {
    let style = node_style();
    let kind = match node.shape {
        NodeShape::Box | NodeShape::Subroutine => SemanticKind::Rect { rect: node.rect, style },
        NodeShape::Round => SemanticKind::RoundedRect { rect: node.rect, radius: ROUND_RADIUS, style },
        NodeShape::Stadium => SemanticKind::RoundedRect { rect: node.rect, radius: node.rect.height * 0.5, style },
        NodeShape::Cylinder => SemanticKind::Group { children: cylinder_children(&node.id, node.rect, style), opacity: 1.0 },
        NodeShape::Circle => {
            let radius = node.rect.width.min(node.rect.height) * 0.5;
            SemanticKind::Ellipse { center: node.rect.center(), radii: Size::new(radius, radius), style }
        }
        NodeShape::Rhombus => SemanticKind::Path { path: rhombus_path(node.rect), style },
    };
    SemanticNode::new(kind)
}

fn cylinder_children(id: &str, rect: Rect, style: ShapeStyle) -> Vec<SemanticNode> {
    let cap = (rect.height * 0.15).max(2.0);
    let center = rect.center();
    let body = Rect::new(rect.x, rect.y + cap, rect.width, (rect.height - cap * 2.0).max(0.0));
    vec![
        SemanticNode::new(SemanticKind::Rect { rect: body, style: style.clone() }).with_id(format!("diagram-node-{id}-body")),
        SemanticNode::new(SemanticKind::Ellipse {
            center: Point::new(center.x, rect.bottom() - cap),
            radii: Size::new(rect.width * 0.5, cap),
            style: style.clone(),
        })
        .with_id(format!("diagram-node-{id}-bottom")),
        SemanticNode::new(SemanticKind::Ellipse { center: Point::new(center.x, rect.y + cap), radii: Size::new(rect.width * 0.5, cap), style })
            .with_id(format!("diagram-node-{id}-top")),
    ]
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

fn node_style() -> ShapeStyle {
    ShapeStyle { fill: Some(NODE_FILL), stroke: Some(NODE_STROKE), stroke_width: 1.5, opacity: 1.0 }
}

fn cluster_style() -> ShapeStyle {
    ShapeStyle { fill: Some(CLUSTER_FILL), stroke: Some(CLUSTER_STROKE), stroke_width: 1.0, opacity: 1.0 }
}

fn edge_style(style: EdgeStyle) -> ShapeStyle {
    let width = match style {
        EdgeStyle::ThickArrow => 2.5,
        _ => 1.5,
    };
    ShapeStyle::stroked(EDGE_COLOR, width)
}

fn label_text_style(font_size: f64, fill: Color) -> TextStyle {
    TextStyle {
        fill: Some(fill),
        opacity: 1.0,
        font_family: Some("sans-serif".to_string()),
        font_size,
        font_weight: None,
        text_anchor: TextAnchor::Middle,
    }
}

fn has_arrow(style: EdgeStyle) -> bool {
    matches!(style, EdgeStyle::Arrow | EdgeStyle::DottedArrow | EdgeStyle::ThickArrow)
}
