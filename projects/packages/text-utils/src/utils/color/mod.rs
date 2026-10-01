pub use crate::Result;
use std::str::FromStr;

pub use css_color_parser::{Color, NAMED_COLORS};

/// - Named: eg. `slateblue`
/// - `#fff`
/// - `#ff0011`
/// - `rgba(255, 128, 12, 0.5)`
pub fn parse_color<S: AsRef<str>>(color: S) -> Option<Color> {
    Color::from_str(color.as_ref()).ok()
}
