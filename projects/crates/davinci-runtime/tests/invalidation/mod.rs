//! Layer invalidation per action and set operations.

use std::collections::BTreeSet;

use davinci_runtime::{Action, Invalidation, InvalidationScope, InvalidationSet, invalidate};
use davinci_scene::Point;

fn set(scopes: &[InvalidationScope]) -> InvalidationSet {
    InvalidationSet::of(scopes.iter().copied())
}

fn targets(targets: &[&str]) -> BTreeSet<String> {
    targets.iter().map(|target| (*target).to_owned()).collect()
}

#[test]
fn hover_only_repaints_the_overlay() {
    let invalidation = invalidate(&Action::Hover(Some("a".to_owned())));

    assert_eq!(invalidation.scopes, set(&[InvalidationScope::Overlay]));
    assert_eq!(invalidation.targets, targets(&["a"]));
    for scope in [
        InvalidationScope::Style,
        InvalidationScope::Viewport,
        InvalidationScope::Measurement,
        InvalidationScope::Layout,
        InvalidationScope::Data,
        InvalidationScope::Motion,
    ] {
        assert!(!invalidation.contains(scope), "hover must not invalidate {scope:?}");
    }
}

#[test]
fn clearing_hover_has_no_semantic_target() {
    let invalidation = invalidate(&Action::Hover(None));

    assert_eq!(invalidation.scopes, set(&[InvalidationScope::Overlay]));
    assert!(invalidation.targets.is_empty());
}

#[test]
fn focus_press_and_release_stay_on_the_overlay() {
    let focused = invalidate(&Action::Focus(Some("b".to_owned())));
    assert_eq!(focused.scopes, set(&[InvalidationScope::Overlay]));
    assert_eq!(focused.targets, targets(&["b"]));

    let pressed = invalidate(&Action::Press(Some("c".to_owned())));
    assert_eq!(pressed.scopes, set(&[InvalidationScope::Overlay]));
    assert_eq!(pressed.targets, targets(&["c"]));

    let released = invalidate(&Action::Release);
    assert_eq!(released.scopes, set(&[InvalidationScope::Overlay]));
    assert!(!released.contains(InvalidationScope::Layout));
    assert!(released.targets.is_empty());
}

#[test]
fn selection_changes_only_repaint_the_overlay() {
    let selected = invalidate(&Action::Select { target: "d".to_owned(), additive: true });
    assert_eq!(selected.scopes, set(&[InvalidationScope::Overlay]));
    assert_eq!(selected.targets, targets(&["d"]));
    assert!(!selected.contains(InvalidationScope::Data));

    let cleared = invalidate(&Action::ClearSelection);
    assert_eq!(cleared.scopes, set(&[InvalidationScope::Overlay]));
    assert!(!cleared.contains(InvalidationScope::Layout));
    assert!(cleared.targets.is_empty());
}

#[test]
fn zoom_and_pan_only_touch_the_viewport() {
    let zoomed = invalidate(&Action::Zoom { factor: 1.5, anchor: Some(Point::new(10.0, 10.0)) });
    assert_eq!(zoomed.scopes, set(&[InvalidationScope::Viewport]));
    assert!(!zoomed.contains(InvalidationScope::Layout));
    assert!(!zoomed.contains(InvalidationScope::Data));

    let panned = invalidate(&Action::Pan { dx: 4.0, dy: 5.0 });
    assert_eq!(panned.scopes, set(&[InvalidationScope::Viewport]));
    assert!(!panned.contains(InvalidationScope::Data));
}

#[test]
fn resize_invalidates_measurement_and_layout() {
    let invalidation = invalidate(&Action::Resize { width: 300.0, height: 200.0 });

    assert_eq!(invalidation.scopes, set(&[InvalidationScope::Measurement, InvalidationScope::Layout]));
    assert!(!invalidation.contains(InvalidationScope::Data));
    assert!(!invalidation.contains(InvalidationScope::Overlay));
}

#[test]
fn parameters_reach_data_through_the_closure() {
    let invalidation = invalidate(&Action::SetParameter { name: "zoom".to_owned(), value: 2.0 });

    assert_eq!(
        invalidation.scopes,
        set(&[InvalidationScope::Data, InvalidationScope::Measurement, InvalidationScope::Layout, InvalidationScope::Motion])
    );
    assert!(invalidation.contains(InvalidationScope::Layout), "data changes must relayout");
    assert!(!invalidation.contains(InvalidationScope::Style));
    assert!(!invalidation.contains(InvalidationScope::Overlay));
}

#[test]
fn theme_selection_only_invalidates_style() {
    let invalidation = invalidate(&Action::SetTheme(Some("night".to_owned())));

    assert_eq!(invalidation.scopes, set(&[InvalidationScope::Style]));
    assert!(!invalidation.contains(InvalidationScope::Layout));
    assert!(!invalidation.contains(InvalidationScope::Measurement));
    assert!(!invalidation.contains(InvalidationScope::Data));
}

#[test]
fn playback_actions_only_invalidate_motion() {
    for action in [Action::Play, Action::Pause, Action::Seek(1.0), Action::Advance(0.016)] {
        let invalidation = invalidate(&action);
        assert_eq!(invalidation.scopes, set(&[InvalidationScope::Motion]), "{action:?}");
        assert!(!invalidation.contains(InvalidationScope::Layout), "{action:?}");
        assert!(!invalidation.contains(InvalidationScope::Data), "{action:?}");
        assert!(!invalidation.contains(InvalidationScope::Measurement), "{action:?}");
    }
}

#[test]
fn sets_express_multiple_layers_and_union() {
    let pointer = set(&[InvalidationScope::Overlay]);
    let viewport = set(&[InvalidationScope::Viewport, InvalidationScope::Style]);

    let merged = pointer.union(&viewport);
    assert_eq!(merged.len(), 3);
    for scope in [InvalidationScope::Overlay, InvalidationScope::Viewport, InvalidationScope::Style] {
        assert!(merged.contains(scope), "{scope:?}");
    }
    assert!(!merged.contains(InvalidationScope::Data));
    assert_eq!(pointer.len(), 1, "union must not mutate its inputs");

    let mut accumulated = pointer.clone();
    accumulated.union_with(&viewport);
    assert_eq!(accumulated, merged);
}

#[test]
fn closure_propagates_only_documented_dependencies() {
    assert_eq!(
        set(&[InvalidationScope::Data]).closure(),
        set(&[InvalidationScope::Data, InvalidationScope::Measurement, InvalidationScope::Layout])
    );
    assert_eq!(set(&[InvalidationScope::Measurement]).closure(), set(&[InvalidationScope::Measurement, InvalidationScope::Layout]));
    assert_eq!(set(&[InvalidationScope::Layout]).closure(), set(&[InvalidationScope::Layout]));

    let independent =
        set(&[InvalidationScope::Overlay, InvalidationScope::Style, InvalidationScope::Viewport, InvalidationScope::Motion]).closure();
    assert_eq!(independent.len(), 4, "overlay, style, viewport, and motion imply nothing");
    assert!(!independent.contains(InvalidationScope::Layout));
    assert!(!independent.contains(InvalidationScope::Measurement));
}

#[test]
fn empty_invalidation_is_reported() {
    let invalidation = Invalidation::default();

    assert!(invalidation.is_empty());
    assert!(!invalidation.contains(InvalidationScope::Style));
    assert!(InvalidationSet::empty().is_empty());
}
