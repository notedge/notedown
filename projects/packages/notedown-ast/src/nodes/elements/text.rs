use diagnostic_quick::{QError, QResult};

use super::*;

///
#[repr(u8)]
#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum TextSpan {
    ///
    Empty = 0,
    ///
    Normal(String),
    ///
    Raw(String),
    ///
    HTMLRawInline(String),
    /// The Unicode codepoint sequence of this emoji.
    /// The actual/rendered emoji.
    Emoji(String),
    ///
    Escaped(char),
    ///
    SoftNewline,
    ///
    HardNewline,
    ///
    CheckBox(bool),
}

impl Default for TextSpan {
    fn default() -> Self {
        Self::Empty
    }
}

impl IntoNotedown for TextSpan {
    fn into_node(self, span: &Span, file: &FileID) -> NotedownNode {
        NotedownKind::TextSpan(Box::new(self)).into_node(span, file)
    }
}

impl NotedownKind {
    /// Aka. `<br>`
    #[inline]
    pub fn hard_break(span: &Span, file: &FileID) -> NotedownNode {
        TextSpan::HardNewline.into_node(span, file)
    }
    /// Aka. `\n`
    #[inline]
    pub fn soft_break(span: &Span, file: &FileID) -> NotedownNode {
        TextSpan::SoftNewline.into_node(span, file)
    }
    ///
    #[inline]
    pub fn text(s: impl Into<String>, span: &Span, file: &FileID) -> NotedownNode {
        TextSpan::Normal(s.into()).into_node(span, file)
    }
    ///
    #[inline]
    pub fn text_raw(s: impl Into<String>, span: &Span, file: &FileID) -> NotedownNode {
        TextSpan::Raw(s.into()).into_node(span, file)
    }
    /// Constructor of [`TextSpan::Escaped`]
    #[inline]
    pub fn escaped(text: &str, span: &Span, file: &FileID) -> QResult<NotedownNode> {
        let c = match text.chars().next() {
            None => '\\',
            Some(s) => s,
        };
        Ok(TextSpan::Escaped(c).into_node(span, file))
    }
    /// Constructor of [`TextSpan::Escaped`]
    #[inline]
    pub fn escaped_char(c: char, span: &Span, file: &FileID) -> NotedownNode {
        TextSpan::Escaped(c).into_node(span, file)
    }
    /// Constructor of [`TextSpan::Normal`]
    #[inline]
    pub fn escaped_html(char: char, span: &Span, file: &FileID) -> QResult<NotedownNode> {
        todo!()
    }
    /// Constructor of [`TextSpan::Emoji`]
    #[inline]
    pub fn emoji(text: &str, span: &Span, file: &FileID) -> QResult<NotedownNode> {
        match text_utils::parse_emoji(text) {
            None => {
                let msg = format!("{} can not be parsed as emoji", text);
                Err(QError::syntax_error(msg).with_range(span))?
            }
            Some(s) => Ok(TextSpan::Emoji(s.grapheme.to_string()).into_node(span, file)),
        }
    }
}
