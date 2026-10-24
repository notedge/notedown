//! Conversion from host input events to semantic actions.

use davinci_scene::{DisplayScene, Point};

use crate::{Action, NormalizedEvent, RuntimeState, hit_test};

/// Exponent per wheel unit. A negative delta, which is a wheel-up scroll,
/// produces a factor above one and therefore zooms in.
const WHEEL_ZOOM_RATE: f64 = 0.0015;

/// Turn one host event into the actions it implies.
///
/// This function is pure: it reads the current [`RuntimeState`] and scene but
/// never mutates either. Pointer events resolve their semantic target by hit
/// testing the scene, so actions always carry a semantic id rather than raw
/// coordinates.
///
/// Only edge transitions produce actions. A [`NormalizedEvent::PointerMove`]
/// whose resolved target equals [`RuntimeState::hover`] yields no action, while
/// a move that leaves every node yields `Hover(None)`. A `PointerDown` without
/// an explicit target tests the given position; when nothing is hit no action
/// is produced.
#[must_use]
pub fn normalize(event: &NormalizedEvent, state: &RuntimeState, scene: &DisplayScene) -> Vec<Action> {
    match event {
        NormalizedEvent::PointerMove { x, y } => {
            let target = hit_test(scene, Point::new(*x, *y)).map(|hit| hit.id);
            if target == state.hover { Vec::new() } else { vec![Action::Hover(target)] }
        }
        NormalizedEvent::PointerEnter { target } => vec![Action::Hover(Some(target.clone()))],
        NormalizedEvent::PointerLeave => vec![Action::Hover(None)],
        NormalizedEvent::PointerDown { target, x, y } => {
            let resolved = target.clone().or_else(|| hit_test(scene, Point::new(*x, *y)).map(|hit| hit.id));
            match resolved {
                Some(target) => vec![Action::Press(Some(target))],
                None => Vec::new(),
            }
        }
        NormalizedEvent::PointerUp => vec![Action::Release],
        NormalizedEvent::Focus { target } => vec![Action::Focus(Some(target.clone()))],
        NormalizedEvent::Blur => vec![Action::Focus(None)],
        NormalizedEvent::Toggle { target } => vec![Action::Select { target: target.clone(), additive: true }],
        NormalizedEvent::Wheel { delta, x, y } => {
            vec![Action::Zoom { factor: wheel_factor(*delta), anchor: Some(Point::new(*x, *y)) }]
        }
        NormalizedEvent::Resize { width, height } => vec![Action::Resize { width: *width, height: *height }],
        NormalizedEvent::Play => vec![Action::Play],
        NormalizedEvent::Pause => vec![Action::Pause],
        NormalizedEvent::Seek { time } => vec![Action::Seek(*time)],
        NormalizedEvent::Tick { delta } => vec![Action::Advance(*delta)],
    }
}

/// Convert a wheel delta into a multiplicative zoom factor.
///
/// The mapping is exponential, so repeated wheel steps compose linearly instead
/// of accelerating toward the ends of the scale range.
fn wheel_factor(delta: f64) -> f64 {
    (-delta * WHEEL_ZOOM_RATE).exp()
}
