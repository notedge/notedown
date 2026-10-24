//! Timeline tests: advancing, sampling, completion, and interruption.

use davinci_motion::{InterruptionPolicy, Timeline, plan_transition, sample};
use davinci_scene::DisplayScene;

use crate::common::{find, linear_intent, rect, scene, x};

fn pair(start: f64, end: f64) -> (DisplayScene, DisplayScene) {
    (scene(vec![rect("a", start, 1.0)]), scene(vec![rect("a", end, 1.0)]))
}

#[test]
fn advance_accumulates_and_clamps() {
    let (previous, next) = pair(0.0, 100.0);
    let plan = plan_transition(&previous, &next, &linear_intent(1.0));
    let mut timeline = Timeline::new(previous.clone(), plan.clone(), next.clone());
    assert_eq!(timeline.elapsed, 0.0);
    assert!(!timeline.is_complete());

    let first = timeline.advance(0.25);
    assert_eq!(timeline.elapsed, 0.25);
    assert_eq!(x(&first, "a"), 25.0);
    assert_eq!(first, sample(&plan, &previous, &next, 0.25));

    timeline.advance(0.25);
    assert_eq!(timeline.elapsed, 0.5);

    timeline.advance(10.0);
    assert_eq!(timeline.elapsed, 1.0);
    assert!(timeline.is_complete());
    assert_eq!(x(&timeline.scene_at(1.0), "a"), 100.0);

    timeline.advance(-10.0);
    assert_eq!(timeline.elapsed, 0.0);
    assert!(!timeline.is_complete());
    assert_eq!(x(&timeline.scene_at(0.0), "a"), 0.0);
}

#[test]
fn scene_at_does_not_change_elapsed_time() {
    let (previous, next) = pair(0.0, 100.0);
    let plan = plan_transition(&previous, &next, &linear_intent(1.0));
    let timeline = Timeline::new(previous.clone(), plan.clone(), next.clone());

    assert_eq!(x(&timeline.scene_at(0.75), "a"), 75.0);
    assert_eq!(timeline.elapsed, 0.0);
    assert_eq!(timeline.scene_at(0.75), sample(&plan, &previous, &next, 0.75));
}

#[test]
fn zero_duration_timeline_is_complete() {
    let (previous, next) = pair(0.0, 100.0);
    let plan = plan_transition(&previous, &next, &linear_intent(0.0));
    let mut timeline = Timeline::new(previous, plan, next.clone());

    assert!(timeline.is_complete());
    assert_eq!(timeline.advance(5.0), next);
    assert_eq!(timeline.elapsed, 0.0);
}

#[test]
fn cancel_and_restart_restarts_the_supplied_plan() {
    let (previous, target) = pair(0.0, 100.0);
    let mut plan = plan_transition(&previous, &target, &linear_intent(1.0));
    plan.intent.interruption = InterruptionPolicy::CancelAndRestart;
    let mut timeline = Timeline::new(previous, plan, target);
    let current = timeline.advance(0.5);
    assert_eq!(x(&current, "a"), 50.0);

    let (next_start, next_end) = pair(100.0, 200.0);
    let mut next_plan = plan_transition(&next_start, &next_end, &linear_intent(1.0));
    next_plan.intent.interruption = InterruptionPolicy::CancelAndRestart;

    let returned = timeline.interrupt(next_plan.clone(), next_end.clone(), current.clone());
    assert_eq!(timeline.elapsed, 0.0);
    assert_eq!(timeline.previous, current);
    assert_eq!(timeline.next, next_end);
    assert_eq!(timeline.plan, next_plan);
    assert_eq!(x(&returned, "a"), 100.0);
    assert!(!timeline.is_complete());
}

#[test]
fn continue_from_current_starts_at_the_visible_frame() {
    let (previous, target) = pair(0.0, 100.0);
    let plan = plan_transition(&previous, &target, &linear_intent(1.0));
    let mut timeline = Timeline::new(previous, plan, target);
    let current = timeline.advance(0.5);
    assert_eq!(x(&current, "a"), 50.0);

    let (next_start, next_end) = pair(100.0, 200.0);
    let mut next_plan = plan_transition(&next_start, &next_end, &linear_intent(1.0));
    next_plan.intent.interruption = InterruptionPolicy::ContinueFromCurrent;

    let returned = timeline.interrupt(next_plan, next_end.clone(), current.clone());
    assert_eq!(timeline.elapsed, 0.0);
    assert_eq!(timeline.previous, current);
    assert_eq!(timeline.scene_at(0.0), current);
    assert_eq!(x(&returned, "a"), 50.0);

    let half_way = timeline.advance(0.5);
    assert_eq!(x(&half_way, "a"), 125.0);
    assert_eq!(find(&half_way, "a").map(|node| node.transform.e()), Some(125.0));
}

#[test]
fn jump_to_end_returns_the_final_scene() {
    let (previous, target) = pair(0.0, 100.0);
    let mut plan = plan_transition(&previous, &target, &linear_intent(1.0));
    plan.intent.interruption = InterruptionPolicy::JumpToEnd;
    let mut timeline = Timeline::new(previous, plan, target);
    let current = timeline.advance(0.2);

    let (next_start, next_end) = pair(100.0, 200.0);
    let mut next_plan = plan_transition(&next_start, &next_end, &linear_intent(1.0));
    next_plan.intent.interruption = InterruptionPolicy::JumpToEnd;

    let returned = timeline.interrupt(next_plan.clone(), next_end.clone(), current);
    assert_eq!(timeline.elapsed, 1.0);
    assert_eq!(returned, next_end);
    assert!(timeline.is_complete());
    assert_eq!(timeline.scene_at(1.0), next_end);
    assert_eq!(timeline.plan, next_plan);
}
