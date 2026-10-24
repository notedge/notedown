//! Resolution tests: identity preservation, transform flattening, clip
//! inheritance, and SVG equivalence.

use davinci_scene::{
    DisplayScene, NodeKind, PathData, Point, Rect, ResourceRef, SceneNode, SceneResolver, SemanticKind, SemanticNode, SemanticRole,
    SemanticScene, ShapeStyle, Size, SourceRef, Transform,
};

fn resolver() -> SceneResolver {
    SceneResolver::default()
}

fn find<'a>(scene: &'a DisplayScene, id: &str) -> &'a SceneNode {
    fn walk<'a>(node: &'a SceneNode, id: &str) -> Option<&'a SceneNode> {
        if node.id.as_deref() == Some(id) {
            return Some(node);
        }
        if let NodeKind::Group { children, .. } = &node.kind {
            return children.iter().find_map(|child| walk(child, id));
        }
        None
    }
    scene.nodes.iter().find_map(|node| walk(node, id)).unwrap_or_else(|| panic!("missing display node `{id}`"))
}

fn rect_node(id: &str, rect: Rect, transform: Transform) -> SemanticNode {
    SemanticNode::new(SemanticKind::Rect { rect, style: ShapeStyle::filled(davinci_scene::Color::BLACK) }).with_id(id).with_transform(transform)
}

#[test]
fn resolution_preserves_identity_source_and_hit_region() {
    let mut scene = SemanticScene::new(Size::new(200.0, 100.0));
    scene.push(
        SemanticNode::new(SemanticKind::Image {
            rect: Rect::new(0.0, 0.0, 20.0, 10.0),
            resource: ResourceRef::new("assets/a.png").with_mime("image/png"),
            opacity: 0.75,
        })
        .with_id("image-a")
        .with_role(SemanticRole::Node)
        .with_source(SourceRef::element("A").with_span(3, 9))
        .with_hit_region(Rect::new(-1.0, -1.0, 22.0, 12.0))
        .with_transform(Transform::translate(10.0, 20.0)),
    );

    let display = scene.resolve(&resolver());
    let node = find(&display, "image-a");
    assert_eq!(node.id.as_deref(), Some("image-a"));
    assert_eq!(node.role, Some(SemanticRole::Node));
    assert_eq!(node.source.as_ref().and_then(|source| source.element.as_deref()), Some("A"));
    assert_eq!(node.source.as_ref().and_then(|source| source.span), Some((3, 9)));
    assert_eq!(node.hit_region, Some(Rect::new(-1.0, -1.0, 22.0, 12.0)));
    assert_eq!(node.transform, Transform::translate(10.0, 20.0));
    assert!(matches!(&node.kind, NodeKind::Image { resource, opacity, .. } if resource.href == "assets/a.png" && *opacity == 0.75));

    assert_eq!(scene.resolve(&resolver()), display, "resolution must be deterministic");
}

#[test]
fn nested_group_transforms_compose_into_absolute_node_transforms() {
    let mut scene = SemanticScene::new(Size::new(100.0, 100.0));
    let inner = SemanticNode::group(vec![rect_node("box", Rect::new(0.0, 0.0, 10.0, 10.0), Transform::IDENTITY)])
        .with_id("inner")
        .with_transform(Transform::translate(5.0, 0.0));
    scene.push(
        SemanticNode::group(vec![inner])
            .with_id("outer")
            .with_transform(Transform::translate(20.0, 10.0))
            .with_hit_region(Rect::new(0.0, 0.0, 10.0, 10.0)),
    );

    let display = scene.resolve(&resolver());
    let outer = find(&display, "outer");
    let inner = find(&display, "inner");
    let box_node = find(&display, "box");

    assert_eq!(outer.transform, Transform::IDENTITY, "groups become identity containers");
    assert_eq!(inner.transform, Transform::IDENTITY, "nested groups become identity containers");
    assert_eq!(box_node.transform, Transform::new(1.0, 0.0, 0.0, 1.0, 25.0, 10.0));
    assert_eq!(outer.hit_region, Some(Rect::new(20.0, 10.0, 10.0, 10.0)));
    assert_eq!(display.bounds().map(|bounds| (bounds.min, bounds.max)), Some((Point::new(25.0, 10.0), Point::new(35.0, 20.0))));
}

#[test]
fn clips_are_inherited_and_rewritten_into_node_local_coordinates() {
    let mut scene = SemanticScene::new(Size::new(200.0, 200.0));
    let leaf = rect_node("leaf", Rect::new(0.0, 0.0, 4.0, 4.0), Transform::translate(5.0, 5.0));
    let inner = SemanticNode::group(vec![leaf]).with_id("inner").with_clip(Rect::new(10.0, 10.0, 50.0, 50.0));
    scene.push(
        SemanticNode::group(vec![inner])
            .with_id("outer")
            .with_clip(Rect::new(0.0, 0.0, 100.0, 100.0))
            .with_transform(Transform::translate(0.0, 0.0)),
    );

    let display = scene.resolve(&resolver());
    assert_eq!(find(&display, "outer").clip, Some(Rect::new(0.0, 0.0, 100.0, 100.0)));
    assert_eq!(find(&display, "inner").clip, Some(Rect::new(10.0, 10.0, 50.0, 50.0)));
    assert_eq!(find(&display, "leaf").clip, Some(Rect::new(5.0, 5.0, 50.0, 50.0)));
}

#[test]
fn resolved_svg_matches_the_equivalent_flat_display_tree() {
    let path = PathData::new().move_to(Point::new(0.0, 0.0)).line_to(Point::new(10.0, 0.0));
    let mut scene = SemanticScene::new(Size::new(100.0, 100.0));
    scene.push(
        SemanticNode::group(vec![
            SemanticNode::group(vec![
                SemanticNode::new(SemanticKind::Path { path: path.clone(), style: ShapeStyle::stroked(davinci_scene::Color::BLACK, 1.0) })
                    .with_id("line")
                    .with_role(SemanticRole::Edge),
            ])
            .with_id("inner")
            .with_transform(Transform::translate(5.0, 0.0)),
        ])
        .with_id("outer")
        .with_transform(Transform::translate(20.0, 10.0)),
    );

    let resolved = scene.resolve(&resolver());
    let svg = resolved.to_svg().expect("resolved scene should serialize");
    assert!(svg.contains("matrix(1, 0, 0, 1, 25, 10)"), "{svg}");
    assert_eq!(find(&resolved, "line").transform, Transform::new(1.0, 0.0, 0.0, 1.0, 25.0, 10.0));

    let mut flat = DisplayScene::new(Size::new(100.0, 100.0));
    flat.push(
        SceneNode::group(vec![
            SceneNode::group(vec![
                SceneNode::new(NodeKind::Path { path, style: ShapeStyle::stroked(davinci_scene::Color::BLACK, 1.0) })
                    .with_id("line")
                    .with_role(SemanticRole::Edge)
                    .with_transform(Transform::translate(25.0, 10.0)),
            ])
            .with_id("inner"),
        ])
        .with_id("outer"),
    );
    assert_eq!(svg, flat.to_svg().expect("flat scene should serialize"));
}
