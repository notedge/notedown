//! Runtime state defaults and viewport projection.

use std::collections::{BTreeMap, BTreeSet};

use davinci_runtime::{MAX_SCALE, MIN_SCALE, Playback, RuntimeState, Viewport};
use davinci_scene::Point;

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "expected {expected}, got {actual}");
}

#[test]
fn default_state_is_neutral() {
    let state = RuntimeState::default();

    assert_eq!(state.hover, None);
    assert_eq!(state.focus, None);
    assert_eq!(state.pressed, None);
    assert_eq!(state.selection, BTreeSet::new());
    assert_eq!(state.parameters, BTreeMap::new());
    assert_eq!(state.viewport, Viewport { width: 0.0, height: 0.0, scale: 1.0, translate: Point::ZERO });
    assert_eq!(state.playback, Playback { playing: false, time: 0.0, rate: 1.0 });
}

#[test]
fn viewport_round_trips_screen_and_world_points() {
    let viewport = Viewport { width: 800.0, height: 600.0, scale: 2.5, translate: Point::new(-30.0, 12.0) };
    let world = Point::new(42.0, -7.0);

    let screen = viewport.world_to_screen(world);
    assert_close(screen.x, 42.0 * 2.5 - 30.0);
    assert_close(screen.y, -7.0 * 2.5 + 12.0);

    let round_tripped = viewport.screen_to_world(screen);
    assert_close(round_tripped.x, world.x);
    assert_close(round_tripped.y, world.y);
}

#[test]
fn degenerate_scale_does_not_produce_nan() {
    let viewport = Viewport { width: 10.0, height: 10.0, scale: 0.0, translate: Point::ZERO };
    let point = Point::new(4.0, 5.0);

    assert_eq!(viewport.screen_to_world(point), point);
}

#[test]
fn scale_limits_are_documented() {
    assert_eq!(MIN_SCALE, 0.05);
    assert_eq!(MAX_SCALE, 64.0);
}
