//! Deterministic semantic scene rendering for a chart compile plan.
//!
//! Rendering only translates final plan geometry into [`SemanticScene`] nodes.
//! It never re-runs statistics, trains scales, measures text, or emits backend
//! commands. Resolution into display geometry is a separate step.

use davinci_scene::{
    Color, PathData, Point, Rect, SemanticKind, SemanticNode, SemanticRole, SemanticScene, ShapeStyle, SourceRef, TextAnchor, TextStyle,
};

use crate::plan::{AxisPlan, ChartCompilePlan, MarkGeometry, MarkInstance, MarkStyle};

const AXIS_COLOR: Color = Color::rgb(0x6b, 0x74, 0x85);
const TICK_LENGTH: f64 = 4.0;
const TEXT_BASELINE_SHIFT: f64 = 0.35;

/// Render a compile plan into a deterministic semantic scene.
///
/// Every mark instance becomes one node carrying [`SemanticRole::Mark`] (or
/// [`SemanticRole::Label`] for text marks) with its stable id and a
/// [`SourceRef`] built from the plan source descriptor. Axes become
/// [`SemanticRole::Axis`] lines and [`SemanticRole::Tick`] marks.
///
/// The returned scene is semantic only: it carries no SVG string, field
/// binding, statistic, or scale. [`SemanticScene::resolve`] produces the
/// display scene, and only that scene can serialize through
/// [`davinci_scene::DisplayScene::to_svg`].
pub fn render(plan: &ChartCompilePlan) -> SemanticScene {
    let mut scene = SemanticScene::new(plan.canvas);
    for axis in &plan.axes {
        push_axis(&mut scene, axis, plan.font_size);
    }
    for instance in &plan.marks {
        push_mark(&mut scene, instance, plan.font_size);
    }
    scene
}

fn push_mark(scene: &mut SemanticScene, instance: &MarkInstance, font_size: f64) {
    let source = SourceRef::element(instance.source.clone());
    let (role, node) = match &instance.geometry {
        MarkGeometry::Rect(rect) => {
            (SemanticRole::Mark, SemanticNode::new(SemanticKind::Rect { rect: *rect, style: shape_style(&instance.style) }))
        }
        MarkGeometry::Point { center, size, shape } => (
            SemanticRole::Mark,
            SemanticNode::new(SemanticKind::Marker { at: *center, shape: *shape, size: *size, style: shape_style(&instance.style) }),
        ),
        MarkGeometry::Polyline(points) => {
            (SemanticRole::Mark, SemanticNode::new(SemanticKind::Path { path: polyline(points), style: shape_style(&instance.style) }))
        }
        MarkGeometry::Polygon(points) => {
            (SemanticRole::Mark, SemanticNode::new(SemanticKind::Path { path: polyline(points).close(), style: shape_style(&instance.style) }))
        }
        MarkGeometry::Segment { from, to } => {
            (SemanticRole::Mark, SemanticNode::new(SemanticKind::Line { from: *from, to: *to, style: shape_style(&instance.style) }))
        }
        MarkGeometry::Label { anchor, text } => {
            let fill = instance.style.fill.unwrap_or(Color::BLACK);
            let baseline = anchor.offset(0.0, font_size * TEXT_BASELINE_SHIFT);
            (
                SemanticRole::Label,
                SemanticNode::new(SemanticKind::Text {
                    position: baseline,
                    text: text.clone(),
                    style: text_style(fill, font_size, TextAnchor::Middle),
                }),
            )
        }
    };

    let mut node = node.with_id(instance.id.clone()).with_role(role).with_source(source);
    if let Some(bounds) = geometry_bounds(&instance.geometry) {
        node = node.with_hit_region(bounds);
    }
    scene.push(node);
}

fn push_axis(scene: &mut SemanticScene, axis: &AxisPlan, font_size: f64) {
    let channel = axis.channel.as_str();
    scene.push(
        SemanticNode::new(SemanticKind::Line { from: axis.from, to: axis.to, style: ShapeStyle::stroked(AXIS_COLOR, 1.0) })
            .with_id(format!("chart-axis-{channel}"))
            .with_role(SemanticRole::Axis),
    );

    let anchor = if axis.channel == crate::spec::Channel::X { TextAnchor::Middle } else { TextAnchor::End };
    let (dx, dy) = if axis.channel == crate::spec::Channel::X { (0.0, TICK_LENGTH) } else { (-TICK_LENGTH, 0.0) };

    for (index, tick) in axis.ticks.iter().enumerate() {
        let end = tick.position.offset(dx, dy);
        scene.push(
            SemanticNode::new(SemanticKind::Line { from: tick.position, to: end, style: ShapeStyle::stroked(AXIS_COLOR, 1.0) })
                .with_id(format!("chart-{channel}-tick-{index}-mark"))
                .with_role(SemanticRole::Tick),
        );
        scene.push(
            SemanticNode::new(SemanticKind::Text {
                position: tick.label_position,
                text: tick.label.clone(),
                style: text_style(AXIS_COLOR, font_size, anchor),
            })
            .with_id(format!("chart-{channel}-tick-{index}"))
            .with_role(SemanticRole::Tick),
        );
    }
}

fn polyline(points: &[Point]) -> PathData {
    let mut path = PathData::new();
    for (index, point) in points.iter().enumerate() {
        path = if index == 0 { path.move_to(*point) } else { path.line_to(*point) };
    }
    path
}

fn shape_style(style: &MarkStyle) -> ShapeStyle {
    ShapeStyle { fill: style.fill, stroke: style.stroke, stroke_width: style.stroke_width, opacity: style.opacity }
}

fn text_style(fill: Color, font_size: f64, text_anchor: TextAnchor) -> TextStyle {
    TextStyle { fill: Some(fill), opacity: 1.0, font_family: Some("sans-serif".to_string()), font_size, font_weight: None, text_anchor }
}

fn geometry_bounds(geometry: &MarkGeometry) -> Option<Rect> {
    match geometry {
        MarkGeometry::Rect(rect) => Some(*rect),
        MarkGeometry::Point { center, size, .. } => Some(Rect::new(center.x - size * 0.5, center.y - size * 0.5, *size, *size)),
        MarkGeometry::Polyline(points) | MarkGeometry::Polygon(points) => bounds_of(points),
        MarkGeometry::Segment { from, to } => Some(Rect::from_points(*from, *to)),
        MarkGeometry::Label { .. } => None,
    }
}

fn bounds_of(points: &[Point]) -> Option<Rect> {
    let first = points.first()?;
    let mut bounds = Rect::from_points(*first, *first);
    for point in points {
        let min_x = bounds.x.min(point.x);
        let min_y = bounds.y.min(point.y);
        let max_x = bounds.right().max(point.x);
        let max_y = bounds.bottom().max(point.y);
        bounds = Rect::new(min_x, min_y, max_x - min_x, max_y - min_y);
    }
    Some(bounds)
}
