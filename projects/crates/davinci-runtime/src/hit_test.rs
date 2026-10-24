//! Pointer hit testing against scene geometry.

use davinci_scene::{Bounds, DisplayScene, NodeKind, Point, Rect, SceneNode, SemanticRole, SourceRef, TextAnchor, Transform};

/// A semantic object resolved from a pointer position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HitTarget {
    /// Stable identity of the hit node.
    pub id: String,
    /// Domain meaning declared by the node, when present.
    pub role: Option<SemanticRole>,
    /// Source provenance declared by the node, when present.
    pub source: Option<SourceRef>,
    /// Child indices from the scene root to the hit node, in paint order.
    ///
    /// The last index is the hit node itself. Siblings are visited from last to
    /// first, which is exactly reverse paint order, so the path also proves
    /// which sibling won.
    pub scene_path: Vec<usize>,
}

/// Resolve the topmost semantic target at a scene point.
///
/// Siblings are visited in reverse paint order, so a later node wins over an
/// earlier one. Groups recurse into their children before testing themselves:
/// the deepest node with an `id` inside the winning subtree is reported, and an
/// id-less ancestor receives the hit when no descendant carries an id. A node
/// without an `id` is never reported, but testing continues around it.
///
/// The point is mapped into each node's local space with the exact inverse of
/// that node's affine transform, so rotated and scaled geometry is tested
/// against its real local box rather than the axis-aligned hull of its world
/// footprint. Clips prune a node and all of its descendants.
///
/// A node's [`SceneNode::hit_region`] replaces its painted geometry as the
/// pointer target for that node only; it does not clip children. Hit regions
/// and clips are both in the node's local coordinates.
///
/// Returns `None` when no node with an `id` is under the point.
#[must_use]
pub fn hit_test(scene: &DisplayScene, point: Point) -> Option<HitTarget> {
    let mut path = Vec::new();
    for (index, node) in scene.nodes.iter().enumerate().rev() {
        path.push(index);
        if let Some(target) = hit_node(node, point, &mut path) {
            return Some(target);
        }
        path.pop();
    }
    None
}

/// Test one node and its subtree. `parent_point` is expressed in the coordinate
/// space this node's transform maps from.
fn hit_node(node: &SceneNode, parent_point: Point, path: &mut Vec<usize>) -> Option<HitTarget> {
    let local_point = inverse(node.transform)?.apply_point(parent_point);
    let clipped_out = node.clip.is_some_and(|clip| !rect_contains(clip, local_point));
    if clipped_out {
        return None;
    }
    if let NodeKind::Group { children, .. } = &node.kind {
        for (index, child) in children.iter().enumerate().rev() {
            path.push(index);
            if let Some(target) = hit_node(child, local_point, path) {
                return Some(target);
            }
            path.pop();
        }
    }
    if !hits_self(node, local_point) {
        return None;
    }
    Some(HitTarget { id: node.id.clone()?, role: node.role, source: node.source.clone(), scene_path: path.clone() })
}

/// Whether the node's own pointer region covers a point in its local space.
fn hits_self(node: &SceneNode, local_point: Point) -> bool {
    match node.hit_region {
        Some(region) => rect_contains(region, local_point),
        None => local_geometry_bounds(&node.kind).is_some_and(|bounds| bounds.contains(local_point)),
    }
}

/// Local-space geometry bounds of a node's kind, excluding its own transform,
/// clip, and hit region.
///
/// `davinci-scene` keeps this helper private, so the runtime mirrors it to test
/// exact local geometry instead of a transformed bounding box. Path curves use
/// the control-point hull and text uses the documented width approximation of
/// the scene crate, so both remain rectangular approximations. Group bounds are
/// the union of child bounds, which already include child transforms and clips.
fn local_geometry_bounds(kind: &NodeKind) -> Option<Bounds> {
    match kind {
        NodeKind::Group { children, .. } => {
            let mut bounds = children.iter().filter_map(SceneNode::bounds);
            let first = bounds.next()?;
            Some(bounds.fold(first, Bounds::union))
        }
        NodeKind::Path { path, .. } => path.bounds(),
        NodeKind::Line { from, to, .. } => Some(Bounds::new(*from, *to)),
        NodeKind::Rect { rect, .. } | NodeKind::RoundedRect { rect, .. } => Some(rect_bounds(*rect)),
        NodeKind::Ellipse { center, radii, .. } => Some(Bounds::new(
            Point::new(center.x - radii.width, center.y - radii.height),
            Point::new(center.x + radii.width, center.y + radii.height),
        )),
        NodeKind::Text { position, text, style } => Some(text_bounds(*position, text, style.font_size, style.text_anchor)),
        NodeKind::Image { rect, .. } => Some(rect_bounds(*rect)),
        NodeKind::Marker { at, size, .. } => {
            let half = size * 0.5;
            Some(Bounds::new(Point::new(at.x - half, at.y - half), Point::new(at.x + half, at.y + half)))
        }
    }
}

/// Normalized bounds of a rectangle, so negative extents still describe a
/// region the constructor order cannot contradict.
fn rect_bounds(rect: Rect) -> Bounds {
    Bounds::new(Point::new(rect.x, rect.y), Point::new(rect.right(), rect.bottom()))
}

/// Inclusive containment test that tolerates negative rectangle extents.
fn rect_contains(rect: Rect, point: Point) -> bool {
    rect_bounds(rect).contains(point)
}

/// Bounds of a single text line.
///
/// Mirrors the deterministic approximation of `davinci-scene`: width is
/// `char_count * font_size * 0.6`, and the block spans `0.8 * font_size` above
/// and `0.2 * font_size` below the baseline. Real text shaping is the host's
/// responsibility, so both crates agree on this rectangle.
fn text_bounds(position: Point, text: &str, font_size: f64, anchor: TextAnchor) -> Bounds {
    let width = text.chars().count() as f64 * font_size * 0.6;
    let left = match anchor {
        TextAnchor::Start => position.x,
        TextAnchor::Middle => position.x - width * 0.5,
        TextAnchor::End => position.x - width,
    };
    Bounds::new(Point::new(left, position.y - font_size * 0.8), Point::new(left + width, position.y + font_size * 0.2))
}

/// Invert an affine transform.
///
/// `davinci_types::Transform` exposes its matrix components but no inverse, so
/// the runtime keeps a private inverse here. A singular transform collapses its
/// geometry to a line or point and is therefore never hittable.
fn inverse(transform: Transform) -> Option<Transform> {
    const SINGULAR: f64 = 1e-12;

    let determinant = transform.determinant();
    if !determinant.is_finite() || determinant.abs() < SINGULAR {
        return None;
    }
    let (a, b, c, d, e, f) = (transform.a(), transform.b(), transform.c(), transform.d(), transform.e(), transform.f());
    Some(Transform::new(
        d / determinant,
        -b / determinant,
        -c / determinant,
        a / determinant,
        (c * f - d * e) / determinant,
        (b * e - a * f) / determinant,
    ))
}
