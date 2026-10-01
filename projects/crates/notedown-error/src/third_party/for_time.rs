use crate::NoteError;
use chrono::ParseError;

impl From<ParseError> for NoteError {
    fn from(e: ParseError) -> Self {
        NoteError::syntax_error(e.to_string())
    }
}
