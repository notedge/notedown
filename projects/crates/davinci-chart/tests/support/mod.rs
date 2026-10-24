#![allow(dead_code)]

use davinci_chart::{Channel, ChannelEncoding, ChartSpec, DataColumn, DataSnapshot, FieldKind, FieldRef, Mark, Value};
use davinci_types::{Diagnostic, DiagnosticSeverity};

pub fn numbers(values: &[f64]) -> Vec<Value> {
    values.iter().copied().map(Value::number).collect()
}

pub fn texts(values: &[&str]) -> Vec<Value> {
    values.iter().map(|value| Value::text(*value)).collect()
}

pub fn column(name: &str, kind: FieldKind, values: Vec<Value>) -> DataColumn {
    DataColumn::new(name, kind, values)
}

pub fn snapshot(columns: Vec<DataColumn>) -> DataSnapshot {
    DataSnapshot::new(columns)
}

pub fn approx(left: f64, right: f64) -> bool {
    (left - right).abs() < 1e-9
}

pub fn has_code(diagnostics: &[Diagnostic], code: &str) -> bool {
    diagnostics.iter().any(|record| record.code().as_str() == code)
}

pub fn severity_of(diagnostics: &[Diagnostic], code: &str) -> Option<DiagnosticSeverity> {
    diagnostics.iter().find(|record| record.code().as_str() == code).map(Diagnostic::severity)
}

pub fn bar_spec() -> ChartSpec {
    ChartSpec::new(Mark::Bar)
        .with_field(FieldRef::new("category", FieldKind::Nominal))
        .with_field(FieldRef::new("value", FieldKind::Quantitative))
        .with_encoding(Channel::X, ChannelEncoding::field("category", FieldKind::Nominal))
        .with_encoding(Channel::Y, ChannelEncoding::field("value", FieldKind::Quantitative))
}

pub fn bar_data() -> DataSnapshot {
    snapshot(vec![
        column("category", FieldKind::Nominal, texts(&["A", "B", "C"])),
        column("value", FieldKind::Quantitative, numbers(&[10.0, 20.0, 5.0])),
    ])
}
