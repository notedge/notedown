use davinci_scene::{
    Bounds, Color, MarkerShape, NodeKind, PathData, Point, Rect, Scene, SceneNode, ShapeStyle, Size, TextAnchor, TextStyle, Transform,
};

fn shape() -> ShapeStyle {
    ShapeStyle::filled(Color::BLACK)
}

fn text_style(font_size: f64, text_anchor: TextAnchor) -> TextStyle {
    TextStyle { font_size, text_anchor, ..TextStyle::default() }
}

#[test]
fn rect_bounds_follow_node_transform() {
    let node =
        SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 20.0), style: shape() }).with_transform(Transform::translate(5.0, 7.0));
    assert_eq!(node.bounds(), Some(Bounds::new(Point::new(5.0, 7.0), Point::new(15.0, 27.0))));
}

#[test]
fn group_accumulates_parent_and_child_transforms() {
    let child =
        SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 10.0), style: shape() }).with_transform(Transform::translate(5.0, 0.0));
    let group = SceneNode::group(vec![child]).with_transform(Transform::scale(2.0, 2.0));
    assert_eq!(group.bounds(), Some(Bounds::new(Point::new(10.0, 0.0), Point::new(30.0, 20.0))));
}

#[test]
fn clip_intersects_local_bounds() {
    let node =
        SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 100.0, 100.0), style: shape() }).with_clip(Rect::new(10.0, 10.0, 20.0, 20.0));
    assert_eq!(node.bounds(), Some(Bounds::new(Point::new(10.0, 10.0), Point::new(30.0, 30.0))));

    let hidden =
        SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 10.0), style: shape() }).with_clip(Rect::new(50.0, 50.0, 10.0, 10.0));
    assert_eq!(hidden.bounds(), None);
}

#[test]
fn text_bounds_respect_anchor_and_baseline() {
    let node = SceneNode::new(NodeKind::Text {
        position: Point::new(50.0, 20.0),
        text: "abcd".to_string(),
        style: text_style(10.0, TextAnchor::Middle),
    });
    assert_eq!(node.bounds(), Some(Bounds::new(Point::new(38.0, 12.0), Point::new(62.0, 22.0))));

    let start = SceneNode::new(NodeKind::Text {
        position: Point::new(50.0, 20.0),
        text: "abcd".to_string(),
        style: text_style(10.0, TextAnchor::Start),
    });
    assert_eq!(start.bounds(), Some(Bounds::new(Point::new(50.0, 12.0), Point::new(74.0, 22.0))));

    let end =
        SceneNode::new(NodeKind::Text { position: Point::new(50.0, 20.0), text: "abcd".to_string(), style: text_style(10.0, TextAnchor::End) });
    assert_eq!(end.bounds(), Some(Bounds::new(Point::new(26.0, 12.0), Point::new(50.0, 22.0))));
}

#[test]
fn marker_bounds_use_size_and_transform() {
    let node = SceneNode::new(NodeKind::Marker { at: Point::new(10.0, 10.0), shape: MarkerShape::Triangle, size: 10.0, style: shape() })
        .with_transform(Transform::translate(100.0, 100.0));
    assert_eq!(node.bounds(), Some(Bounds::new(Point::new(105.0, 105.0), Point::new(115.0, 115.0))));
}

#[test]
fn path_bounds_use_control_point_hull() {
    let path = PathData::new()
        .move_to(Point::new(0.0, 0.0))
        .line_to(Point::new(10.0, 0.0))
        .quadratic_to(Point::new(10.0, 10.0), Point::new(20.0, 10.0));
    let node = SceneNode::new(NodeKind::Path { path, style: ShapeStyle::stroked(Color::BLACK, 1.0) });
    assert_eq!(node.bounds(), Some(Bounds::new(Point::new(0.0, 0.0), Point::new(20.0, 10.0))));
}

#[test]
fn scene_bounds_union_nodes() {
    let mut scene = Scene::new(Size::new(100.0, 100.0));
    assert_eq!(scene.bounds(), None);
    scene.push(SceneNode::new(NodeKind::Rect { rect: Rect::new(10.0, 10.0, 10.0, 10.0), style: shape() }));
    scene.push(SceneNode::new(NodeKind::Ellipse { center: Point::new(50.0, 50.0), radii: Size::new(5.0, 5.0), style: shape() }));
    assert_eq!(scene.bounds(), Some(Bounds::new(Point::new(10.0, 10.0), Point::new(55.0, 55.0))));
}
