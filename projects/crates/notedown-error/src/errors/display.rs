use super::*;

impl Debug for NoteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.kind, f)
    }
}

impl Display for NoteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.kind, f)
    }
}

impl Error for NoteError {}

impl Display for NoteErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NoteErrorKind::IOError { message, source } => {
                write!(f, "IOError: {} ({})", message, source)
            }
            NoteErrorKind::Syntax { message, range, file } => {
                write!(f, "SyntaxError: {} (range: {:?}, file: {:?})", message, range, file)
            }
            NoteErrorKind::Custom { message } => f.write_str(message),
            NoteErrorKind::Unknown { message } => {
                write!(f, "UnknownError: {}", message)
            }
        }
    }
}

impl Error for NoteErrorKind {}
