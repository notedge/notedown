//! Traversal and value helpers shared by planning and sampling.

use std::collections::{HashMap, HashSet};

use davinci_scene::{DisplayScene, NodeKind, SceneNode, Transform};

/// Linear interpolation between two scalars with exact endpoints.
///
/// `t <= 0` returns `from` and `t >= 1` returns `to` without arithmetic, so
/// endpoints are bit-exact even when the arithmetic would round differently.
pub(crate) fn lerp(from: f64, to: f64, t: f64) -> f64 {
    if t <= 0.0 {
        from
    }
    else if t >= 1.0 {
        to
    }
    else {
        from + (to - from) * t
    }
}

/// Component-wise linear interpolation of two affine matrices.
///
/// `davinci_types::Transform` intentionally has no interpolation operator, so
/// motion owns this helper. Rotation and scale are interpolated as matrix
/// components rather than decomposed, which keeps the result deterministic and
/// makes the interpolation contract explicit.
pub(crate) fn lerp_transform(from: Transform, to: Transform, t: f64) -> Transform {
    Transform::new(
        lerp(from.a(), to.a(), t),
        lerp(from.b(), to.b(), t),
        lerp(from.c(), to.c(), t),
        lerp(from.d(), to.d(), t),
        lerp(from.e(), to.e(), t),
        lerp(from.f(), to.f(), t),
    )
}

/// The node's own opacity, independent of its ancestors.
///
/// The scene model stores opacity on the variant that owns it: groups and
/// images carry an `opacity` field, while shapes and text store it in their
/// style. Motion treats these as one animatable opacity channel.
pub(crate) fn node_opacity(node: &SceneNode) -> f64 {
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

/// Set the node's own opacity, clamped to `0.0..=1.0`.
pub(crate) fn set_node_opacity(node: &mut SceneNode, value: f64) {
    let value = value.clamp(0.0, 1.0);
    match &mut node.kind {
        NodeKind::Group { opacity, .. } | NodeKind::Image { opacity, .. } => *opacity = value,
        NodeKind::Path { style, .. }
        | NodeKind::Line { style, .. }
        | NodeKind::Rect { style, .. }
        | NodeKind::RoundedRect { style, .. }
        | NodeKind::Ellipse { style, .. }
        | NodeKind::Marker { style, .. } => style.opacity = value,
        NodeKind::Text { style, .. } => style.opacity = value,
    }
}

/// Scale the node's own opacity by `factor`, clamped to `0.0..=1.0`.
pub(crate) fn multiply_node_opacity(node: &mut SceneNode, factor: f64) {
    set_node_opacity(node, node_opacity(node) * factor);
}

/// First node for every id in pre-order document order.
///
/// Duplicate ids keep the first occurrence, which is the contract every other
/// operation in this crate uses.
pub(crate) fn first_nodes(scene: &DisplayScene) -> HashMap<&str, &SceneNode> {
    let mut nodes = HashMap::new();
    for node in &scene.nodes {
        collect_first_nodes(node, &mut nodes);
    }
    nodes
}

fn collect_first_nodes<'a>(node: &'a SceneNode, nodes: &mut HashMap<&'a str, &'a SceneNode>) {
    if let Some(id) = node.id.as_deref() {
        nodes.entry(id).or_insert(node);
    }
    if let NodeKind::Group { children, .. } = &node.kind {
        for child in children {
            collect_first_nodes(child, nodes);
        }
    }
}

/// Index path of the first occurrence of every id, where a root node is a
/// one-element path and each further element is a child index inside a group.
pub(crate) fn first_paths(scene: &DisplayScene) -> HashMap<&str, Vec<usize>> {
    let mut paths = HashMap::new();
    for (index, node) in scene.nodes.iter().enumerate() {
        let mut path = vec![index];
        collect_first_paths(node, &mut path, &mut paths);
    }
    paths
}

fn collect_first_paths<'a>(node: &'a SceneNode, path: &mut Vec<usize>, paths: &mut HashMap<&'a str, Vec<usize>>) {
    if let Some(id) = node.id.as_deref() {
        paths.entry(id).or_insert_with(|| path.clone());
    }
    if let NodeKind::Group { children, .. } = &node.kind {
        for (index, child) in children.iter().enumerate() {
            path.push(index);
            collect_first_paths(child, path, paths);
            path.pop();
        }
    }
}

/// Ids in pre-order document order, keeping only the first occurrence.
pub(crate) fn ordered_ids(scene: &DisplayScene) -> Vec<&str> {
    let mut seen = HashSet::new();
    let mut ids = Vec::new();
    for node in &scene.nodes {
        collect_ordered_ids(node, &mut seen, &mut ids);
    }
    ids
}

fn collect_ordered_ids<'a>(node: &'a SceneNode, seen: &mut HashSet<&'a str>, ids: &mut Vec<&'a str>) {
    if let Some(id) = node.id.as_deref() {
        if seen.insert(id) {
            ids.push(id);
        }
    }
    if let NodeKind::Group { children, .. } = &node.kind {
        for child in children {
            collect_ordered_ids(child, seen, ids);
        }
    }
}
