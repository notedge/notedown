//! Shared scene builders for the integration tests.

use davinci_motion::{Easing, MotionIntent};
use davinci_scene::{DisplayScene, NodeKind, Point, Rect, SceneNode, ShapeStyle, Size, TextStyle, Transform};

/// Create a 200x100 scene from ordered root nodes.
pub fn scene(nodes: Vec<SceneNode>) -> DisplayScene {
    let mut scene = DisplayScene::new(Size::new(200.0, 100.0));
    for node in nodes {
        scene.push(node);
    }
    scene
}

/// A 10x10 rectangle with a stable id, translated along both axes.
pub fn rect_at(id: &str, x: f64, y: f64, opacity: f64) -> SceneNode {
    SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 10.0), style: ShapeStyle { opacity, ..ShapeStyle::default() } })
        .with_id(id)
        .with_transform(Transform::translate(x, y))
}

/// A 10x10 rectangle with a stable id, translated horizontally.
pub fn rect(id: &str, x: f64, opacity: f64) -> SceneNode {
    rect_at(id, x, 0.0, opacity)
}

/// A 10x10 rectangle without an id.
pub fn unlabeled_rect(x: f64) -> SceneNode {
    SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 10.0), style: ShapeStyle::default() })
        .with_transform(Transform::translate(x, 0.0))
}

/// A group with a stable id and explicit opacity.
pub fn group(id: &str, opacity: f64, children: Vec<SceneNode>) -> SceneNode {
    SceneNode::new(NodeKind::Group { children, opacity }).with_id(id)
}

/// A single line of text with a stable id.
pub fn text(id: &str, opacity: f64) -> SceneNode {
    SceneNode::new(NodeKind::Text {
        position: Point::new(0.0, 0.0),
        text: "label".to_owned(),
        style: TextStyle { opacity, ..TextStyle::default() },
    })
    .with_id(id)
}

/// Find the first node with an id anywhere in the tree.
pub fn find<'a>(scene: &'a DisplayScene, id: &str) -> Option<&'a SceneNode> {
    scene.nodes.iter().find_map(|node| find_node(node, id))
}

fn find_node<'a>(node: &'a SceneNode, id: &str) -> Option<&'a SceneNode> {
    if node.id.as_deref() == Some(id) {
        return Some(node);
    }
    match &node.kind {
        NodeKind::Group { children, .. } => children.iter().find_map(|child| find_node(child, id)),
        _ => None,
    }
}

/// Read the node's own opacity the same way motion does.
pub fn opacity(node: &SceneNode) -> f64 {
    match &node.kind {
        NodeKind::Group { opacity, .. } | NodeKind::Image { opacity, .. } => *opacity,
        NodeKind::Path { style, .. }
        | NodeKind::Line { style, .. }
        | NodeKind::Rect { style, .. }
        | NodeKind::RoundedRect { style, .. }
        | NodeKind::Ellipse { style, .. }
        | NodeKind::Marker { style, .. } => style.opacity,
        NodeKind::Text { style, .. } => style.opacity,
    }
}

/// Horizontal translation of a node with the given id.
pub fn x(scene: &DisplayScene, id: &str) -> f64 {
    find(scene, id).expect("node should exist").transform.e()
}

/// A linear intent with the given duration and otherwise default policies.
pub fn linear_intent(duration: f64) -> MotionIntent {
    MotionIntent { duration, ease: Easing::Linear, ..MotionIntent::default() }
}
