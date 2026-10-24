//! Interpolation, enter/exit policy, determinism, and boundary tests.

use std::collections::BTreeSet;

use davinci_motion::{Channel, Easing, EnterPolicy, ExitPolicy, MotionIntent, plan_transition, sample};
use davinci_scene::Transform;

use crate::common::{find, group, linear_intent, opacity, rect, rect_at, scene, text, x};

#[test]
fn interpolates_transform_and_opacity() {
    let previous = scene(vec![rect_at("a", 0.0, 0.0, 1.0)]);
    let next = scene(vec![rect_at("a", 10.0, 20.0, 0.5)]);
    let intent = linear_intent(1.0);
    let plan = plan_transition(&previous, &next, &intent);
    assert_eq!(plan.tracks.len(), 2);

    let at_start = sample(&plan, &previous, &next, 0.0);
    assert_eq!(x(&at_start, "a"), 0.0);
    assert_eq!(find(&at_start, "a").unwrap().transform.f(), 0.0);
    assert_eq!(opacity(find(&at_start, "a").unwrap()), 1.0);

    let at_middle = sample(&plan, &previous, &next, 0.5);
    assert_eq!(x(&at_middle, "a"), 5.0);
    assert_eq!(find(&at_middle, "a").unwrap().transform.f(), 10.0);
    assert_eq!(opacity(find(&at_middle, "a").unwrap()), 0.75);

    let at_end = sample(&plan, &previous, &next, 1.0);
    assert_eq!(find(&at_end, "a").unwrap().transform, Transform::translate(10.0, 20.0));
    assert_eq!(opacity(find(&at_end, "a").unwrap()), 0.5);
}

#[test]
fn omits_unchanged_channels() {
    let previous = scene(vec![rect_at("a", 10.0, 0.0, 1.0)]);
    let next = scene(vec![rect_at("a", 10.0, 0.0, 1.0)]);
    let plan = plan_transition(&previous, &next, &linear_intent(1.0));
    assert_eq!(plan.correspondences.len(), 1);
    assert!(plan.tracks.is_empty());

    let next = scene(vec![rect_at("a", 30.0, 0.0, 0.5)]);
    let intent = MotionIntent { channels: BTreeSet::from([Channel::Transform]), ..linear_intent(1.0) };
    let plan = plan_transition(&previous, &next, &intent);
    assert_eq!(plan.tracks.len(), 1);
    assert_eq!(plan.tracks[0].channel, Channel::Transform);
}

#[test]
fn uses_the_first_occurrence_of_a_duplicate_id() {
    let previous = scene(vec![rect_at("dup", 0.0, 0.0, 1.0)]);
    let next = scene(vec![rect_at("dup", 50.0, 0.0, 1.0), rect_at("dup", 10.0, 0.0, 1.0)]);
    let plan = plan_transition(&previous, &next, &linear_intent(1.0));

    let middle = sample(&plan, &previous, &next, 0.5);
    assert_eq!(x(&middle, "dup"), 25.0);
}

#[test]
fn easing_matches_documented_formulas() {
    assert_eq!(Easing::Linear.apply(0.25), 0.25);
    assert_eq!(Easing::EaseIn.apply(0.5), 0.25);
    assert_eq!(Easing::EaseOut.apply(0.5), 0.75);
    assert_eq!(Easing::EaseInOut.apply(0.25), 0.125);
    assert_eq!(Easing::EaseInOut.apply(0.5), 0.5);
    assert_eq!(Easing::EaseInOut.apply(0.75), 0.875);
}

#[test]
fn easing_clamps_and_hits_endpoints() {
    for easing in [Easing::Linear, Easing::EaseIn, Easing::EaseOut, Easing::EaseInOut] {
        assert_eq!(easing.apply(-1.0), 0.0);
        assert_eq!(easing.apply(0.0), 0.0);
        assert_eq!(easing.apply(1.0), 1.0);
        assert_eq!(easing.apply(2.0), 1.0);
    }
}

#[test]
fn easing_is_monotonic() {
    for easing in [Easing::Linear, Easing::EaseIn, Easing::EaseOut, Easing::EaseInOut] {
        let mut previous = easing.apply(0.0);
        for step in 1..=100 {
            let current = easing.apply(f64::from(step) / 100.0);
            assert!(current >= previous, "{easing:?} decreased at step {step}");
            previous = current;
        }
    }
}

#[test]
fn applies_non_default_easing_to_updates() {
    let previous = scene(vec![rect_at("a", 0.0, 0.0, 1.0)]);
    let next = scene(vec![rect_at("a", 10.0, 0.0, 0.0)]);
    let intent = MotionIntent { ease: Easing::EaseIn, ..linear_intent(1.0) };
    let plan = plan_transition(&previous, &next, &intent);

    let middle = sample(&plan, &previous, &next, 0.5);
    assert_eq!(x(&middle, "a"), 2.5);
    assert_eq!(opacity(find(&middle, "a").unwrap()), 0.75);

    let start = sample(&plan, &previous, &next, 0.0);
    assert_eq!(x(&start, "a"), 0.0);
    assert_eq!(opacity(find(&start, "a").unwrap()), 1.0);

    let end = sample(&plan, &previous, &next, 1.0);
    assert_eq!(x(&end, "a"), 10.0);
    assert_eq!(opacity(find(&end, "a").unwrap()), 0.0);
}

#[test]
fn enter_fade_starts_transparent_and_enter_snap_is_immediate() {
    let previous = scene(vec![rect("old", 0.0, 1.0)]);
    let next = scene(vec![rect("new", 0.0, 1.0)]);
    let intent = MotionIntent { enter: EnterPolicy::Fade, exit: ExitPolicy::Snap, ..linear_intent(1.0) };
    let plan = plan_transition(&previous, &next, &intent);

    for (time, expected) in [(0.0, 0.0), (0.25, 0.25), (0.5, 0.5), (1.0, 1.0)] {
        let sampled = sample(&plan, &previous, &next, time);
        assert_eq!(opacity(find(&sampled, "new").unwrap()), expected, "fade at {time}");
        assert!(find(&sampled, "old").is_none(), "snapped exit at {time}");
    }

    let intent = MotionIntent { enter: EnterPolicy::Snap, exit: ExitPolicy::Snap, ..linear_intent(1.0) };
    let plan = plan_transition(&previous, &next, &intent);
    for time in [0.0, 0.5, 1.0] {
        let sampled = sample(&plan, &previous, &next, time);
        assert_eq!(opacity(find(&sampled, "new").unwrap()), 1.0, "snap at {time}");
    }
}

#[test]
fn enter_fade_multiplies_group_opacity_only_at_the_entering_root() {
    let previous = scene(vec![rect("old", 0.0, 1.0)]);
    let next = scene(vec![group("g", 0.8, vec![text("label", 1.0)])]);
    let intent = MotionIntent { enter: EnterPolicy::Fade, exit: ExitPolicy::Snap, ..linear_intent(1.0) };
    let plan = plan_transition(&previous, &next, &intent);

    let start = sample(&plan, &previous, &next, 0.0);
    assert_eq!(opacity(find(&start, "g").unwrap()), 0.0);
    assert_eq!(opacity(find(&start, "label").unwrap()), 1.0);

    let middle = sample(&plan, &previous, &next, 0.5);
    assert_eq!(opacity(find(&middle, "g").unwrap()), 0.4);

    let end = sample(&plan, &previous, &next, 1.0);
    assert_eq!(opacity(find(&end, "g").unwrap()), 0.8);
}

#[test]
fn exit_fade_retains_then_removes_the_previous_subtree() {
    let previous = scene(vec![rect("gone", 0.0, 0.5)]);
    let next = scene(vec![rect("stay", 0.0, 1.0)]);
    let intent = MotionIntent { enter: EnterPolicy::Snap, exit: ExitPolicy::Fade, ..linear_intent(1.0) };
    let plan = plan_transition(&previous, &next, &intent);

    assert_eq!(opacity(find(&sample(&plan, &previous, &next, 0.0), "gone").unwrap()), 0.5);
    assert_eq!(opacity(find(&sample(&plan, &previous, &next, 0.5), "gone").unwrap()), 0.25);
    assert!(find(&sample(&plan, &previous, &next, 1.0), "gone").is_none());
}

#[test]
fn exit_snap_removes_immediately_and_exit_keep_persists() {
    let previous = scene(vec![rect("gone", 0.0, 1.0)]);
    let next = scene(vec![rect("stay", 0.0, 1.0)]);

    let snap_intent = MotionIntent { enter: EnterPolicy::Snap, exit: ExitPolicy::Snap, ..linear_intent(1.0) };
    let snap_plan = plan_transition(&previous, &next, &snap_intent);
    for time in [0.0, 0.5, 1.0] {
        assert!(find(&sample(&snap_plan, &previous, &next, time), "gone").is_none(), "snap at {time}");
    }

    let keep_intent = MotionIntent { enter: EnterPolicy::Snap, exit: ExitPolicy::Keep, ..linear_intent(1.0) };
    let keep_plan = plan_transition(&previous, &next, &keep_intent);
    for time in [0.0, 0.5, 1.0] {
        let sampled = sample(&keep_plan, &previous, &next, time);
        assert_eq!(opacity(find(&sampled, "gone").unwrap()), 1.0, "keep at {time}");
        assert_eq!(x(&sampled, "gone"), 0.0, "keep position at {time}");
    }
}

#[test]
fn exit_subtree_is_reattached_to_its_matched_group() {
    let previous = scene(vec![group("g", 1.0, vec![rect("a", 0.0, 1.0), rect("b", 0.0, 1.0)])]);
    let next = scene(vec![group("g", 1.0, vec![rect("a", 10.0, 1.0)])]);
    let intent = MotionIntent { enter: EnterPolicy::Snap, exit: ExitPolicy::Fade, ..linear_intent(1.0) };
    let plan = plan_transition(&previous, &next, &intent);

    let start = sample(&plan, &previous, &next, 0.0);
    assert_eq!(start.nodes.len(), 1);
    assert_eq!(opacity(find(&start, "b").unwrap()), 1.0);
}

#[test]
fn interpolates_nested_group_children_by_id() {
    let previous = scene(vec![group("g", 1.0, vec![rect("a", 0.0, 1.0), rect("b", 0.0, 1.0)])]);
    let next = scene(vec![group("g", 1.0, vec![rect("b", 40.0, 1.0), rect("a", 20.0, 1.0)])]);
    let plan = plan_transition(&previous, &next, &linear_intent(1.0));

    let middle = sample(&plan, &previous, &next, 0.5);
    assert_eq!(x(&middle, "a"), 10.0);
    assert_eq!(x(&middle, "b"), 20.0);
    assert_eq!(find(&middle, "g").unwrap().transform, Transform::IDENTITY);
}

#[test]
fn sampling_and_planning_are_deterministic() {
    let previous = scene(vec![group("g", 0.8, vec![rect("a", 0.0, 1.0), rect("gone", 0.0, 1.0)]), rect("solo", 0.0, 1.0)]);
    let next = scene(vec![group("g", 0.5, vec![rect("a", 30.0, 0.4), rect("new", 0.0, 1.0)])]);
    let intent = linear_intent(0.8);

    let first_plan = plan_transition(&previous, &next, &intent);
    let second_plan = plan_transition(&previous, &next, &intent);
    assert_eq!(first_plan, second_plan);

    for time in [0.0, 0.1, 0.4, 0.8] {
        assert_eq!(sample(&first_plan, &previous, &next, time), sample(&second_plan, &previous, &next, time), "time {time}");
    }
}

#[test]
fn zero_duration_returns_the_final_scene() {
    let previous = scene(vec![rect("a", 0.0, 1.0), rect("gone", 0.0, 1.0)]);
    let next = scene(vec![rect("a", 10.0, 1.0), rect("new", 0.0, 1.0)]);
    let plan = plan_transition(&previous, &next, &linear_intent(0.0));

    for time in [0.0, 0.5, 5.0] {
        assert_eq!(sample(&plan, &previous, &next, time), next, "time {time}");
    }
}

#[test]
fn time_is_clamped_to_the_duration() {
    let previous = scene(vec![rect("a", 0.0, 1.0)]);
    let next = scene(vec![rect("a", 10.0, 1.0)]);
    let plan = plan_transition(&previous, &next, &linear_intent(1.0));

    assert_eq!(sample(&plan, &previous, &next, -1.0), sample(&plan, &previous, &next, 0.0));
    assert_eq!(sample(&plan, &previous, &next, 4.0), sample(&plan, &previous, &next, 1.0));
    assert_eq!(x(&sample(&plan, &previous, &next, -1.0), "a"), 0.0);
    assert_eq!(x(&sample(&plan, &previous, &next, 4.0), "a"), 10.0);
}
