//! Scene validation and serialization errors.

use std::fmt::{Display, Formatter, Result as FmtResult};

use davinci_types::{Diagnostic, DiagnosticSeverity, codes, diagnostic};

/// A stable scene failure.
///
/// The error is intentionally a small code plus message pair. The full
/// structured record is available through [`SceneError::diagnostic`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneError {
    code: &'static str,
    message: String,
}

impl SceneError {
    /// Create an error from a stable diagnostic code and a message.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    /// The stable `davinci.*` diagnostic code.
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// The human-readable failure description.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Build a structured diagnostic record for this failure.
    pub fn diagnostic(&self) -> Diagnostic {
        diagnostic("scene", self.code, DiagnosticSeverity::Error, self.message.clone())
    }

    pub(crate) fn invalid_geometry(message: impl Into<String>) -> Self {
        Self::new(codes::INVALID_GEOMETRY, message)
    }

    pub(crate) fn invalid_style(message: impl Into<String>) -> Self {
        Self::new(codes::INVALID_STYLE, message)
    }

    pub(crate) fn invalid_transform(message: impl Into<String>) -> Self {
        Self::new(codes::INVALID_TRANSFORM, message)
    }
}

impl Display for SceneError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for SceneError {}
