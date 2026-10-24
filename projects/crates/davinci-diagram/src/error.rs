//! Structured diagram lowering failures.

use std::fmt::{Display, Formatter, Result as FmtResult};

use davinci_scene::SceneError;
use davinci_types::{Diagnostic, DiagnosticSeverity, codes, diagnostic};

/// A fatal diagram parsing or lowering failure.
///
/// The small code-and-message surface mirrors [`SceneError`]. The full
/// structured record is available through [`DiagramError::diagnostic`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagramError {
    code: &'static str,
    message: String,
    diagnostic: Diagnostic,
}

impl DiagramError {
    /// Create an error from a stable diagnostic code and message.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        let message = message.into();
        let diagnostic = diagnostic("diagram", code, DiagnosticSeverity::Error, message.clone());
        Self { code, message, diagnostic }
    }

    /// The stable `davinci.*` diagnostic code.
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// The human-readable failure description.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The structured diagnostic record for this failure.
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub(crate) fn parse_failed(message: impl Into<String>) -> Self {
        Self::new(codes::PARSE_FAILED, message)
    }

    pub(crate) fn ast_unavailable(message: impl Into<String>) -> Self {
        Self::new(codes::AST_UNAVAILABLE, message)
    }

    pub(crate) fn unsupported(message: impl Into<String>) -> Self {
        Self::new(codes::UNSUPPORTED, message)
    }

    pub(crate) fn empty_diagram(message: impl Into<String>) -> Self {
        Self::new(codes::EMPTY_DIAGRAM, message)
    }
}

impl Display for DiagramError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for DiagramError {}

impl From<SceneError> for DiagramError {
    fn from(error: SceneError) -> Self {
        Self::new(codes::SERIALIZATION_FAILED, format!("{}: {}", error.code(), error.message()))
    }
}
