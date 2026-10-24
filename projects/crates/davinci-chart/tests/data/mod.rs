use davinci_chart::{DataSnapshot, FieldKind, Value, codes};
use davinci_types::DiagnosticSeverity;

use crate::support::{column, has_code, severity_of, snapshot, texts};

#[test]
fn ragged_columns_report_their_row_count() {
    let data = snapshot(vec![
        column("category", FieldKind::Nominal, texts(&["A"])),
        column("value", FieldKind::Quantitative, vec![Value::number(1.0), Value::number(2.0)]),
    ]);

    let diagnostics = data.validate();
    assert!(has_code(&diagnostics, codes::RAGGED_COLUMN), "columns with different lengths must be rejected");
    assert!(!data.is_valid());
}

#[test]
fn values_must_match_their_declared_field_kind() {
    let data = snapshot(vec![column("value", FieldKind::Quantitative, texts(&["ten"]))]);

    let diagnostics = data.validate();
    assert!(has_code(&diagnostics, codes::TYPE_MISMATCH));
    assert_eq!(severity_of(&diagnostics, codes::TYPE_MISMATCH), Some(DiagnosticSeverity::Error));
}

#[test]
fn an_empty_snapshot_reports_missing_columns_and_rows() {
    let no_columns = DataSnapshot::new(Vec::new());
    assert!(has_code(&no_columns.validate(), codes::EMPTY_DATA));
    assert!(no_columns.is_empty());

    let no_rows = snapshot(vec![column("value", FieldKind::Quantitative, Vec::new())]);
    assert!(has_code(&no_rows.validate(), codes::EMPTY_DATA));
    assert!(no_rows.is_empty());
}

#[test]
fn missing_values_warn_without_producing_nan() {
    let data = snapshot(vec![column("value", FieldKind::Quantitative, vec![Value::Null, Value::number(3.0)])]);

    let diagnostics = data.validate();
    assert!(has_code(&diagnostics, codes::MISSING_VALUE));
    assert_eq!(severity_of(&diagnostics, codes::MISSING_VALUE), Some(DiagnosticSeverity::Warning));
    assert!(!has_code(&diagnostics, codes::NON_FINITE_VALUE));
    assert!(data.is_valid(), "a missing value is reported, not silently accepted as `NaN`");
}

#[test]
fn non_finite_numbers_are_rejected() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let data = snapshot(vec![column("value", FieldKind::Quantitative, vec![Value::number(value)])]);
        let diagnostics = data.validate();
        assert!(has_code(&diagnostics, codes::NON_FINITE_VALUE), "{value} must be rejected");
        assert!(!data.is_valid());
    }
}

#[test]
fn duplicate_columns_are_reported() {
    let data = snapshot(vec![
        column("value", FieldKind::Quantitative, vec![Value::number(1.0)]),
        column("value", FieldKind::Quantitative, vec![Value::number(2.0)]),
    ]);

    assert!(has_code(&data.validate(), codes::DUPLICATE_FIELD));
}

#[test]
fn temporal_columns_require_timestamps() {
    let data = snapshot(vec![column("seen", FieldKind::Temporal, vec![Value::number(1.0)])]);
    assert!(has_code(&data.validate(), codes::TYPE_MISMATCH));

    let timestamps = snapshot(vec![column("seen", FieldKind::Temporal, vec![Value::timestamp(1_700_000_000_000)])]);
    assert!(timestamps.validate().is_empty());
}

#[test]
fn ordinal_columns_accept_text() {
    let data = snapshot(vec![column("size", FieldKind::Ordinal, texts(&["s", "m", "l"]))]);
    assert!(data.validate().is_empty());
}
