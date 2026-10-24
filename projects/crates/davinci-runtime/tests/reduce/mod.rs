//! Reducer purity, pointer state, selection, viewport, and playback behavior.

use std::collections::BTreeSet;

use davinci_runtime::{
    Action, InvalidationScope, InvalidationSet, MAX_SCALE, MIN_SCALE, Playback, RuntimeState, Viewport, reduce, reduce_update,
};
use davinci_scene::Point;

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "expected {expected}, got {actual}");
}

#[test]
fn reduce_never_mutates_its_input() {
    let state = RuntimeState {
        hover: Some("hovered".to_owned()),
        playback: Playback { playing: true, ..Playback::default() },
        ..RuntimeState::default()
    };
    let snapshot = state.clone();

    let next = reduce(&state, &Action::Select { target: "node".to_owned(), additive: false });

    assert_eq!(state, snapshot);
    assert_eq!(next.selection.iter().map(String::as_str).collect::<Vec<_>>(), ["node"]);
}

#[test]
fn pointer_state_transitions_track_each_slot() {
    let state = RuntimeState::default();

    let hovered = reduce(&state, &Action::Hover(Some("a".to_owned())));
    assert_eq!(hovered.hover.as_deref(), Some("a"));
    assert_eq!(hovered.focus, None);
    assert_eq!(hovered.pressed, None);

    let focused = reduce(&hovered, &Action::Focus(Some("b".to_owned())));
    assert_eq!(focused.hover.as_deref(), Some("a"));
    assert_eq!(focused.focus.as_deref(), Some("b"));

    let pressed = reduce(&focused, &Action::Press(Some("c".to_owned())));
    assert_eq!(pressed.pressed.as_deref(), Some("c"));
    assert_eq!(pressed.focus.as_deref(), Some("b"));

    let released = reduce(&pressed, &Action::Release);
    assert_eq!(released.pressed, None);
    assert_eq!(released.hover.as_deref(), Some("a"));
    assert_eq!(released.focus.as_deref(), Some("b"));
}

#[test]
fn release_leaves_the_selection_alone() {
    let mut state = RuntimeState { pressed: Some("p".to_owned()), ..RuntimeState::default() };
    state.selection.insert("a".to_owned());

    let next = reduce(&state, &Action::Release);

    assert_eq!(next.pressed, None);
    assert_eq!(next.selection.iter().map(String::as_str).collect::<Vec<_>>(), ["a"]);
}

#[test]
fn selection_replaces_and_clears() {
    let mut state = RuntimeState::default();
    state.selection.extend(["a".to_owned(), "b".to_owned()]);

    let replaced = reduce(&state, &Action::Select { target: "c".to_owned(), additive: false });
    assert_eq!(replaced.selection.iter().map(String::as_str).collect::<Vec<_>>(), ["c"]);

    let cleared = reduce(&replaced, &Action::ClearSelection);
    assert!(cleared.selection.is_empty());
}

#[test]
fn additive_selection_toggles_membership() {
    let mut state = RuntimeState::default();
    state.selection.insert("a".to_owned());

    let added = reduce(&state, &Action::Select { target: "b".to_owned(), additive: true });
    assert_eq!(added.selection.iter().map(String::as_str).collect::<Vec<_>>(), ["a", "b"]);

    let removed = reduce(&added, &Action::Select { target: "a".to_owned(), additive: true });
    assert_eq!(removed.selection.iter().map(String::as_str).collect::<Vec<_>>(), ["b"]);
}

#[test]
fn selection_order_is_stable() {
    let state = RuntimeState::default();
    let first = reduce(&state, &Action::Select { target: "c".to_owned(), additive: false });
    let second = reduce(&first, &Action::Select { target: "a".to_owned(), additive: true });
    let third = reduce(&second, &Action::Select { target: "b".to_owned(), additive: true });

    assert_eq!(third.selection.iter().map(String::as_str).collect::<Vec<_>>(), ["a", "b", "c"]);
}

#[test]
fn zoom_keeps_the_anchored_scene_point_fixed() {
    let state = RuntimeState {
        viewport: Viewport { width: 800.0, height: 600.0, scale: 2.0, translate: Point::new(30.0, -10.0) },
        ..RuntimeState::default()
    };
    let anchor = Point::new(200.0, 150.0);
    let world_before = state.viewport.screen_to_world(anchor);

    let next = reduce(&state, &Action::Zoom { factor: 1.8, anchor: Some(anchor) });
    let world_after = next.viewport.screen_to_world(anchor);

    assert_close(next.viewport.scale, 3.6);
    assert_close(world_after.x, world_before.x);
    assert_close(world_after.y, world_before.y);
    assert_close(next.viewport.world_to_screen(world_before).x, anchor.x);
    assert_close(next.viewport.world_to_screen(world_before).y, anchor.y);
}

#[test]
fn zoom_without_anchor_only_changes_scale() {
    let state = RuntimeState {
        viewport: Viewport { width: 800.0, height: 600.0, scale: 2.0, translate: Point::new(11.0, 22.0) },
        ..RuntimeState::default()
    };

    let next = reduce(&state, &Action::Zoom { factor: 0.5, anchor: None });

    assert_close(next.viewport.scale, 1.0);
    assert_eq!(next.viewport.translate, Point::new(11.0, 22.0));
}

#[test]
fn zoom_scale_is_clamped_to_the_documented_range() {
    let state = RuntimeState::default();

    let zoomed_in = reduce(&state, &Action::Zoom { factor: 1e9, anchor: None });
    assert_close(zoomed_in.viewport.scale, MAX_SCALE);

    let zoomed_out = reduce(&state, &Action::Zoom { factor: 1e-9, anchor: None });
    assert_close(zoomed_out.viewport.scale, MIN_SCALE);
}

#[test]
fn invalid_zoom_factors_are_ignored() {
    let mut state = RuntimeState::default();
    state.viewport.scale = 2.0;

    for factor in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let next = reduce(&state, &Action::Zoom { factor, anchor: Some(Point::new(1.0, 1.0)) });
        assert_close(next.viewport.scale, 2.0);
        assert_eq!(next.viewport.translate, Point::ZERO);
    }
}

#[test]
fn pan_accumulates_translation() {
    let state = RuntimeState::default();

    let first = reduce(&state, &Action::Pan { dx: 10.0, dy: -4.0 });
    let second = reduce(&first, &Action::Pan { dx: 5.0, dy: 9.0 });

    assert_eq!(second.viewport.translate, Point::new(15.0, 5.0));
    assert_close(second.viewport.scale, 1.0);
}

#[test]
fn resize_only_changes_viewport_extents() {
    let state =
        RuntimeState { hover: Some("kept".to_owned()), viewport: Viewport { scale: 3.0, ..Viewport::default() }, ..RuntimeState::default() };

    let next = reduce(&state, &Action::Resize { width: 640.0, height: -20.0 });

    assert_close(next.viewport.width, 640.0);
    assert_close(next.viewport.height, 0.0);
    assert_close(next.viewport.scale, 3.0);
    assert_eq!(next.hover.as_deref(), Some("kept"));
}

#[test]
fn parameters_are_inserted_and_overwritten() {
    let state = RuntimeState::default();

    let first = reduce(&state, &Action::SetParameter { name: "zoom".to_owned(), value: 1.5 });
    assert_eq!(first.parameters.get("zoom"), Some(&1.5));

    let second = reduce(&first, &Action::SetParameter { name: "zoom".to_owned(), value: 2.5 });
    assert_eq!(second.parameters.get("zoom"), Some(&2.5));
    assert_eq!(state.parameters.get("zoom"), None);
}

#[test]
fn advance_does_nothing_while_paused() {
    let mut state = RuntimeState::default();
    state.playback.rate = 2.0;

    let next = reduce(&state, &Action::Advance(5.0));

    assert_close(next.playback.time, 0.0);
}

#[test]
fn advance_uses_playback_rate_while_playing() {
    let mut state = RuntimeState::default();
    state.playback.playing = true;
    state.playback.rate = 2.0;

    let next = reduce(&state, &Action::Advance(0.5));

    assert_close(next.playback.time, 1.0);
}

#[test]
fn seek_clamps_negative_time() {
    let state = RuntimeState::default();

    assert_close(reduce(&state, &Action::Seek(-3.0)).playback.time, 0.0);
    assert_close(reduce(&state, &Action::Seek(4.5)).playback.time, 4.5);
}

#[test]
fn play_and_pause_only_change_the_playing_flag() {
    let mut state = RuntimeState::default();
    state.playback.time = 3.0;

    let playing = reduce(&state, &Action::Play);
    assert!(playing.playback.playing);
    assert_close(playing.playback.time, 3.0);

    let paused = reduce(&playing, &Action::Pause);
    assert!(!paused.playback.playing);
    assert_close(paused.playback.time, 3.0);
}

#[test]
fn reduce_update_carries_state_and_layers() {
    let state = RuntimeState::default();

    let update = reduce_update(&state, &Action::Hover(Some("a".to_owned())));

    assert_eq!(update.state.hover.as_deref(), Some("a"));
    assert_eq!(update.invalidation, InvalidationSet::of([InvalidationScope::Overlay]));
    assert_eq!(update.targets, BTreeSet::from(["a".to_owned()]));
    assert_eq!(state, RuntimeState::default(), "reduce_update must not mutate its input");
}

#[test]
fn reduce_update_names_the_released_target_from_the_state_diff() {
    let state = RuntimeState { pressed: Some("p".to_owned()), ..RuntimeState::default() };

    let update = reduce_update(&state, &Action::Release);

    assert_eq!(update.state.pressed, None);
    assert_eq!(update.targets, BTreeSet::from(["p".to_owned()]));
    assert_eq!(update.invalidation, InvalidationSet::of([InvalidationScope::Overlay]));
}

#[test]
fn reduce_update_reports_cleared_selection_members() {
    let mut state = RuntimeState::default();
    state.selection.extend(["a".to_owned(), "b".to_owned()]);

    let update = reduce_update(&state, &Action::ClearSelection);

    assert!(update.state.selection.is_empty());
    assert_eq!(update.targets, BTreeSet::from(["a".to_owned(), "b".to_owned()]));
}

#[test]
fn no_op_actions_have_no_invalidation() {
    let paused = RuntimeState::default();
    let advanced = reduce_update(&paused, &Action::Advance(1.0));
    assert!(advanced.invalidation.is_empty());
    assert!(advanced.targets.is_empty());

    let hovered = RuntimeState { hover: Some("a".to_owned()), ..RuntimeState::default() };
    let repeated = reduce_update(&hovered, &Action::Hover(Some("a".to_owned())));
    assert!(repeated.invalidation.is_empty());
    assert!(repeated.targets.is_empty());
    assert_eq!(repeated.state, hovered);
}

#[test]
fn parameter_changes_carry_the_closed_data_chain() {
    let state = RuntimeState::default();

    let update = reduce_update(&state, &Action::SetParameter { name: "brush".to_owned(), value: 0.5 });

    assert_eq!(
        update.invalidation,
        InvalidationSet::of([InvalidationScope::Data, InvalidationScope::Measurement, InvalidationScope::Layout, InvalidationScope::Motion,])
    );
    assert_eq!(update.state.parameters.get("brush"), Some(&0.5));
}
