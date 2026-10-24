//! Host input shapes accepted by [`normalize`](crate::normalize).

/// A host input event already converted into scene coordinates.
///
/// Normalization keeps this enum free of DOM, framework, or device details: a
/// host maps its own events onto these variants before the runtime sees them.
#[derive(Clone, Debug, PartialEq)]
pub enum NormalizedEvent {
    /// The pointer moved to a scene position.
    PointerMove {
        /// Horizontal scene coordinate.
        x: f64,
        /// Vertical scene coordinate.
        y: f64,
    },
    /// The pointer entered a semantic target resolved by the host.
    PointerEnter {
        /// Semantic id now under the pointer.
        target: String,
    },
    /// The pointer left the scene.
    PointerLeave,
    /// The pointer was pressed at a scene position.
    ///
    /// When `target` is absent the position is resolved by hit testing.
    PointerDown {
        /// Semantic id already resolved by the host, when available.
        target: Option<String>,
        /// Horizontal scene coordinate.
        x: f64,
        /// Vertical scene coordinate.
        y: f64,
    },
    /// The pointer was released.
    PointerUp,
    /// A semantic target received focus.
    Focus {
        /// Semantic id that gained focus.
        target: String,
    },
    /// Focus left the scene.
    Blur,
    /// A semantic target was toggled by a host activation, such as keyboard
    /// `Enter` or `Space` on a focused element.
    Toggle {
        /// Semantic id to toggle in the selection.
        target: String,
    },
    /// The wheel travelled by `delta` at a scene position.
    Wheel {
        /// Raw wheel delta. Positive values scroll down and zoom out.
        delta: f64,
        /// Horizontal scene coordinate of the pointer.
        x: f64,
        /// Vertical scene coordinate of the pointer.
        y: f64,
    },
    /// The host viewport changed size.
    Resize {
        /// New width in viewport pixels.
        width: f64,
        /// New height in viewport pixels.
        height: f64,
    },
    /// Playback should start or resume.
    Play,
    /// Playback should pause.
    Pause,
    /// Playback should jump to an absolute time in seconds.
    Seek {
        /// Requested time in seconds.
        time: f64,
    },
    /// The host clock advanced by `delta` seconds.
    Tick {
        /// Elapsed wall-clock seconds since the previous tick.
        delta: f64,
    },
}
