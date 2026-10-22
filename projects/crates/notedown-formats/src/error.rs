use std::fmt::{Display, Formatter, Result as FmtResult};

use boost_diagnostic::{Diagnostic, DiagnosticOrigin, DiagnosticSeverity, Message, MessageArg};

/// Notedown product namespace for all format diagnostics.
pub const NAMESPACE: &str = "notedown";

/// Stable dotted diagnostic codes emitted by the formats layer.
/// Format: `notedown.<component>.<failure>` — never rename after wire contract release.
pub mod codes {
    /// Generic format adapter reports a failure.
    pub const ADAPTER_FAILURE: &str = "notedown.format.adapter-failure";
    /// Format module is not wired yet.
    pub const NOT_IMPLEMENTED: &str = "notedown.format.not-implemented";
    /// Caller supplied invalid bytes or parameters.
    pub const INVALID_INPUT: &str = "notedown.format.invalid-input";
    /// Parser failed while reading a format.
    pub const PARSE_FAILED: &str = "notedown.format.parse-failed";
    /// Requested block or feature cannot be expressed in the target format.
    pub const UNSUPPORTED: &str = "notedown.format.unsupported";
    /// Format work exceeded the caller's budget.
    pub const BUDGET_EXHAUSTED: &str = "notedown.format.budget-exhausted";
}

/// Build a [`DiagnosticOrigin`] for Notedown formats component.
pub fn origin_formats() -> DiagnosticOrigin {
    DiagnosticOrigin::new(NAMESPACE, "formats")
}

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

/// Convert a [`FormatError`] into a structured [`Diagnostic`] on the shared
/// boost-diagnostic wire. This is the authoritative bridge between the
/// formats-layer typed error enum and the neutral diagnostic envelope.
pub fn from_format_error(error: &FormatError) -> Diagnostic {
    use boost_diagnostic::DiagnosticCode;

    let origin = origin_formats();
    let fallback = error.to_string();
    match error {
        FormatError::NotImplemented { format, direction } => Diagnostic::new(
            DiagnosticCode::new(codes::NOT_IMPLEMENTED),
            DiagnosticSeverity::Error,
            origin,
            Message::new(codes::NOT_IMPLEMENTED)
                .with_arg("format", MessageArg::Text(format.clone()))
                .with_arg("direction", MessageArg::Text(direction.clone()))
                .with_fallback(fallback),
        ),
        FormatError::InvalidInput { message } => Diagnostic::new(
            DiagnosticCode::new(codes::INVALID_INPUT),
            DiagnosticSeverity::Error,
            origin,
            Message::new(codes::INVALID_INPUT)
                .with_arg("message", MessageArg::Text(message.clone()))
                .with_fallback(fallback),
        ),
        FormatError::Parse { format, message } => Diagnostic::new(
            DiagnosticCode::new(codes::PARSE_FAILED),
            DiagnosticSeverity::Error,
            origin,
            Message::new(codes::PARSE_FAILED)
                .with_arg("format", MessageArg::Text(format.clone()))
                .with_arg("message", MessageArg::Text(message.clone()))
                .with_fallback(fallback),
        ),
        FormatError::Unsupported { format, operation } => Diagnostic::new(
            DiagnosticCode::new(codes::UNSUPPORTED),
            DiagnosticSeverity::Error,
            origin,
            Message::new(codes::UNSUPPORTED)
                .with_arg("format", MessageArg::Text(format.clone()))
                .with_arg("operation", MessageArg::Text(operation.clone()))
                .with_fallback(fallback),
        ),
        FormatError::Budget { format, message } => Diagnostic::new(
            DiagnosticCode::new(codes::BUDGET_EXHAUSTED),
            DiagnosticSeverity::Error,
            origin,
            Message::new(codes::BUDGET_EXHAUSTED)
                .with_arg("format", MessageArg::Text(format.clone()))
                .with_arg("message", MessageArg::Text(message.clone()))
                .with_fallback(fallback),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boost_diagnostic::DiagnosticCode;

    #[test]
    fn codes_are_notedown_prefixed() {
        assert!(codes::NOT_IMPLEMENTED.starts_with("notedown.format."));
        assert!(codes::INVALID_INPUT.starts_with("notedown.format."));
        assert!(codes::PARSE_FAILED.starts_with("notedown.format."));
        assert!(codes::UNSUPPORTED.starts_with("notedown.format."));
        assert!(codes::BUDGET_EXHAUSTED.starts_with("notedown.format."));
    }

    #[test]
    fn from_format_error_preserves_code_and_origin() {
        let err = FormatError::parse("docx", "table border overflow");
        let diag = from_format_error(&err);
        assert_eq!(diag.code(), &DiagnosticCode::new(codes::PARSE_FAILED));
        assert_eq!(diag.origin().namespace(), "notedown");
        assert_eq!(diag.origin().component(), "formats");
        assert_eq!(diag.message().key(), codes::PARSE_FAILED);
        assert!(diag.message().fallback().is_some());
    }

    #[test]
    fn from_format_error_includes_typed_args() {
        let err = FormatError::unsupported("pdf", "hyperlinks");
        let diag = from_format_error(&err);
        let args: Vec<_> = diag.message().args().iter().map(|(name, _)| name.clone()).collect();
        assert!(args.contains(&"format".to_string()));
        assert!(args.contains(&"operation".to_string()));
    }
}
