use davinci_scene::{Color, NodeKind, Rect, Scene, SceneNode, ShapeStyle, Size, Transform};

#[test]
fn nested_group_transform_is_composed_into_svg_matrix() {
    let child = SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 10.0), style: ShapeStyle::filled(Color::BLACK) })
        .with_transform(Transform::translate(5.0, 0.0));
    let group = SceneNode::group(vec![child]).with_transform(Transform::translate(20.0, 10.0));

    let mut scene = Scene::new(Size::new(100.0, 100.0));
    scene.push(group);
    let svg = scene.to_svg().expect("valid scene");

    assert!(svg.contains("<g transform=\"matrix(1, 0, 0, 1, 20, 10)\">"), "{svg}");
    assert!(svg.contains("transform=\"matrix(1, 0, 0, 1, 5, 0)\""), "{svg}");
}

#[test]
fn rotated_node_bounds_use_transformed_hull() {
    let node = SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 2.0), style: ShapeStyle::filled(Color::BLACK) })
        .with_transform(Transform::rotate_degrees(90.0));
    let bounds = node.bounds().expect("bounds");
    assert!((bounds.min.x - -2.0).abs() < 1e-9, "{bounds:?}");
    assert!((bounds.min.y - 0.0).abs() < 1e-9, "{bounds:?}");
    assert!((bounds.max.x - 0.0).abs() < 1e-9, "{bounds:?}");
    assert!((bounds.max.y - 10.0).abs() < 1e-9, "{bounds:?}");
}
