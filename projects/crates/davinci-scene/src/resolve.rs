//! Deterministic resolution from semantic scenes into display scenes.
//!
//! Resolution composes nested transforms into absolute node transforms,
//! inherits clips down the tree, and keeps group identity and paint order.
//! It never performs layout, text measurement, or I/O, so equal semantic
//! scenes always produce equal display scenes.

use davinci_types::{Bounds, Point, Rect, Transform};

use crate::{
    DisplayScene, NodeKind, SceneNode,
    semantic::{SemanticKind, SemanticNode, SemanticScene},
};

/// Resolver configuration for semantic-to-display resolution.
///
/// Resolution is deterministic and currently configuration-free. The type
/// exists so call sites name the resolution step explicitly and future
/// invariant checks have a stable home.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SceneResolver;

impl SceneResolver {
    /// Create a resolver with the default rules.
    pub const fn new() -> Self {
        Self
    }

    /// Resolve a semantic scene into backend-ready display geometry.
    ///
    /// Nested transforms are composed into each node's transform relative to
    /// the scene root. Groups stay in the tree as identity containers so paint
    /// order, opacity, and correspondence ids survive. Clips are intersected
    /// with every ancestor clip and converted into each node's local
    /// coordinates. Rotated clips are represented by their axis-aligned hull.
    pub fn resolve(&self, source: &SemanticScene) -> DisplayScene {
        let frame = Frame { world: Transform::IDENTITY, clip: None };
        let nodes = source.nodes.iter().map(|node| resolve_node(node, &frame)).collect();
        DisplayScene { size: source.size, nodes }
    }
}

impl SemanticScene {
    /// Resolve this semantic scene with the given resolver.
    pub fn resolve(&self, resolver: &SceneResolver) -> DisplayScene {
        resolver.resolve(self)
    }
}

struct Frame {
    /// The node's parent frame composed from the scene root.
    world: Transform,
    /// Clip inherited from ancestors in scene coordinates.
    clip: Option<Rect>,
}

fn resolve_node(node: &SemanticNode, parent: &Frame) -> SceneNode {
    let world = node.transform.then(parent.world);
    let own_clip = node.clip.map(|clip| rect_hull(clip, world));
    let inherited_clip = intersect_clip(parent.clip, own_clip);

    match &node.kind {
        SemanticKind::Group { children, opacity } => {
            let child_frame = Frame { world, clip: inherited_clip };
            let mut display = SceneNode::new(NodeKind::Group {
                children: children.iter().map(|child| resolve_node(child, &child_frame)).collect(),
                opacity: *opacity,
            });
            display.transform = Transform::IDENTITY;
            display.clip = inherited_clip;
            display.hit_region = node.hit_region.map(|region| rect_hull(region, world));
            apply_identity(&mut display, node);
            display
        }
        _ => {
            let mut display = SceneNode::new(resolve_kind(&node.kind));
            display.transform = world;
            display.clip = inherited_clip.and_then(|clip| inverse_rect(clip, world)).and_then(|clip| match node.clip {
                Some(own) => intersect_rect(clip, own),
                None => Some(clip),
            });
            display.hit_region = node.hit_region;
            apply_identity(&mut display, node);
            display
        }
    }
}

fn apply_identity(display: &mut SceneNode, node: &SemanticNode) {
    display.id = node.id.clone();
    display.role = node.role;
    display.source = node.source.clone();
}

fn resolve_kind(kind: &SemanticKind) -> NodeKind {
    match kind {
        SemanticKind::Group { .. } => unreachable!("groups are resolved by `resolve_node`"),
        SemanticKind::Path { path, style } => NodeKind::Path { path: path.clone(), style: style.clone() },
        SemanticKind::Line { from, to, style } => NodeKind::Line { from: *from, to: *to, style: style.clone() },
        SemanticKind::Rect { rect, style } => NodeKind::Rect { rect: *rect, style: style.clone() },
        SemanticKind::RoundedRect { rect, radius, style } => NodeKind::RoundedRect { rect: *rect, radius: *radius, style: style.clone() },
        SemanticKind::Ellipse { center, radii, style } => NodeKind::Ellipse { center: *center, radii: *radii, style: style.clone() },
        SemanticKind::Text { position, text, style } => NodeKind::Text { position: *position, text: text.clone(), style: style.clone() },
        SemanticKind::Image { rect, resource, opacity } => NodeKind::Image { rect: *rect, resource: resource.clone(), opacity: *opacity },
        SemanticKind::Marker { at, shape, size, style } => NodeKind::Marker { at: *at, shape: *shape, size: *size, style: style.clone() },
    }
}

fn rect_hull(rect: Rect, transform: Transform) -> Rect {
    let corners = [
        Point::new(rect.x, rect.y),
        Point::new(rect.right(), rect.y),
        Point::new(rect.right(), rect.bottom()),
        Point::new(rect.x, rect.bottom()),
    ];
    let mut min = Point::new(f64::INFINITY, f64::INFINITY);
    let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for corner in corners {
        let point = transform.apply_point(corner);
        min = Point::new(min.x.min(point.x), min.y.min(point.y));
        max = Point::new(max.x.max(point.x), max.y.max(point.y));
    }
    Rect::new(min.x, min.y, max.x - min.x, max.y - min.y)
}

fn inverse_rect(rect: Rect, transform: Transform) -> Option<Rect> {
    let determinant = transform.determinant();
    if determinant == 0.0 {
        return None;
    }
    let a = transform.d();
    let b = -transform.b();
    let c = -transform.c();
    let d = transform.a();
    let e = transform.c() * transform.f() - transform.d() * transform.e();
    let f = transform.b() * transform.e() - transform.a() * transform.f();
    let inverse = Transform::new(a / determinant, b / determinant, c / determinant, d / determinant, e / determinant, f / determinant);
    Some(rect_hull(rect, inverse))
}

fn intersect_clip(left: Option<Rect>, right: Option<Rect>) -> Option<Rect> {
    match (left, right) {
        (Some(left), Some(right)) => intersect_rect(left, right),
        (Some(clip), None) | (None, Some(clip)) => Some(clip),
        (None, None) => None,
    }
}

fn intersect_rect(left: Rect, right: Rect) -> Option<Rect> {
    let bounds = Bounds::new(Point::new(left.x, left.y), Point::new(left.right(), left.bottom()))
        .intersection(Bounds::new(Point::new(right.x, right.y), Point::new(right.right(), right.bottom())))?;
    Some(Rect::new(bounds.min.x, bounds.min.y, bounds.max.x - bounds.min.x, bounds.max.y - bounds.min.y))
}
