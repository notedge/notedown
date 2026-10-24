//! Incremental pass orchestration per layer set.

use davinci_runtime::{Action, InvalidationScope, InvalidationSet, Pass, RuntimeState, derive_steps, reduce_update};

fn set(scopes: &[InvalidationScope]) -> InvalidationSet {
    InvalidationSet::of(scopes.iter().copied())
}

#[test]
fn hover_only_repaints_the_overlay() {
    let hover = reduce_update(&RuntimeState::default(), &Action::Hover(Some("mark:1".to_owned())));

    assert_eq!(derive_steps(&hover.invalidation), vec![Pass::RepaintOverlay]);
}

#[test]
fn focus_press_and_selection_never_recompile_the_tree() {
    for action in
        [Action::Focus(Some("a".to_owned())), Action::Press(Some("b".to_owned())), Action::Select { target: "c".to_owned(), additive: true }]
    {
        let update = reduce_update(&RuntimeState::default(), &action);
        assert_eq!(derive_steps(&update.invalidation), vec![Pass::RepaintOverlay], "{action:?}");
    }

    let mut selected = RuntimeState::default();
    selected.selection.insert("c".to_owned());
    let cleared = reduce_update(&selected, &Action::ClearSelection);
    assert_eq!(derive_steps(&cleared.invalidation), vec![Pass::RepaintOverlay]);
}

#[test]
fn data_changes_run_data_then_measurement_then_layout() {
    assert_eq!(derive_steps(&set(&[InvalidationScope::Data])), vec![Pass::RecompileData, Pass::Remeasure, Pass::Relayout]);

    let parameter = reduce_update(&RuntimeState::default(), &Action::SetParameter { name: "x".to_owned(), value: 1.0 });
    assert_eq!(derive_steps(&parameter.invalidation), vec![Pass::RecompileData, Pass::Remeasure, Pass::Relayout, Pass::RetargetMotion]);
}

#[test]
fn viewport_changes_never_reload_data_or_relayout() {
    let zoom = reduce_update(&RuntimeState::default(), &Action::Zoom { factor: 2.0, anchor: None });
    let steps = derive_steps(&zoom.invalidation);

    assert_eq!(steps, vec![Pass::RecomputeViewport]);
    assert!(!steps.contains(&Pass::RecompileData));
    assert!(!steps.contains(&Pass::Relayout));
}

#[test]
fn resize_runs_measurement_before_layout() {
    let resize = reduce_update(&RuntimeState::default(), &Action::Resize { width: 100.0, height: 50.0 });

    assert_eq!(derive_steps(&resize.invalidation), vec![Pass::Remeasure, Pass::Relayout]);
}

#[test]
fn measurement_alone_implies_layout() {
    assert_eq!(derive_steps(&set(&[InvalidationScope::Measurement])), vec![Pass::Remeasure, Pass::Relayout]);
    assert_eq!(derive_steps(&set(&[InvalidationScope::Layout])), vec![Pass::Relayout], "layout implies nothing");
}

#[test]
fn theme_and_motion_map_to_their_own_passes() {
    let theme = reduce_update(&RuntimeState::default(), &Action::SetTheme(Some("night".to_owned())));
    assert_eq!(derive_steps(&theme.invalidation), vec![Pass::Restyle]);

    let play = reduce_update(&RuntimeState::default(), &Action::Play);
    assert_eq!(derive_steps(&play.invalidation), vec![Pass::RetargetMotion]);
}

#[test]
fn steps_follow_the_documented_pipeline_order() {
    let all = set(&[
        InvalidationScope::Overlay,
        InvalidationScope::Style,
        InvalidationScope::Viewport,
        InvalidationScope::Layout,
        InvalidationScope::Motion,
    ]);

    assert_eq!(derive_steps(&all), vec![Pass::RecomputeViewport, Pass::Relayout, Pass::Restyle, Pass::RetargetMotion, Pass::RepaintOverlay,]);
}

#[test]
fn no_layers_produce_no_steps() {
    assert!(derive_steps(&InvalidationSet::empty()).is_empty());
}
