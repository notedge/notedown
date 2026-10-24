//! Semantic actions produced by [`normalize`](crate::normalize) and consumed by
//! [`reduce`](crate::reduce).

use davinci_types::Point;

/// One semantic change requested by input.
///
/// Actions never carry document edits. They only move runtime state, which is
/// why an interaction can never rewrite the canonical spec.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Set or clear the hovered semantic id.
    Hover(Option<String>),
    /// Set or clear the focused semantic id.
    Focus(Option<String>),
    /// Set or clear the pressed semantic id.
    Press(Option<String>),
    /// Clear the pressed semantic id, leaving the selection untouched.
    Release,
    /// Select a semantic id.
    Select {
        /// Semantic id to select.
        target: String,
        /// When `true`, toggle membership instead of replacing the selection.
        additive: bool,
    },
    /// Remove every selected id.
    ClearSelection,
    /// Scale the viewport.
    Zoom {
        /// Multiplicative scale change. Must be finite and positive.
        factor: f64,
        /// Viewport pixel that keeps its scene position, usually the pointer.
        anchor: Option<Point>,
    },
    /// Offset the viewport by a pixel delta.
    Pan {
        /// Horizontal pixel delta.
        dx: f64,
        /// Vertical pixel delta.
        dy: f64,
    },
    /// Resize the viewport in pixels.
    Resize {
        /// New width.
        width: f64,
        /// New height.
        height: f64,
    },
    /// Set a named numeric parameter.
    SetParameter {
        /// Parameter name read by layout or motion.
        name: String,
        /// New parameter value.
        value: f64,
    },
    /// Start or resume playback.
    Play,
    /// Pause playback.
    Pause,
    /// Seek to an absolute time in seconds.
    Seek(f64),
    /// Advance playback by a wall-clock delta in seconds.
    Advance(f64),
}
