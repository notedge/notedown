use std::fmt::{Display, Formatter, Result as FmtResult};

/// Format import or export failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// Format module is not wired yet.
    NotImplemented {
        format: String,
        direction: String,
    },
    /// Caller supplied invalid bytes or parameters.
    InvalidInput {
        message: String,
    },
}

impl Display for FormatError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::NotImplemented { format, direction } => {
                write!(f, "{direction} for `{format}` is not implemented")
            }
            Self::InvalidInput { message } => write!(f, "invalid input: {message}"),
        }
    }
}

impl std::error::Error for FormatError {}
