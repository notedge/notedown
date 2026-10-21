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
    /// Parser or adapter failed while reading a format.
    Parse {
        format: String,
        message: String,
    },
    /// Requested block or feature cannot be expressed in the target format.
    Unsupported {
        format: String,
        operation: String,
    },
}

impl FormatError {
    pub fn not_implemented(format: impl Into<String>, direction: impl Into<String>) -> Self {
        Self::NotImplemented {
            format: format.into(),
            direction: direction.into(),
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    pub fn parse(format: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Parse {
            format: format.into(),
            message: message.into(),
        }
    }

    pub fn unsupported(format: impl Into<String>, operation: impl Into<String>) -> Self {
        Self::Unsupported {
            format: format.into(),
            operation: operation.into(),
        }
    }
}

impl Display for FormatError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::NotImplemented { format, direction } => {
                write!(f, "{direction} for `{format}` is not implemented")
            }
            Self::InvalidInput { message } => write!(f, "invalid input: {message}"),
            Self::Parse { format, message } => write!(f, "{format} parse error: {message}"),
            Self::Unsupported { format, operation } => {
                write!(f, "{format} does not support {operation}")
            }
        }
    }
}

impl std::error::Error for FormatError {}
