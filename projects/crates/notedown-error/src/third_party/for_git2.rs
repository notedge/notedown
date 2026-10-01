use crate::NoteError;
use git2::Error;

impl From<Error> for NoteError {
    fn from(e: Error) -> Self {
        NoteError::runtime_error(e.to_string())
    }
}
