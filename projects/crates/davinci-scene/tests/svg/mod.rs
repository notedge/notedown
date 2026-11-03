use davinci_scene::{
    Color, DisplayScene, MarkerShape, NodeKind, PathData, Point, Rect, ResourceRef, SceneNode, SemanticRole, ShapeStyle, Size, SourceRef,
    TextAnchor, TextStyle, Transform,
};

fn sample_scene() -> DisplayScene {
    let mut group = SceneNode::group(vec![
        SceneNode::new(NodeKind::Rect {
            rect: Rect::new(0.0, 0.0, 120.0, 60.0),
            style: ShapeStyle { fill: Some(Color::rgb(0xd9, 0xe8, 0xff)), stroke: Some(Color::BLACK), stroke_width: 1.5, opacity: 1.0 },
        })
        .with_id("box"),
        SceneNode::new(NodeKind::Text {
            position: Point::new(60.0, 36.0),
            text: "A & B".to_string(),
            style: TextStyle {
                fill: Some(Color::BLACK),
                opacity: 1.0,
                font_family: Some("sans-serif".to_string()),
                font_size: 14.0,
                font_weight: Some(600),
                text_anchor: TextAnchor::Middle,
            },
        })
        .with_id("label"),
        SceneNode::new(NodeKind::Path {
            path: PathData::new().move_to(Point::new(120.0, 60.0)).line_to(Point::new(160.0, 90.0)),
            style: ShapeStyle::stroked(Color::BLACK, 1.0),
        }),
        SceneNode::new(NodeKind::Marker {
            at: Point::new(160.0, 90.0),
            shape: MarkerShape::Triangle,
            size: 10.0,
            style: ShapeStyle::filled(Color::BLACK),
        })
        .with_transform(Transform::rotate_degrees(26.565_051_177_078)),
        SceneNode::new(NodeKind::Ellipse {
            center: Point::new(200.0, 40.0),
            radii: Size::new(16.0, 10.0),
            style: ShapeStyle::filled(Color::rgba(255, 0, 0, 128)),
        }),
    ]);
    group.transform = Transform::translate(20.0, 10.0);

    let mut scene = DisplayScene::new(Size::new(240.0, 160.0));
    scene.push(group);
    scene
}

#[test]
fn svg_matches_golden_fixture() {
    let svg = sample_scene().to_svg().expect("valid scene");
    assert_eq!(svg, include_str!("../fixtures/scene.svg"));
}

#[test]
fn svg_output_is_byte_stable() {
    let scene = sample_scene();
    assert_eq!(scene.to_svg().expect("valid"), scene.to_svg().expect("valid"));
}

#[test]
fn text_is_escaped_and_styled() {
    let mut scene = DisplayScene::new(Size::new(100.0, 50.0));
    scene.push(SceneNode::new(NodeKind::Text {
        position: Point::new(10.0, 20.0),
        text: "<A> & \"B\"".to_string(),
        style: TextStyle { font_size: 12.0, text_anchor: TextAnchor::End, opacity: 0.5, ..TextStyle::default() },
    }));
    let svg = scene.to_svg().expect("valid scene");
    assert!(svg.contains("text-anchor=\"end\""), "{svg}");
    assert!(svg.contains("font-size=\"12\""), "{svg}");
    assert!(svg.contains("opacity=\"0.5\""), "{svg}");
    assert!(svg.contains("&lt;A&gt; &amp; &quot;B&quot;"), "{svg}");
}

#[test]
fn clips_get_stable_unique_ids() {
    let rect = Rect::new(0.0, 0.0, 20.0, 20.0);
    let first = SceneNode::new(NodeKind::Rect { rect, style: ShapeStyle::filled(Color::BLACK) }).with_clip(Rect::new(0.0, 0.0, 10.0, 10.0));
    let second = SceneNode::new(NodeKind::Rect { rect, style: ShapeStyle::filled(Color::BLACK) }).with_clip(Rect::new(2.0, 2.0, 5.0, 5.0));

    let mut scene = DisplayScene::new(Size::new(40.0, 40.0));
    scene.push(SceneNode::group(vec![first, second]));
    let svg = scene.to_svg().expect("valid scene");

    assert!(svg.contains("<clipPath id=\"clip-0\">"), "{svg}");
    assert!(svg.contains("<clipPath id=\"clip-1\">"), "{svg}");
    assert!(svg.contains("clip-path=\"url(#clip-0)\""), "{svg}");
    assert!(svg.contains("clip-path=\"url(#clip-1)\""), "{svg}");
}

#[test]
fn image_and_rounded_rect_emit_expected_attributes() {
    let mut scene = DisplayScene::new(Size::new(100.0, 100.0));
    scene.push(
        SceneNode::new(NodeKind::RoundedRect { rect: Rect::new(1.0, 2.0, 30.0, 40.0), radius: 6.0, style: ShapeStyle::filled(Color::WHITE) })
            .with_id("rounded"),
    );
    scene.push(
        SceneNode::new(NodeKind::Image {
            rect: Rect::new(5.0, 5.0, 10.0, 10.0),
            resource: ResourceRef::new("assets/a&b.png").with_mime("image/png"),
            opacity: 0.75,
        })
        .with_id("img"),
    );
    let svg = scene.to_svg().expect("valid scene");

    assert!(svg.contains("<rect id=\"rounded\" x=\"1\" y=\"2\" width=\"30\" height=\"40\" rx=\"6\" fill=\"#ffffff\"/>"), "{svg}");
    assert!(svg.contains("href=\"assets/a&amp;b.png\""), "{svg}");
    assert!(svg.contains("data-mime=\"image/png\""), "{svg}");
    assert!(svg.contains("opacity=\"0.75\""), "{svg}");
}

#[test]
fn semantic_role_and_source_emit_data_attributes() {
    let mut scene = DisplayScene::new(Size::new(100.0, 100.0));
    scene.push(
        SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 10.0), style: ShapeStyle::filled(Color::WHITE) })
            .with_id("node-a")
            .with_role(SemanticRole::Node)
            .with_source(SourceRef::element("A").with_span(3, 4))
            .with_hit_region(Rect::new(-1.0, -1.0, 12.0, 12.0)),
    );
    let svg = scene.to_svg().expect("valid scene");

    assert!(svg.contains("data-role=\"node\""), "{svg}");
    assert!(svg.contains("data-source=\"A\""), "{svg}");
    assert!(svg.contains("data-source-range=\"3-4\""), "{svg}");
    assert!(!svg.contains("hit_region"), "{svg}");
}

#[test]
fn namespaced_hooks_prefix_ids_and_scope_role_classes() {
    let build = || {
        let mut scene = DisplayScene::new(Size::new(100.0, 100.0));
        scene.push(
            SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 10.0), style: ShapeStyle::filled(Color::WHITE) })
                .with_id("node-A")
                .with_role(SemanticRole::Node)
                .with_clip(Rect::new(0.0, 0.0, 20.0, 20.0)),
        );
        scene.push(
            SceneNode::new(NodeKind::Line { from: Point::new(0.0, 0.0), to: Point::new(10.0, 10.0), style: ShapeStyle::stroked(Color::BLACK, 1.5) })
                .with_id("edge-0")
                .with_role(SemanticRole::Edge),
        );
        scene
    };

    let first = build().to_svg_with_hooks("diagram-1", "diagram", &davinci_types::DIAGRAM_THEME).expect("first scene");
    let second = build().to_svg_with_hooks("diagram-2", "diagram", &davinci_types::DIAGRAM_THEME).expect("second scene");

    // Root carries the container class, ids and clip paths are namespaced.
    assert!(first.contains(r#"<svg xmlns="http://www.w3.org/2000/svg" class="diagram""#), "{first}");
    assert!(first.contains(r#"<clipPath id="diagram-1-clip-0">"#), "{first}");
    assert!(first.contains(r#"clip-path="url(#diagram-1-clip-0)""#), "{first}");
    assert!(first.contains(r#"id="diagram-1-node-A""#), "{first}");
    assert!(first.contains(r#"class="diagram-node""#), "{first}");
    assert!(first.contains(r#"id="diagram-1-edge-0""#), "{first}");
    assert!(first.contains(r#"class="diagram-edge""#), "{first}");

    // Identical scenes under different namespaces share no id.
    assert!(second.contains(r#"id="diagram-2-node-A""#), "{second}");
    let ids = |svg: &str| {
        svg.match_indices("id=\"")
            .map(|(start, _)| {
                let rest = &svg[start + 4..];
                rest[..rest.find('"').expect("closing quote")].to_string()
            })
            .collect::<Vec<_>>()
    };
    let first_ids = ids(&first);
    let second_ids = ids(&second);
    assert!(first_ids.iter().all(|id| id.starts_with("diagram-1-")), "{first_ids:?}");
    assert!(second_ids.iter().all(|id| id.starts_with("diagram-2-")), "{second_ids:?}");
    assert!(first_ids.iter().all(|id| !second_ids.contains(id)), "namespaces must not overlap: {first_ids:?} vs {second_ids:?}");

    // No bare generic class names leak into the output.
    for bare in ["class=\"node\"", "class=\"edge\"", "class=\"label\"", "class=\"cluster\""] {
        assert!(!first.contains(bare), "bare class `{bare}` must not appear: {first}");
    }

    // The plain serializer keeps verbatim ids and emits no classes.
    let plain = build().to_svg().expect("plain scene");
    assert!(plain.contains(r#"id="node-A""#), "{plain}");
    assert!(!plain.contains("class="), "{plain}");
}

#[test]
fn ellipses_lines_and_diamond_markers_render() {
    let mut scene = DisplayScene::new(Size::new(50.0, 50.0));
    scene.push(SceneNode::new(NodeKind::Line {
        from: Point::new(0.0, 0.0),
        to: Point::new(10.0, 10.0),
        style: ShapeStyle::stroked(Color::BLACK, 2.0),
    }));
    scene.push(SceneNode::new(NodeKind::Ellipse {
        center: Point::new(20.0, 20.0),
        radii: Size::new(4.0, 2.0),
        style: ShapeStyle::filled(Color::WHITE),
    }));
    scene.push(SceneNode::new(NodeKind::Marker {
        at: Point::new(30.0, 30.0),
        shape: MarkerShape::Diamond,
        size: 8.0,
        style: ShapeStyle::filled(Color::BLACK),
    }));
    scene.push(SceneNode::new(NodeKind::Marker {
        at: Point::new(40.0, 40.0),
        shape: MarkerShape::Circle,
        size: 6.0,
        style: ShapeStyle::filled(Color::BLACK),
    }));
    scene.push(SceneNode::new(NodeKind::Group { children: Vec::new(), opacity: 0.5 }));

    let svg = scene.to_svg().expect("valid scene");
    assert!(svg.contains("<line x1=\"0\" y1=\"0\" x2=\"10\" y2=\"10\" stroke=\"#000000\" stroke-width=\"2\"/>"), "{svg}");
    assert!(svg.contains("<ellipse cx=\"20\" cy=\"20\" rx=\"4\" ry=\"2\" fill=\"#ffffff\"/>"), "{svg}");
    assert!(svg.contains("points=\"30,26 34,30 30,34 26,30\""), "{svg}");
    assert!(svg.contains("<circle cx=\"40\" cy=\"40\" r=\"3\" fill=\"#000000\"/>"), "{svg}");
    assert!(svg.contains("<g opacity=\"0.5\">"), "{svg}");
}

#[test]
fn themed_output_inlines_both_palettes_and_skips_inline_paint() {
    let mut scene = DisplayScene::new(Size::new(100.0, 100.0));
    scene.push(
        SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 10.0), style: ShapeStyle::filled(Color::WHITE) })
            .with_id("node-A")
            .with_role(SemanticRole::Node),
    );
    let svg = scene.to_svg_with_hooks("diagram-1", "diagram", &davinci_types::DIAGRAM_THEME).expect("themed scene");

    // Both palettes are inlined, switched by the media query, and every
    // selector hangs under the container class.
    assert!(svg.contains("<style>"), "{svg}");
    assert!(svg.contains("@media (prefers-color-scheme: dark)"), "{svg}");
    assert!(svg.contains(".diagram .diagram-node { fill: #ececff; stroke: #9370db }"), "{svg}");
    assert!(svg.contains(".diagram .diagram-node { fill: #2d2a4a; stroke: #8b7ec8 }"), "{svg}");
    assert!(!svg.contains("<style>.diagram-node") && !svg.contains("}.diagram-node"), "selectors must be scoped under the container class: {svg}");

    // Classed elements carry no inline paint: colors come from the theme.
    assert!(svg.contains(r#"<rect id="diagram-1-node-A" data-role="node" class="diagram-node""#), "{svg}");
    assert!(!svg.contains("fill=\"#ffffff\""), "themed elements must not inline colors: {svg}");

    // The retired dark-invisible palette is gone.
    assert!(!svg.contains("#2d3748"), "{svg}");

    // The unthemed serializer keeps full inline paint as the light fallback.
    let plain = scene.to_svg().expect("plain scene");
    assert!(plain.contains("fill=\"#ffffff\""), "{plain}");
    assert!(!plain.contains("<style>"), "{plain}");
}
