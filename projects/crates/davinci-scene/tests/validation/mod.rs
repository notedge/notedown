use davinci_scene::{Color, DisplayScene, NodeKind, Point, Rect, SceneNode, ShapeStyle, Size, TextStyle, Transform};
use davinci_types::codes;

fn scene_with(node: SceneNode) -> DisplayScene {
    let mut scene = DisplayScene::new(Size::new(100.0, 100.0));
    scene.push(node);
    scene
}

#[test]
fn non_finite_geometry_is_rejected() {
    let scene = scene_with(SceneNode::new(NodeKind::Line {
        from: Point::new(f64::NAN, 0.0),
        to: Point::new(1.0, 1.0),
        style: ShapeStyle::filled(Color::BLACK),
    }));
    let error = scene.validate().expect_err("nan point");
    assert_eq!(error.code(), codes::INVALID_GEOMETRY);
    assert!(error.message().contains("from.x"), "{}", error.message());
}

#[test]
fn negative_rect_and_radius_are_rejected() {
    let rect = scene_with(SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, -1.0, 10.0), style: ShapeStyle::filled(Color::BLACK) }));
    assert_eq!(rect.validate().expect_err("negative width").code(), codes::INVALID_GEOMETRY);

    let rounded = scene_with(SceneNode::new(NodeKind::RoundedRect {
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        radius: -2.0,
        style: ShapeStyle::filled(Color::BLACK),
    }));
    assert_eq!(rounded.validate().expect_err("negative radius").code(), codes::INVALID_GEOMETRY);
}

#[test]
fn invalid_opacity_and_font_size_are_rejected() {
    let opacity = scene_with(SceneNode::new(NodeKind::Rect {
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        style: ShapeStyle { opacity: 2.0, ..ShapeStyle::default() },
    }));
    assert_eq!(opacity.validate().expect_err("opacity").code(), codes::INVALID_STYLE);

    let font = scene_with(SceneNode::new(NodeKind::Text {
        position: Point::new(0.0, 0.0),
        text: "x".to_string(),
        style: TextStyle { font_size: 0.0, ..TextStyle::default() },
    }));
    assert_eq!(font.validate().expect_err("font size").code(), codes::INVALID_STYLE);
}

#[test]
fn non_finite_transform_is_rejected() {
    let scene = scene_with(
        SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 10.0), style: ShapeStyle::filled(Color::BLACK) })
            .with_transform(Transform::new(f64::INFINITY, 0.0, 0.0, 1.0, 0.0, 0.0)),
    );
    assert_eq!(scene.validate().expect_err("transform").code(), codes::INVALID_TRANSFORM);
}

#[test]
fn scene_error_exposes_structured_diagnostic() {
    let scene = scene_with(SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, -1.0), style: ShapeStyle::filled(Color::BLACK) }));
    let error = scene.to_svg().expect_err("negative height");
    let record = error.diagnostic();
    assert_eq!(record.code().as_str(), codes::INVALID_GEOMETRY);
    assert_eq!(record.origin().namespace(), "davinci");
    assert_eq!(record.origin().component(), "scene");
    assert_eq!(record.message().fallback(), Some(error.message()));
}
