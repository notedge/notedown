//! Node-level narrowing of invalidation sets.

use std::collections::BTreeSet;

use davinci_runtime::{
    Action, InvalidationScope, InvalidationSet, RuntimeState, RuntimeUpdate, invalidated_nodes, invalidated_semantic_nodes, reduce_update,
};
use davinci_scene::{
    DisplayScene, NodeKind, Point, Rect, SceneNode, SemanticKind, SemanticNode, SemanticRole, SemanticScene, ShapeStyle, Size, SourceRef,
    TextStyle,
};

fn shape(id: Option<&str>, role: Option<SemanticRole>, source: Option<&str>) -> SceneNode {
    let mut node = SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 10.0), style: ShapeStyle::default() });
    if let Some(id) = id {
        node = node.with_id(id);
    }
    if let Some(role) = role {
        node = node.with_role(role);
    }
    if let Some(source) = source {
        node = node.with_source(SourceRef::element(source));
    }
    node
}

fn text(id: Option<&str>, role: Option<SemanticRole>) -> SceneNode {
    let mut node = SceneNode::new(NodeKind::Text { position: Point::ZERO, text: "label".to_owned(), style: TextStyle::default() });
    if let Some(id) = id {
        node = node.with_id(id);
    }
    if let Some(role) = role {
        node = node.with_role(role);
    }
    node
}

fn display_scene() -> DisplayScene {
    let plot = SceneNode::group(vec![
        shape(Some("mark:1"), Some(SemanticRole::Mark), Some("a")),
        shape(Some("mark:1/halo"), Some(SemanticRole::Decoration), None),
        shape(Some("mark:2"), Some(SemanticRole::Mark), Some("b")),
        shape(Some("label:a"), Some(SemanticRole::Label), Some("a")),
        shape(Some("axis:y"), Some(SemanticRole::Axis), None),
        shape(Some("legend"), Some(SemanticRole::Legend), None),
        text(Some("caption"), Some(SemanticRole::Label)),
    ])
    .with_id("plot")
    .with_role(SemanticRole::Region);

    let mut scene = DisplayScene::new(Size::new(120.0, 80.0));
    scene.push(plot);
    scene
}

fn update(invalidation: InvalidationSet, targets: &[&str]) -> RuntimeUpdate {
    RuntimeUpdate { state: RuntimeState::default(), invalidation, targets: targets.iter().map(|target| (*target).to_owned()).collect() }
}

fn ids(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn hover_narrows_to_the_target_its_source_and_prefixed_highlights() {
    let scene = display_scene();
    let hover = reduce_update(&RuntimeState::default(), &Action::Hover(Some("mark:1".to_owned())));

    let narrowed = invalidated_nodes(&hover, &scene);

    assert_eq!(narrowed.recompute, ids(&["mark:1", "mark:1/halo", "label:a"]));
    assert_eq!(narrowed.retained, ids(&["plot", "mark:2", "axis:y", "legend", "caption"]));
    assert!(narrowed.whole_layers.is_empty(), "a named overlay target stays narrow");
}

#[test]
fn overlay_without_targets_is_a_whole_layer() {
    let scene = display_scene();

    let narrowed = invalidated_nodes(&update(InvalidationSet::of([InvalidationScope::Overlay]), &[]), &scene);

    assert!(narrowed.recompute.is_empty(), "an unnamed overlay cannot claim specific nodes");
    assert!(narrowed.whole_layers.contains(InvalidationScope::Overlay));
    assert_eq!(narrowed.retained.len(), 8);
}

#[test]
fn style_reaches_every_identified_node() {
    let scene = display_scene();

    let narrowed = invalidated_nodes(&update(InvalidationSet::of([InvalidationScope::Style]), &[]), &scene);

    assert_eq!(narrowed.recompute.len(), 8);
    assert!(narrowed.recompute.contains("plot"));
    assert!(narrowed.recompute.contains("caption"));
    assert!(narrowed.retained.is_empty());
    assert!(narrowed.whole_layers.is_empty(), "every styleable node has an id");
}

#[test]
fn measurement_only_remeasures_text_nodes() {
    let scene = display_scene();

    let narrowed = invalidated_nodes(&update(InvalidationSet::of([InvalidationScope::Measurement]), &[]), &scene);

    assert_eq!(narrowed.recompute, ids(&["caption"]));
    assert!(narrowed.retained.contains("mark:1"));
    assert!(narrowed.retained.contains("plot"));
    assert!(narrowed.whole_layers.is_empty());
}

#[test]
fn data_reaches_data_roles_and_sourced_nodes() {
    let scene = display_scene();

    let narrowed = invalidated_nodes(&update(InvalidationSet::of([InvalidationScope::Data]), &[]), &scene);

    assert_eq!(narrowed.recompute, ids(&["mark:1", "mark:2", "label:a", "axis:y", "legend"]));
    assert_eq!(narrowed.retained, ids(&["plot", "mark:1/halo", "caption"]));
    assert!(narrowed.whole_layers.is_empty());
}

#[test]
fn global_layers_are_reported_as_whole() {
    let scene = display_scene();

    let layout = invalidated_nodes(&update(InvalidationSet::of([InvalidationScope::Layout]), &[]), &scene);
    assert_eq!(layout.recompute.len(), 8);
    assert!(layout.whole_layers.contains(InvalidationScope::Layout));
    assert!(layout.retained.is_empty());

    let viewport = invalidated_nodes(&update(InvalidationSet::of([InvalidationScope::Viewport]), &[]), &scene);
    assert!(viewport.recompute.is_empty());
    assert_eq!(viewport.retained.len(), 8, "projection does not rebuild node content");
    assert!(viewport.whole_layers.contains(InvalidationScope::Viewport));

    let motion = invalidated_nodes(&update(InvalidationSet::of([InvalidationScope::Motion]), &[]), &scene);
    assert!(motion.recompute.is_empty());
    assert!(motion.whole_layers.contains(InvalidationScope::Motion));
}

#[test]
fn unidentified_nodes_force_the_affected_layer_whole() {
    let mut scene = DisplayScene::new(Size::new(120.0, 80.0));
    scene.push(shape(Some("mark:1"), Some(SemanticRole::Mark), Some("a")));
    scene.push(shape(None, Some(SemanticRole::Mark), Some("a")));
    scene.push(text(None, Some(SemanticRole::Label)));

    let styled = invalidated_nodes(&update(InvalidationSet::of([InvalidationScope::Style]), &[]), &scene);
    assert_eq!(styled.recompute, ids(&["mark:1"]));
    assert!(styled.whole_layers.contains(InvalidationScope::Style));

    let measured = invalidated_nodes(&update(InvalidationSet::of([InvalidationScope::Measurement]), &[]), &scene);
    assert!(measured.recompute.is_empty());
    assert!(measured.whole_layers.contains(InvalidationScope::Measurement));

    let hover = reduce_update(&RuntimeState::default(), &Action::Hover(Some("mark:1".to_owned())));
    let overlay = invalidated_nodes(&hover, &scene);
    assert_eq!(overlay.recompute, ids(&["mark:1"]));
    assert!(overlay.whole_layers.contains(InvalidationScope::Overlay), "an id-less sourced node matches the target");
}

#[test]
fn empty_sets_retain_every_identified_node() {
    let scene = display_scene();

    let narrowed = invalidated_nodes(&update(InvalidationSet::empty(), &[]), &scene);

    assert!(narrowed.is_empty());
    assert_eq!(narrowed.retained.len(), 8);
    assert!(narrowed.recompute.is_empty());
}

#[test]
fn semantic_scenes_use_the_same_identity_rules() {
    let plot = SemanticNode::group(vec![
        SemanticNode::new(SemanticKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 10.0), style: ShapeStyle::default() })
            .with_id("mark:1")
            .with_role(SemanticRole::Mark)
            .with_source(SourceRef::element("a")),
        SemanticNode::new(SemanticKind::Text { position: Point::ZERO, text: "1".to_owned(), style: TextStyle::default() })
            .with_id("label:a")
            .with_role(SemanticRole::Label)
            .with_source(SourceRef::element("a")),
        SemanticNode::new(SemanticKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 10.0), style: ShapeStyle::default() })
            .with_id("mark:2")
            .with_role(SemanticRole::Mark),
    ])
    .with_id("plot")
    .with_role(SemanticRole::Region);
    let mut scene = SemanticScene::new(Size::new(120.0, 80.0));
    scene.push(plot);

    let hover = reduce_update(&RuntimeState::default(), &Action::Hover(Some("mark:1".to_owned())));
    let narrowed = invalidated_semantic_nodes(&hover, &scene);

    assert_eq!(narrowed.recompute, ids(&["mark:1", "label:a"]));
    assert_eq!(narrowed.retained, ids(&["plot", "mark:2"]));
    assert!(narrowed.whole_layers.is_empty());
}
