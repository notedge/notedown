//! Bounds computation for scene nodes and entire scenes.
//!
//! Transform composition is exact because node transforms are affine matrices.
//! Bounds of rotated or non-uniformly scaled geometry are the axis-aligned hull
//! of the transformed local box. Curve bounds use the control-point hull.

use davinci_types::{Bounds, Point, Rect, Transform};

use crate::{NodeKind, Scene, SceneNode, TextAnchor};

impl SceneNode {
    /// Axis-aligned bounds of this node in the parent coordinate system.
    ///
    /// The result includes the node transform and is intersected with the
    /// node clip before transformation. A node clipped completely away
    /// returns `None`.
    pub fn bounds(&self) -> Option<Bounds> {
        let local = local_bounds(&self.kind)?;
        let clipped = match self.clip {
            Some(clip) => local.intersection(Bounds::new(Point::new(clip.x, clip.y), Point::new(clip.right(), clip.bottom())))?,
            None => local,
        };
        Some(transform_bounds(clipped, self.transform))
    }
}

impl Scene {
    /// Union of all root node bounds, or `None` for an empty scene.
    pub fn bounds(&self) -> Option<Bounds> {
        let mut bounds = self.nodes.iter().filter_map(SceneNode::bounds);
        let first = bounds.next()?;
        Some(bounds.fold(first, Bounds::union))
    }
}

fn local_bounds(kind: &NodeKind) -> Option<Bounds> {
    match kind {
        NodeKind::Group { children, .. } => {
            let mut child_bounds = children.iter().filter_map(SceneNode::bounds);
            let first = child_bounds.next()?;
            Some(child_bounds.fold(first, Bounds::union))
        }
        NodeKind::Path { path, .. } => path.bounds(),
        NodeKind::Line { from, to, .. } => Some(Bounds::new(*from, *to)),
        NodeKind::Rect { rect, .. } | NodeKind::RoundedRect { rect, .. } => Some(normalized_rect_bounds(*rect)),
        NodeKind::Ellipse { center, radii, .. } => Some(Bounds::new(
            Point::new(center.x - radii.width, center.y - radii.height),
            Point::new(center.x + radii.width, center.y + radii.height),
        )),
        NodeKind::Text { position, text, style } => Some(text_bounds(*position, text, style.font_size, style.text_anchor)),
        NodeKind::Image { rect, .. } => Some(normalized_rect_bounds(*rect)),
        NodeKind::Marker { at, size, .. } => {
            let half = size * 0.5;
            Some(Bounds::new(Point::new(at.x - half, at.y - half), Point::new(at.x + half, at.y + half)))
        }
    }
}

fn normalized_rect_bounds(rect: Rect) -> Bounds {
    Bounds::new(Point::new(rect.x, rect.y), Point::new(rect.right(), rect.bottom()))
}

/// Bounds of a single text line using a deterministic width approximation.
///
/// The width is `char_count * font_size * 0.6`, and the block spans
/// `0.8 * font_size` above and `0.2 * font_size` below the baseline. This is
/// stable across hosts but not a substitute for font shaping, so hosts that
/// need exact text extents must measure it themselves.
fn text_bounds(position: Point, text: &str, font_size: f64, anchor: TextAnchor) -> Bounds {
    let width = text.chars().count() as f64 * font_size * 0.6;
    let left = match anchor {
        TextAnchor::Start => position.x,
        TextAnchor::Middle => position.x - width * 0.5,
        TextAnchor::End => position.x - width,
    };
    Bounds::new(Point::new(left, position.y - font_size * 0.8), Point::new(left + width, position.y + font_size * 0.2))
}

fn transform_bounds(bounds: Bounds, transform: Transform) -> Bounds {
    let corners = [
        transform.apply_point(bounds.min),
        transform.apply_point(Point::new(bounds.max.x, bounds.min.y)),
        transform.apply_point(bounds.max),
        transform.apply_point(Point::new(bounds.min.x, bounds.max.y)),
    ];
    corners.iter().skip(1).fold(Bounds::from_point(corners[0]), |acc, point| acc.union(Bounds::from_point(*point)))
}
