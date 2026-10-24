//! Stable chart diagnostics and fatal compilation failures.

use std::fmt::{Display, Formatter, Result as FmtResult};

use davinci_types::{Diagnostic, DiagnosticSeverity, diagnostic};

/// Stable dotted diagnostic codes emitted by the chart domain.
///
/// Codes use the `davinci.chart.<failure>` shape and must not be renamed after
/// a wire contract consumes them.
pub mod codes {
    /// A data snapshot has no columns or no rows.
    pub const EMPTY_DATA: &str = "davinci.chart.empty-data";
    /// Columns disagree on their row count.
    pub const RAGGED_COLUMN: &str = "davinci.chart.ragged-column";
    /// A value does not match its declared field kind.
    pub const TYPE_MISMATCH: &str = "davinci.chart.type-mismatch";
    /// A row carries a missing value.
    pub const MISSING_VALUE: &str = "davinci.chart.missing-value";
    /// A quantitative value is not finite.
    pub const NON_FINITE_VALUE: &str = "davinci.chart.non-finite-value";
    /// Two data columns or two declared fields share one name.
    pub const DUPLICATE_FIELD: &str = "davinci.chart.duplicate-field";
    /// A field reference does not exist.
    pub const UNKNOWN_FIELD: &str = "davinci.chart.unknown-field";
    /// The chart specification is internally inconsistent.
    pub const INVALID_SPEC: &str = "davinci.chart.invalid-spec";
    /// A scale declaration cannot be applied to its channel.
    pub const INVALID_SCALE: &str = "davinci.chart.invalid-scale";
    /// A channel has no values to train its scale.
    pub const EMPTY_DOMAIN: &str = "davinci.chart.empty-domain";
    /// A scale domain collapsed to a single value and was expanded.
    pub const DEGENERATE_DOMAIN: &str = "davinci.chart.degenerate-domain";
    /// A mark or guide is drawn approximately, or not at all.
    pub const APPROXIMATED: &str = "davinci.chart.approximated";
}

pub(crate) fn chart_error(code: &str, message: impl Into<String>) -> Diagnostic {
    diagnostic("chart", code, DiagnosticSeverity::Error, message)
}

pub(crate) fn chart_warning(code: &str, message: impl Into<String>) -> Diagnostic {
    diagnostic("chart", code, DiagnosticSeverity::Warning, message)
}

/// A fatal chart validation or compilation failure.
///
/// The small code-and-message surface mirrors [`davinci_scene::SceneError`] and
/// [`davinci_diagram::DiagramError`]. The structured record is available
/// through [`ChartError::diagnostic`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChartError {
    code: String,
    message: String,
    diagnostic: Box<Diagnostic>,
}

impl ChartError {
    /// Create an error from a stable diagnostic code and message.
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        let message = message.into();
        let diagnostic = chart_error(code, message.clone());
        Self { code: code.to_string(), message, diagnostic: Box::new(diagnostic) }
    }

    /// The stable `davinci.chart.*` diagnostic code.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// The human-readable failure description.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The structured diagnostic record for this failure.
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub(crate) fn from_diagnostic(record: &Diagnostic) -> Self {
        Self {
            code: record.code().as_str().to_string(),
            message: record.message().fallback().unwrap_or_default().to_string(),
            diagnostic: Box::new(record.clone()),
        }
    }
}

impl Display for ChartError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ChartError {}
