//! Mutable interaction state kept outside the canonical graphic spec.

use std::collections::{BTreeMap, BTreeSet};

use davinci_types::Point;

/// Smallest viewport scale the reducer accepts.
pub const MIN_SCALE: f64 = 0.05;

/// Largest viewport scale the reducer accepts.
pub const MAX_SCALE: f64 = 64.0;

/// The visible window over scene coordinates.
///
/// Projection is `screen = scene * scale + translate`, so `translate` is
/// applied after scaling and is expressed in viewport pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Visible width in viewport pixels.
    pub width: f64,
    /// Visible height in viewport pixels.
    pub height: f64,
    /// Scene-to-pixel scale factor.
    pub scale: f64,
    /// Pixel offset applied after scaling.
    pub translate: Point,
}

impl Viewport {
    /// Project a scene point onto the viewport.
    #[must_use]
    pub fn world_to_screen(self, point: Point) -> Point {
        Point::new(point.x * self.scale + self.translate.x, point.y * self.scale + self.translate.y)
    }

    /// Map a viewport pixel back to scene coordinates.
    ///
    /// The result is only meaningful for a finite, non-zero `scale`, which
    /// [`reduce`](crate::reduce) always leaves inside `MIN_SCALE..=MAX_SCALE`.
    /// A degenerate scale returns the input point unchanged instead of `NaN`.
    #[must_use]
    pub fn screen_to_world(self, point: Point) -> Point {
        if !self.scale.is_finite() || self.scale == 0.0 {
            return point;
        }
        Point::new((point.x - self.translate.x) / self.scale, (point.y - self.translate.y) / self.scale)
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self { width: 0.0, height: 0.0, scale: 1.0, translate: Point::ZERO }
    }
}

/// Time cursor for animation and parameter playback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playback {
    /// Whether time may advance.
    pub playing: bool,
    /// Current time in seconds. Never negative.
    pub time: f64,
    /// Time multiplier applied to each advance.
    pub rate: f64,
}

impl Default for Playback {
    fn default() -> Self {
        Self { playing: false, time: 0.0, rate: 1.0 }
    }
}

/// All mutable interaction state for one mounted scene.
///
/// Every field is outside the canonical spec. Deriving a new graphic from this
/// state is a host concern; the scene itself is never rewritten.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeState {
    /// Semantic id currently under the pointer.
    pub hover: Option<String>,
    /// Semantic id holding keyboard focus.
    pub focus: Option<String>,
    /// Semantic id held down by a pointer press.
    pub pressed: Option<String>,
    /// Selected semantic ids in sorted order, so iteration is host-stable.
    pub selection: BTreeSet<String>,
    /// Current viewport projection.
    pub viewport: Viewport,
    /// Named numeric parameters read by layout and motion.
    pub parameters: BTreeMap<String, f64>,
    /// Selected host theme or style preset.
    pub theme: Option<String>,
    /// Playback cursor.
    pub playback: Playback,
}
