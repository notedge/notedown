//! Columnar data snapshots and their field semantics.
//!
//! A snapshot is a set of named columns. Every column declares one
//! [`FieldKind`] and carries one [`Value`] per row, so validation can reject
//! ragged columns, kind mismatches, missing values, and non-finite numbers
//! before any scale sees the data.

use davinci_types::{Diagnostic, DiagnosticSeverity};

use crate::error::{chart_error, chart_warning, codes};

/// The semantic kind of one data field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// A continuous numeric measure.
    Quantitative,
    /// An instant expressed as epoch milliseconds.
    Temporal,
    /// An unordered category.
    Nominal,
    /// An ordered category.
    Ordinal,
}

impl FieldKind {
    /// A stable lowercase keyword for diagnostics and source references.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quantitative => "quantitative",
            Self::Temporal => "temporal",
            Self::Nominal => "nominal",
            Self::Ordinal => "ordinal",
        }
    }

    /// Returns `true` for kinds that a linear scale can map.
    pub fn is_quantitative(self) -> bool {
        matches!(self, Self::Quantitative | Self::Temporal)
    }

    /// Returns `true` for kinds that a band or point scale can map.
    pub fn is_categorical(self) -> bool {
        !self.is_quantitative()
    }
}

/// One cell in a data snapshot.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// An absent value. Nulls are reported and never mapped to `NaN`.
    Null,
    /// A finite quantitative value.
    Number(f64),
    /// A categorical value.
    Text(String),
    /// An instant in epoch milliseconds.
    Timestamp(i64),
}

impl Value {
    /// Create a quantitative value.
    pub fn number(value: f64) -> Self {
        Self::Number(value)
    }

    /// Create a categorical value.
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    /// Create a temporal value.
    pub fn timestamp(millis: i64) -> Self {
        Self::Timestamp(millis)
    }

    /// Returns `true` for [`Value::Null`].
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Returns `true` when this value can belong to a field of `kind`.
    pub fn matches_kind(&self, kind: FieldKind) -> bool {
        match self {
            Self::Null => true,
            Self::Number(_) => kind == FieldKind::Quantitative,
            Self::Timestamp(_) => kind == FieldKind::Temporal,
            Self::Text(_) => matches!(kind, FieldKind::Nominal | FieldKind::Ordinal),
        }
    }

    /// The numeric projection used by scales and statistics.
    ///
    /// Temporal values project to epoch milliseconds.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Timestamp(millis) => Some(*millis as f64),
            _ => None,
        }
    }

    /// The categorical projection used by nominal, band, and point scales.
    pub fn label(&self) -> String {
        match self {
            Self::Null => "null".to_string(),
            Self::Number(value) => format_number(*value),
            Self::Text(value) => value.clone(),
            Self::Timestamp(millis) => millis.to_string(),
        }
    }
}

/// Format a finite number without trailing zeroes.
///
/// Integral values print as integers so category labels and axis ticks stay
/// stable and byte-identical, and floating point noise is trimmed.
pub fn format_number(value: f64) -> String {
    let rounded = value.round();
    if value != 0.0 && (value - rounded).abs() < 1e-9 && rounded.abs() < 1e15 {
        format!("{rounded}")
    }
    else if value == 0.0 {
        "0".to_string()
    }
    else {
        format!("{value}")
    }
}

/// One named data column with its declared field kind.
#[derive(Clone, Debug, PartialEq)]
pub struct DataColumn {
    /// Field name used by every encoding reference.
    pub name: String,
    /// Declared semantic kind of every value in this column.
    pub kind: FieldKind,
    /// Cell values in row order.
    pub values: Vec<Value>,
}

impl DataColumn {
    /// Create a column from its name, kind, and values.
    pub fn new(name: impl Into<String>, kind: FieldKind, values: Vec<Value>) -> Self {
        Self { name: name.into(), kind, values }
    }
}

/// A columnar data snapshot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DataSnapshot {
    /// Columns in declaration order.
    pub columns: Vec<DataColumn>,
}

impl DataSnapshot {
    /// Create a snapshot from its columns.
    pub fn new(columns: Vec<DataColumn>) -> Self {
        Self { columns }
    }

    /// Find a column by field name.
    pub fn field(&self, name: &str) -> Option<&DataColumn> {
        self.columns.iter().find(|column| column.name == name)
    }

    /// The row count, taken from the first column.
    pub fn row_count(&self) -> usize {
        self.columns.first().map(|column| column.values.len()).unwrap_or(0)
    }

    /// Returns `true` when the snapshot has no columns or no rows.
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty() || self.row_count() == 0
    }

    /// Validate every column and cell.
    ///
    /// Structural problems such as empty data, ragged columns, duplicate or
    /// blank names, kind mismatches, and non-finite numbers are reported as
    /// errors. Missing values are reported as warnings so compilation can skip
    /// them explicitly instead of letting `NaN` through.
    pub fn validate(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        if self.columns.is_empty() {
            diagnostics.push(chart_error(codes::EMPTY_DATA, "data snapshot has no columns"));
            return diagnostics;
        }

        let expected = self.row_count();
        let mut names: Vec<&str> = Vec::new();
        for column in &self.columns {
            if column.name.is_empty() {
                diagnostics.push(chart_error(codes::INVALID_SPEC, "data column has an empty name"));
            }
            if names.contains(&column.name.as_str()) {
                diagnostics.push(chart_error(codes::DUPLICATE_FIELD, format!("duplicate data column `{}`", column.name)));
            }
            else {
                names.push(column.name.as_str());
            }
            if column.values.is_empty() {
                diagnostics.push(chart_error(codes::EMPTY_DATA, format!("data column `{}` has no rows", column.name)));
            }
            else if column.values.len() != expected {
                diagnostics.push(chart_error(
                    codes::RAGGED_COLUMN,
                    format!("data column `{}` has {} rows but the snapshot has {expected}", column.name, column.values.len()),
                ));
            }

            for (index, value) in column.values.iter().enumerate() {
                if value.is_null() {
                    diagnostics.push(chart_warning(codes::MISSING_VALUE, format!("field `{}` row {index} is missing", column.name)));
                    continue;
                }
                if let Value::Number(number) = value {
                    if !number.is_finite() {
                        diagnostics.push(chart_error(
                            codes::NON_FINITE_VALUE,
                            format!("field `{}` row {index} is not finite, found {number}", column.name),
                        ));
                        continue;
                    }
                }
                if !value.matches_kind(column.kind) {
                    diagnostics.push(chart_error(
                        codes::TYPE_MISMATCH,
                        format!("field `{}` row {index} must be {} but found {:?}", column.name, column.kind.as_str(), value),
                    ));
                }
            }
        }
        diagnostics
    }

    /// Returns `true` when [`DataSnapshot::validate`] reports no errors.
    pub fn is_valid(&self) -> bool {
        !self.validate().iter().any(|record| record.severity() == DiagnosticSeverity::Error)
    }
}
