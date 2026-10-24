//! Deterministic easing curves.

/// A deterministic easing curve applied to normalized progress.
///
/// All curves are pure functions of the clamped input, so sampling the same
/// plan at the same time always produces the same value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    /// Linear progress: `f(t) = t`.
    Linear,
    /// Quadratic acceleration: `f(t) = t * t`.
    EaseIn,
    /// Quadratic deceleration: `f(t) = 1 - (1 - t) * (1 - t)`.
    EaseOut,
    /// Piecewise quadratic curve: `2 * t * t` below the midpoint and
    /// `1 - 2 * (1 - t) * (1 - t)` above it.
    EaseInOut,
}

impl Easing {
    /// Evaluate the curve at normalized progress `t`.
    ///
    /// `t` is clamped to `0.0..=1.0` before the formula is applied, so values
    /// outside the range return the endpoint values. A `NaN` input is returned
    /// unchanged because it has no deterministic position on the curve.
    pub fn apply(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => {
                let inverse = 1.0 - t;
                1.0 - inverse * inverse
            }
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                }
                else {
                    let inverse = 1.0 - t;
                    1.0 - 2.0 * inverse * inverse
                }
            }
        }
    }
}
