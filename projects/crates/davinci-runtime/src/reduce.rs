//! Pure action reducer.

use davinci_types::Point;

use crate::{Action, MAX_SCALE, MIN_SCALE, RuntimeState};

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
