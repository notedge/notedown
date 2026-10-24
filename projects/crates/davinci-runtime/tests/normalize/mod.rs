//! Event normalization from host input to semantic actions.

use davinci_runtime::{Action, NormalizedEvent, RuntimeState, normalize};
use davinci_scene::{NodeKind, Point, Rect, Scene, SceneNode, ShapeStyle, Size};

fn scene() -> Scene {
    let mut scene = Scene::new(Size::new(100.0, 100.0));
    scene.push(SceneNode::new(NodeKind::Rect { rect: Rect::new(0.0, 0.0, 10.0, 10.0), style: ShapeStyle::default() }).with_id("a"));
    scene
}

fn hovering(id: &str) -> RuntimeState {
    RuntimeState { hover: Some(id.to_owned()), ..RuntimeState::default() }
}

#[test]
fn pointer_move_emits_hover_only_when_the_target_changes() {
    let state = RuntimeState::default();
    assert_eq!(normalize(&NormalizedEvent::PointerMove { x: 5.0, y: 5.0 }, &state, &scene()), [Action::Hover(Some("a".to_owned()))]);

    let already_hovered = hovering("a");
    assert!(normalize(&NormalizedEvent::PointerMove { x: 5.0, y: 5.0 }, &already_hovered, &scene()).is_empty());

    assert_eq!(normalize(&NormalizedEvent::PointerMove { x: 80.0, y: 80.0 }, &already_hovered, &scene()), [Action::Hover(None)]);
}

#[test]
fn pointer_enter_uses_the_host_target() {
    assert_eq!(
        normalize(&NormalizedEvent::PointerEnter { target: "b".to_owned() }, &RuntimeState::default(), &scene()),
        [Action::Hover(Some("b".to_owned()))]
    );
}

#[test]
fn pointer_leave_clears_hover() {
    let state = hovering("a");

    assert_eq!(normalize(&NormalizedEvent::PointerLeave, &state, &scene()), [Action::Hover(None)]);
}

#[test]
fn pointer_down_resolves_the_semantic_target_from_coordinates() {
    let actions = normalize(&NormalizedEvent::PointerDown { target: None, x: 5.0, y: 5.0 }, &RuntimeState::default(), &scene());

    assert_eq!(actions, [Action::Press(Some("a".to_owned()))]);
}

#[test]
fn pointer_down_prefers_an_explicit_target() {
    let actions =
        normalize(&NormalizedEvent::PointerDown { target: Some("explicit".to_owned()), x: 5.0, y: 5.0 }, &RuntimeState::default(), &scene());

    assert_eq!(actions, [Action::Press(Some("explicit".to_owned()))]);
}

#[test]
fn pointer_down_on_empty_canvas_emits_nothing() {
    let actions = normalize(&NormalizedEvent::PointerDown { target: None, x: 90.0, y: 90.0 }, &RuntimeState::default(), &scene());

    assert!(actions.is_empty());
}

#[test]
fn pointer_up_releases_the_press() {
    assert_eq!(normalize(&NormalizedEvent::PointerUp, &RuntimeState::default(), &scene()), [Action::Release]);
}

#[test]
fn focus_blur_and_toggle_map_to_semantic_actions() {
    assert_eq!(
        normalize(&NormalizedEvent::Focus { target: "a".to_owned() }, &RuntimeState::default(), &scene()),
        [Action::Focus(Some("a".to_owned()))]
    );
    assert_eq!(normalize(&NormalizedEvent::Blur, &RuntimeState::default(), &scene()), [Action::Focus(None)]);
    assert_eq!(
        normalize(&NormalizedEvent::Toggle { target: "a".to_owned() }, &RuntimeState::default(), &scene()),
        [Action::Select { target: "a".to_owned(), additive: true }]
    );
}

/// Extract the factor and anchor of a single zoom action.
fn zoom(action: &Action) -> (f64, Option<Point>) {
    match action {
        Action::Zoom { factor, anchor } => (*factor, *anchor),
        other => panic!("expected a zoom action, got {other:?}"),
    }
}

#[test]
fn wheel_zoom_is_anchored_at_the_pointer() {
    let zoom_in = normalize(&NormalizedEvent::Wheel { delta: -120.0, x: 30.0, y: 40.0 }, &RuntimeState::default(), &scene());
    let zoom_out = normalize(&NormalizedEvent::Wheel { delta: 120.0, x: 30.0, y: 40.0 }, &RuntimeState::default(), &scene());

    assert_eq!(zoom_in.len(), 1);
    assert_eq!(zoom_out.len(), 1);

    let (in_factor, in_anchor) = zoom(&zoom_in[0]);
    let (out_factor, out_anchor) = zoom(&zoom_out[0]);

    assert_eq!(in_anchor, Some(Point::new(30.0, 40.0)));
    assert_eq!(out_anchor, Some(Point::new(30.0, 40.0)));
    assert!(in_factor > 1.0, "wheel up should zoom in, got {in_factor}");
    assert!(out_factor < 1.0, "wheel down should zoom out, got {out_factor}");
    assert!((in_factor * out_factor - 1.0).abs() < 1e-12);
}

#[test]
fn viewport_and_playback_events_map_to_one_action() {
    let state = RuntimeState::default();
    let scene = scene();

    assert_eq!(
        normalize(&NormalizedEvent::Resize { width: 640.0, height: 480.0 }, &state, &scene),
        [Action::Resize { width: 640.0, height: 480.0 }]
    );
    assert_eq!(normalize(&NormalizedEvent::Play, &state, &scene), [Action::Play]);
    assert_eq!(normalize(&NormalizedEvent::Pause, &state, &scene), [Action::Pause]);
    assert_eq!(normalize(&NormalizedEvent::Seek { time: 2.5 }, &state, &scene), [Action::Seek(2.5)]);
    assert_eq!(normalize(&NormalizedEvent::Tick { delta: 0.016 }, &state, &scene), [Action::Advance(0.016)]);
}

#[test]
fn normalize_never_mutates_its_inputs() {
    let state = hovering("a");
    let snapshot = state.clone();
    let scene = scene();
    let scene_snapshot = scene.clone();

    let _ = normalize(&NormalizedEvent::PointerMove { x: 80.0, y: 80.0 }, &state, &scene);

    assert_eq!(state, snapshot);
    assert_eq!(scene, scene_snapshot);
}
