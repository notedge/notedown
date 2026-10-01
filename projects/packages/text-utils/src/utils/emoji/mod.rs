pub use emojic::emojis::Emoji;

use emojic::parse_alias;

/// Parses the given Emoji name into a unicode Emoji.
pub fn parse_emoji<S: AsRef<str>>(emoji: S) -> Option<&'static Emoji> {
    parse_alias(emoji.as_ref())
}
