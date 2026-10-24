//! Hit testing order, transforms, clips, and semantic targets.

use davinci_runtime::hit_test;
use davinci_scene::{NodeKind, Point, Rect, Scene, SceneNode, SemanticRole, ShapeStyle, Size, SourceRef, Transform};

fn rect_node(id: Option<&str>, x: f64, y: f64, width: f64, height: f64) -> SceneNode {
    let node = SceneNode::new(NodeKind::Rect { rect: Rect::new(x, y, width, height), style: ShapeStyle::default() });
    match id {
        Some(id) => node.with_id(id),
        None => node,
    }
}

fn empty_scene() -> Scene {
    Scene::new(Size::new(200.0, 200.0))
}

fn hit_id(scene: &Scene, point: Point) -> Option<String> {
    hit_test(scene, point).map(|hit| hit.id)
}

#[test]
fn later_paint_order_wins_between_roots() {
    let mut scene = empty_scene();
    scene.push(rect_node(Some("below"), 0.0, 0.0, 50.0, 50.0));
    scene.push(rect_node(Some("above"), 10.0, 10.0, 50.0, 50.0));

    assert_eq!(hit_id(&scene, Point::new(20.0, 20.0)).as_deref(), Some("above"));
    assert_eq!(hit_id(&scene, Point::new(5.0, 5.0)).as_deref(), Some("below"));
    assert_eq!(hit_id(&scene, Point::new(150.0, 150.0)), None);

    let hit = hit_test(&scene, Point::new(20.0, 20.0)).expect("overlap should hit");
    assert_eq!(hit.scene_path, vec![1]);
}

#[test]
fn deepest_id_inside_a_group_wins() {
    let mut scene = empty_scene();
    let inner = rect_node(Some("child"), 2.0, 2.0, 6.0, 6.0);
    scene.push(SceneNode::group(vec![inner]).with_id("cluster"));

    let hit = hit_test(&scene, Point::new(5.0, 5.0)).expect("child should hit");
    assert_eq!(hit.id, "child");
    assert_eq!(hit.scene_path, vec![0, 0]);
}

#[test]
fn grouped_children_without_ids_report_the_ancestor() {
    let mut scene = empty_scene();
    let plain = rect_node(None, 2.0, 2.0, 6.0, 6.0);
    scene.push(SceneNode::group(vec![plain]).with_id("cluster"));

    let hit = hit_test(&scene, Point::new(5.0, 5.0)).expect("group should capture the child hit");
    assert_eq!(hit.id, "cluster");
    assert_eq!(hit.scene_path, vec![0]);
}

#[test]
fn group_clips_prune_the_subtree() {
    let mut scene = empty_scene();
    let child = rect_node(Some("child"), 0.0, 0.0, 50.0, 50.0);
    scene.push(SceneNode::group(vec![child]).with_id("cluster").with_clip(Rect::new(0.0, 0.0, 10.0, 10.0)));

    assert_eq!(hit_id(&scene, Point::new(5.0, 5.0)).as_deref(), Some("child"));
    assert_eq!(hit_id(&scene, Point::new(30.0, 30.0)), None);
}

#[test]
fn scaled_transforms_map_the_point_into_local_space() {
    let mut scene = empty_scene();
    let child = rect_node(Some("scaled"), 0.0, 0.0, 10.0, 10.0).with_transform(Transform::scale(2.0, 3.0));
    scene.push(child);

    assert_eq!(hit_id(&scene, Point::new(19.0, 29.0)).as_deref(), Some("scaled"));
    assert_eq!(hit_id(&scene, Point::new(21.0, 15.0)), None);
    assert_eq!(hit_id(&scene, Point::new(10.0, 31.0)), None);
}

#[test]
fn translated_group_transforms_compose_with_children() {
    let mut scene = empty_scene();
    let child = rect_node(Some("child"), 0.0, 0.0, 10.0, 10.0);
    scene.push(SceneNode::group(vec![child]).with_id("group").with_transform(Transform::translate(100.0, 50.0)));

    assert_eq!(hit_id(&scene, Point::new(105.0, 55.0)).as_deref(), Some("child"));
    assert_eq!(hit_id(&scene, Point::new(5.0, 5.0)), None);
}

#[test]
fn rotated_geometry_is_not_hit_by_its_axis_aligned_hull() {
    let mut scene = empty_scene();
    let rotated = rect_node(Some("rotated"), 0.0, 0.0, 100.0, 100.0).with_transform(Transform::rotate_degrees(45.0));
    scene.push(rotated);

    // The 100x100 square rotates into a diamond whose center lands at (0, 70.71).
    assert_eq!(hit_id(&scene, Point::new(0.0, 70.71)).as_deref(), Some("rotated"));
    // This point is inside the diamond's axis-aligned hull but outside the diamond.
    assert_eq!(hit_id(&scene, Point::new(-60.0, 100.0)), None);
}

#[test]
fn hit_regions_expand_and_shrink_the_target() {
    let mut expanded_scene = empty_scene();
    expanded_scene.push(rect_node(Some("expanded"), 0.0, 0.0, 10.0, 10.0).with_hit_region(Rect::new(0.0, 0.0, 40.0, 40.0)));
    assert_eq!(hit_id(&expanded_scene, Point::new(30.0, 30.0)).as_deref(), Some("expanded"));

    let mut shrunk_scene = empty_scene();
    shrunk_scene.push(rect_node(Some("shrunk"), 0.0, 0.0, 100.0, 100.0).with_hit_region(Rect::new(0.0, 0.0, 10.0, 10.0)));
    assert_eq!(hit_id(&shrunk_scene, Point::new(5.0, 5.0)).as_deref(), Some("shrunk"));
    assert_eq!(hit_id(&shrunk_scene, Point::new(50.0, 50.0)), None);
}

#[test]
fn nodes_without_ids_do_not_block_their_neighbors() {
    let mut scene = empty_scene();
    scene.push(rect_node(Some("target"), 0.0, 0.0, 50.0, 50.0));
    scene.push(rect_node(None, 0.0, 0.0, 50.0, 50.0));

    assert_eq!(hit_id(&scene, Point::new(25.0, 25.0)).as_deref(), Some("target"));
}

#[test]
fn hit_targets_carry_role_and_source() {
    let mut scene = empty_scene();
    let node = rect_node(Some("n1"), 0.0, 0.0, 20.0, 20.0).with_role(SemanticRole::Node).with_source(SourceRef::element("n1").with_span(3, 9));
    scene.push(node);

    let hit = hit_test(&scene, Point::new(10.0, 10.0)).expect("node should hit");

    assert_eq!(hit.role, Some(SemanticRole::Node));
    assert_eq!(hit.source, Some(SourceRef::element("n1").with_span(3, 9)));
}

#[test]
fn singular_transforms_are_not_hittable() {
    let mut scene = empty_scene();
    scene.push(rect_node(Some("collapsed"), 0.0, 0.0, 10.0, 10.0).with_transform(Transform::scale(0.0, 1.0)));

    assert_eq!(hit_id(&scene, Point::new(5.0, 5.0)), None);
}

#[test]
fn empty_scenes_have_no_target() {
    let scene = empty_scene();

    assert_eq!(hit_id(&scene, Point::new(0.0, 0.0)), None);
}
