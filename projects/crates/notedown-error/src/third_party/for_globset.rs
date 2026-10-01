use crate::NoteError;
use globset::Error;

impl From<Error> for NoteError {
    fn from(e: Error) -> Self {
        NoteError::runtime_error(e.to_string())
    }
}
