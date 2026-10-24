//! Pure action reducer.

use std::collections::BTreeSet;

use davinci_types::Point;

use crate::{Action, InvalidationSet, MAX_SCALE, MIN_SCALE, RuntimeState, invalidate};

/// One reduced action: the next state plus the layers it dirties.
///
/// [`reduce`] stays the pure state transition. Hosts that drive incremental
/// recompilation use this richer result instead of recomputing the whole tree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeUpdate {
    /// State after the action.
    pub state: RuntimeState,
    /// Layers that must be recomputed, already closed under implication.
    pub invalidation: InvalidationSet,
    /// Semantic ids whose pointer or selection membership changed.
    ///
    /// The symmetric difference between input and output state is used instead
    /// of the action payload alone, so [`Action::Release`] still names the id
    /// that was pressed and [`Action::ClearSelection`] names every released
    /// selection member.
    pub targets: BTreeSet<String>,
}

/// Apply one action, returning the next state.
///
/// The input state is never mutated, so hosts can keep a state snapshot for
/// undo, replay, or diffing. Each arm documents how it moves state:
///
/// - `Select` without `additive` replaces the selection with one id. With
///   `additive` it toggles that id, which is the semantics behind a host
///   `Toggle` event.
/// - `Zoom` multiplies [`RuntimeState::viewport`] scale and clamps it to
///   `MIN_SCALE..=MAX_SCALE`. An anchor keeps the scene point under that
///   viewport pixel fixed by compensating `translate`; a non-finite or
///   non-positive factor is ignored.
/// - `Pan` only offsets `translate`. `Resize` only sets viewport extents,
///   clamping negative values to zero.
/// - `Seek` clamps time at zero. `Advance` moves time only while playing and
///   multiplies the delta by [`RuntimeState::playback`] rate.
/// - `Release` clears the pressed id and leaves the selection alone.
#[must_use]
pub fn reduce(state: &RuntimeState, action: &Action) -> RuntimeState {
    let mut next = state.clone();
    match action {
        Action::Hover(target) => next.hover = target.clone(),
        Action::Focus(target) => next.focus = target.clone(),
        Action::Press(target) => next.pressed = target.clone(),
        Action::Release => next.pressed = None,
        Action::Select { target, additive } => {
            if *additive {
                if !next.selection.remove(target) {
                    next.selection.insert(target.clone());
                }
            }
            else {
                next.selection.clear();
                next.selection.insert(target.clone());
            }
        }
        Action::ClearSelection => next.selection.clear(),
        Action::Zoom { factor, anchor } => {
            if factor.is_finite() && *factor > 0.0 {
                let scale = (next.viewport.scale * factor).clamp(MIN_SCALE, MAX_SCALE);
                if let Some(anchor) = anchor {
                    let world = next.viewport.screen_to_world(*anchor);
                    next.viewport.scale = scale;
                    next.viewport.translate = Point::new(anchor.x - world.x * scale, anchor.y - world.y * scale);
                }
                else {
                    next.viewport.scale = scale;
                }
            }
        }
        Action::Pan { dx, dy } => next.viewport.translate = next.viewport.translate.offset(*dx, *dy),
        Action::Resize { width, height } => {
            next.viewport.width = width.max(0.0);
            next.viewport.height = height.max(0.0);
        }
        Action::SetParameter { name, value } => {
            next.parameters.insert(name.clone(), *value);
        }
        Action::SetTheme(theme) => next.theme = theme.clone(),
        Action::Play => next.playback.playing = true,
        Action::Pause => next.playback.playing = false,
        Action::Seek(time) => next.playback.time = time.max(0.0),
        Action::Advance(delta) => {
            if next.playback.playing {
                next.playback.time = (next.playback.time + delta * next.playback.rate).max(0.0);
            }
        }
    }
    next
}

/// Apply one action and report the state plus the layers it dirties.
///
/// This wraps [`reduce`] with invalidation metadata, so an interaction yields
/// both the new [`RuntimeState`] and the incremental work it implies:
///
/// - A no-op action, meaning the state compares equal before and after, reports
///   an empty set. Hovering the already hovered id therefore costs nothing.
/// - `targets` is the symmetric difference of pointer ids and the selection
///   between input and output state, so it stays accurate for actions that
///   carry no id, such as `Release`.
/// - `invalidation` is the closed set documented on
///   [`invalidate`](crate::invalidate).
#[must_use]
pub fn reduce_update(state: &RuntimeState, action: &Action) -> RuntimeUpdate {
    let next = reduce(state, action);
    if next == *state {
        return RuntimeUpdate { state: next, invalidation: InvalidationSet::empty(), targets: BTreeSet::new() };
    }
    let invalidation = invalidate(action);
    let targets = changed_targets(state, &next);
    RuntimeUpdate { state: next, invalidation: invalidation.scopes, targets }
}

/// Ids whose pointer slot or selection membership differs between two states.
fn changed_targets(before: &RuntimeState, after: &RuntimeState) -> BTreeSet<String> {
    let mut targets = BTreeSet::new();
    extend_changed(&mut targets, before.hover.as_deref(), after.hover.as_deref());
    extend_changed(&mut targets, before.focus.as_deref(), after.focus.as_deref());
    extend_changed(&mut targets, before.pressed.as_deref(), after.pressed.as_deref());
    for id in before.selection.symmetric_difference(&after.selection) {
        targets.insert(id.clone());
    }
    targets
}

/// Record both ids when one pointer slot moved away from or onto an id.
fn extend_changed(targets: &mut BTreeSet<String>, before: Option<&str>, after: Option<&str>) {
    if before != after {
        targets.extend(before.into_iter().chain(after).map(str::to_owned));
    }
}
