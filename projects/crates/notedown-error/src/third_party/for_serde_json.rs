use crate::NoteError;
use serde_json::Error;

impl From<Error> for NoteError {
    fn from(e: Error) -> Self {
        NoteError::syntax_error(e.to_string())
    }
}
