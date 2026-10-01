use crate::{NoteError, NoteErrorKind};
use pex::StopBecause;
use std::ops::Range;

impl From<StopBecause> for NoteError {
    fn from(value: StopBecause) -> Self {
        let kind = NoteErrorKind::Syntax { message: value.to_string(), range: value.range(), file: None };
        Self { kind: Box::new(kind) }
    }
}
