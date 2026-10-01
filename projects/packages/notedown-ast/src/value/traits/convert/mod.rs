use super::*;

impl From<bool> for NotedownValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}
