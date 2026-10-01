use crate::{DiagnosticLevel, NoteError, NoteErrorKind};
use html_parser::Error;

impl From<Error> for NoteError {
    fn from(e: Error) -> Self {
        let kind = match e {
            Error::Parsing(e) => NoteErrorKind::SyntaxError(e),
            Error::IO(e) => NoteErrorKind::IOError(e),
            Error::Cli(_) => {
                unimplemented!()
            }
            Error::Serde(_) => {
                unimplemented!()
            }
        };
        Self { kind: Box::new(kind), level: DiagnosticLevel::None, file: None, range: None }
    }
}
