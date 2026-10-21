use std::fmt::{Display, Formatter, Result as FmtResult};

/// Format import or export failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// Format module is not wired yet.
    NotImplemented {
        /// Format identifier.
        format: String,
        /// `import` or `export`.
        direction: String,
    },
    /// Caller supplied invalid bytes or parameters.
    InvalidInput {
        /// Human-readable validation message.
        message: String,
    },
    /// Parser or adapter failed while reading a format.
    Parse {
        /// Format identifier.
        format: String,
        /// Parser or adapter message.
        message: String,
    },
    /// Requested block or feature cannot be expressed in the target format.
    Unsupported {
        /// Target format identifier.
        format: String,
        /// Operation that cannot be expressed.
        operation: String,
    },
    /// Format work exceeded the caller's budget.
    Budget {
        /// Format identifier.
        format: String,
        /// Budget limit that was exhausted.
        message: String,
    },
}

impl FormatError {
    /// Build a not-implemented error for a format direction.
    pub fn not_implemented(format: impl Into<String>, direction: impl Into<String>) -> Self {
        Self::NotImplemented { format: format.into(), direction: direction.into() }
    }

    /// Build an invalid-input error.
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput { message: message.into() }
    }

    /// Build a parse error for a format adapter.
    pub fn parse(format: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Parse { format: format.into(), message: message.into() }
    }

    /// Build an unsupported-operation error for a format exporter.
    pub fn unsupported(format: impl Into<String>, operation: impl Into<String>) -> Self {
        Self::Unsupported { format: format.into(), operation: operation.into() }
    }

    /// Build a budget-exhaustion error for a format request.
    pub fn budget(format: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Budget { format: format.into(), message: message.into() }
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
            Self::Budget { format, message } => write!(f, "{format} budget exhausted: {message}"),
        }
    }
}

impl std::error::Error for FormatError {}
