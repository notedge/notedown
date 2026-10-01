use crate::{NoteError, NoteErrorKind};
use url::ParseError;

impl From<ParseError> for NoteError {
    fn from(value: ParseError) -> Self {
        Self { kind: Box::new(NoteErrorKind::Syntax { message: value.to_string(), range: Default::default(), file: None }) }
    }
}
