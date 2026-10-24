//! Compile-scope invalidation per action.

use std::collections::BTreeSet;

use davinci_runtime::{Action, InvalidationScope, invalidate};
use davinci_scene::Point;

fn scopes(scopes: &[InvalidationScope]) -> BTreeSet<InvalidationScope> {
    scopes.iter().copied().collect()
}

fn targets(targets: &[&str]) -> BTreeSet<String> {
    targets.iter().map(|target| (*target).to_owned()).collect()
}

#[test]
fn hover_touches_style_and_overlay_only() {
    let invalidation = invalidate(&Action::Hover(Some("a".to_owned())));

    assert_eq!(invalidation.scopes, scopes(&[InvalidationScope::Style, InvalidationScope::Overlay]));
    assert_eq!(invalidation.targets, targets(&["a"]));
    assert!(!invalidation.contains(InvalidationScope::Layout));
    assert!(!invalidation.contains(InvalidationScope::Data));
    assert!(!invalidation.contains(InvalidationScope::Motion));
}

#[test]
fn clearing_hover_has_no_semantic_target() {
    let invalidation = invalidate(&Action::Hover(None));

    assert_eq!(invalidation.scopes, scopes(&[InvalidationScope::Style, InvalidationScope::Overlay]));
    assert!(invalidation.targets.is_empty());
}

#[test]
fn focus_only_touches_style() {
    let invalidation = invalidate(&Action::Focus(Some("b".to_owned())));

    assert_eq!(invalidation.scopes, scopes(&[InvalidationScope::Style]));
    assert_eq!(invalidation.targets, targets(&["b"]));
    assert!(!invalidation.contains(InvalidationScope::Overlay));
}

#[test]
fn press_and_release_touch_pointer_layers() {
    let pressed = invalidate(&Action::Press(Some("c".to_owned())));
    assert_eq!(pressed.scopes, scopes(&[InvalidationScope::Style, InvalidationScope::Overlay]));
    assert_eq!(pressed.targets, targets(&["c"]));

    let released = invalidate(&Action::Release);
    assert_eq!(released.scopes, scopes(&[InvalidationScope::Style, InvalidationScope::Overlay]));
    assert!(released.targets.is_empty());
}

#[test]
fn selection_targets_only_need_style_and_overlay() {
    let selected = invalidate(&Action::Select { target: "d".to_owned(), additive: true });
    assert_eq!(selected.scopes, scopes(&[InvalidationScope::Style, InvalidationScope::Overlay]));
    assert_eq!(selected.targets, targets(&["d"]));

    let cleared = invalidate(&Action::ClearSelection);
    assert_eq!(cleared.scopes, scopes(&[InvalidationScope::Style, InvalidationScope::Overlay]));
    assert!(cleared.targets.is_empty());
}

#[test]
fn zoom_and_pan_only_touch_the_viewport() {
    let zoomed = invalidate(&Action::Zoom { factor: 1.5, anchor: Some(Point::new(10.0, 10.0)) });
    assert_eq!(zoomed.scopes, scopes(&[InvalidationScope::Viewport]));
    assert!(zoomed.targets.is_empty());

    let panned = invalidate(&Action::Pan { dx: 4.0, dy: 5.0 });
    assert_eq!(panned.scopes, scopes(&[InvalidationScope::Viewport]));
}

#[test]
fn resize_invalidates_layout_and_measurement() {
    let invalidation = invalidate(&Action::Resize { width: 300.0, height: 200.0 });

    assert_eq!(invalidation.scopes, scopes(&[InvalidationScope::Layout, InvalidationScope::Measurement]));
    assert!(!invalidation.contains(InvalidationScope::Data));
}

#[test]
fn parameters_reach_data_layout_and_motion() {
    let invalidation = invalidate(&Action::SetParameter { name: "zoom".to_owned(), value: 2.0 });

    assert_eq!(invalidation.scopes, scopes(&[InvalidationScope::Data, InvalidationScope::Layout, InvalidationScope::Motion]));
    assert!(!invalidation.contains(InvalidationScope::Style));
}

#[test]
fn playback_actions_only_invalidate_motion() {
    for action in [Action::Play, Action::Pause, Action::Seek(1.0), Action::Advance(0.016)] {
        let invalidation = invalidate(&action);
        assert_eq!(invalidation.scopes, scopes(&[InvalidationScope::Motion]), "{action:?}");
        assert!(!invalidation.contains(InvalidationScope::Layout), "{action:?}");
        assert!(!invalidation.contains(InvalidationScope::Data), "{action:?}");
    }
}

#[test]
fn empty_invalidation_is_reported() {
    let invalidation = davinci_runtime::Invalidation::default();

    assert!(invalidation.is_empty());
    assert!(!invalidation.contains(InvalidationScope::Style));
}
