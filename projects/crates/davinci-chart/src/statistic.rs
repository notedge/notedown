//! Minimal row aggregation executed during compilation.
//!
//! Statistics never run in a renderer. [`aggregate`] groups observations by
//! their non-aggregated channel values in first-appearance order and computes
//! one of `count`, `sum`, `mean`, `min`, or `max` for every group.

use crate::{data::Value, spec::Aggregate};

/// One projected observation: group keys plus the optional measured value.
#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    /// Values of the non-aggregated channel bindings, in channel order.
    pub keys: Vec<Value>,
    /// Value of the aggregated channel binding.
    pub measure: Option<Value>,
}

impl Observation {
    /// Create an observation from its group keys and optional measure.
    pub fn new(keys: Vec<Value>, measure: Option<Value>) -> Self {
        Self { keys, measure }
    }
}

/// One aggregated row: the group keys and the computed measure.
#[derive(Clone, Debug, PartialEq)]
pub struct AggregatedRow {
    /// Values of the non-aggregated channel bindings, in channel order.
    pub keys: Vec<Value>,
    /// The computed measure. `Null` when no numeric value exists.
    pub measure: Value,
}

/// Group observations and apply one aggregate.
///
/// Groups keep first-appearance order, so the output is deterministic. Null
/// and non-numeric measures are skipped by `sum`, `mean`, `min`, and `max`.
/// `count` counts every observation in the group.
pub fn aggregate(observations: &[Observation], aggregate: Aggregate) -> Vec<AggregatedRow> {
    let mut groups: Vec<(Vec<Value>, Accumulator)> = Vec::new();
    for observation in observations {
        let index = match groups.iter().position(|(keys, _)| keys == &observation.keys) {
            Some(index) => index,
            None => {
                groups.push((observation.keys.clone(), Accumulator::default()));
                groups.len() - 1
            }
        };
        let accumulator = &mut groups[index].1;
        accumulator.rows += 1;
        if let Some(value) = &observation.measure {
            if let Some(number) = value.as_number() {
                if number.is_finite() {
                    accumulator.values.push(number);
                }
            }
        }
    }

    groups.into_iter().map(|(keys, accumulator)| AggregatedRow { keys, measure: accumulator.finish(aggregate) }).collect()
}

#[derive(Clone, Debug, Default)]
struct Accumulator {
    rows: usize,
    values: Vec<f64>,
}

impl Accumulator {
    fn finish(&self, aggregate: Aggregate) -> Value {
        match aggregate {
            Aggregate::Count => Value::Number(self.rows as f64),
            Aggregate::Sum => Value::Number(self.values.iter().sum()),
            Aggregate::Mean => mean(&self.values).map(Value::Number).unwrap_or(Value::Null),
            Aggregate::Min => self.values.iter().copied().reduce(f64::min).map(Value::Number).unwrap_or(Value::Null),
            Aggregate::Max => self.values.iter().copied().reduce(f64::max).map(Value::Number).unwrap_or(Value::Null),
        }
    }
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let sum: f64 = values.iter().sum();
    Some(sum / values.len() as f64)
}
