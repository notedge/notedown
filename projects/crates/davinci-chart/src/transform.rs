//! Field transformations that a chart declares before scales are trained.
//!
//! `bin` and `timeUnit` are chart semantics, not mark syntax: they decide how a
//! raw column becomes the categorical domain that a band or point scale maps.
//! The spec carries the declaration, [`crate::compile`] resolves it against the
//! data snapshot, and the derived bucket or time-truncated label is what the
//! scene records. Resolutions are integer/UTC arithmetic only, so two runs and
//! two time zones produce identical labels.

use crate::{
    data::{Value, format_number},
    error::{ChartError, codes},
};

/// How a quantitative field is grouped into fixed-width buckets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BinSpec {
    /// Explicit bucket width. Mutually exclusive with `maxbins`.
    pub step: Option<f64>,
    /// Target bucket count used to derive a 1-2-5 step when `step` is absent.
    pub maxbins: Option<u32>,
    /// Explicit lower and upper bounds, or `None` for the finite data extent.
    pub extent: Option<(f64, f64)>,
    /// Align the resolved extent outward to whole multiples of the step.
    pub nice: bool,
}

impl BinSpec {
    /// Default binning: a 1-2-5 step inferred for about ten buckets.
    pub fn new() -> Self {
        Self { step: None, maxbins: Some(DEFAULT_MAXBINS), extent: None, nice: false }
    }

    /// Structurally validate the declaration without touching data.
    pub(crate) fn check(&self) -> Option<&'static str> {
        if let Some(step) = self.step {
            if !step.is_finite() || step <= 0.0 {
                return Some("`step` must be a positive finite number");
            }
        }
        if let Some(maxbins) = self.maxbins {
            if !(MIN_MAXBINS..=MAX_MAXBINS).contains(&maxbins) {
                return Some("`maxbins` must be an integer within 2..=64");
            }
        }
        if self.step.is_some() && self.maxbins.is_some() {
            return Some("`step` and `maxbins` cannot both be declared");
        }
        if let Some((min, max)) = self.extent {
            if !min.is_finite() || !max.is_finite() || min >= max {
                return Some("`extent` needs two finite bounds with min < max");
            }
        }
        None
    }
}

const DEFAULT_MAXBINS: u32 = 10;
const MIN_MAXBINS: u32 = 2;
const MAX_MAXBINS: u32 = 64;

/// The calendar units a temporal field can be truncated to.
///
/// The set is deliberately small: units whose truncation is exact calendar
/// arithmetic in UTC. Quarters, weeks, day-of-year, and daylight-saving
/// aware units are not in the first product slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeUnit {
    /// Calendar year.
    Year,
    /// Calendar month, labelled with its year.
    YearMonth,
    /// Calendar date, labelled with its year and month.
    YearMonthDate,
    /// Month of the year as `01`..`12`.
    Month,
    /// Day of the month as `01`..`31`.
    Date,
    /// Hour of the day as `00`..`23`.
    Hours,
    /// Minute of the hour as `00`..`59`.
    Minutes,
    /// Second of the minute as `00`..`59`.
    Seconds,
    /// Calendar date and hour.
    YearMonthDateHours,
    /// Calendar date, hour, and minute.
    YearMonthDateHoursMinutes,
}

impl TimeUnit {
    /// A stable lowercase keyword used by diagnostics and source references.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Year => "year",
            Self::YearMonth => "yearmonth",
            Self::YearMonthDate => "yearmonthdate",
            Self::Month => "month",
            Self::Date => "date",
            Self::Hours => "hours",
            Self::Minutes => "minutes",
            Self::Seconds => "seconds",
            Self::YearMonthDateHours => "yearmonthdatehours",
            Self::YearMonthDateHoursMinutes => "yearmonthdatehoursminutes",
        }
    }

    /// Resolve an authored keyword, accepting the equivalent `utc` prefix.
    ///
    /// `local:` prefixed units are refused because they depend on the host
    /// time zone and would break byte-identical output.
    pub fn parse(keyword: &str) -> Option<Self> {
        let keyword = keyword.strip_prefix("utc:").unwrap_or(keyword);
        Self::ALL.into_iter().find(|unit| unit.as_str() == keyword)
    }

    /// Every supported unit.
    pub const ALL: [Self; 10] = [
        Self::Year,
        Self::YearMonth,
        Self::YearMonthDate,
        Self::Month,
        Self::Date,
        Self::Hours,
        Self::Minutes,
        Self::Seconds,
        Self::YearMonthDateHours,
        Self::YearMonthDateHoursMinutes,
    ];

    /// The UTC label a value is truncated to.
    ///
    /// Values outside the proleptic Gregorian range accepted by the epoch
    /// milliseconds interface are refused rather than wrapped.
    pub fn label(self, millis: i64) -> Result<String, ChartError> {
        let (year, month, date, hours, minutes, seconds) = utc_parts(millis)?;
        Ok(match self {
            Self::Year => format!("{year:04}"),
            Self::YearMonth => format!("{year:04}-{month:02}"),
            Self::YearMonthDate => format!("{year:04}-{month:02}-{date:02}"),
            Self::Month => format!("{month:02}"),
            Self::Date => format!("{date:02}"),
            Self::Hours => format!("{hours:02}"),
            Self::Minutes => format!("{minutes:02}"),
            Self::Seconds => format!("{seconds:02}"),
            Self::YearMonthDateHours => format!("{year:04}-{month:02}-{date:02}T{hours:02}"),
            Self::YearMonthDateHoursMinutes => format!("{year:04}-{month:02}-{date:02}T{hours:02}:{minutes:02}"),
        })
    }
}

/// One chart-level transformation declared for a positional channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChannelTransform {
    /// Group a quantitative field into fixed-width buckets.
    Bin(BinSpec),
    /// Truncate a temporal field to a calendar unit.
    TimeUnit(TimeUnit),
}

impl ChannelTransform {
    /// A stable keyword for diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bin(_) => "bin",
            Self::TimeUnit(_) => "timeUnit",
        }
    }
}

/// Transform declarations per channel. A missing entry means no transform.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Transforms {
    /// Horizontal channel transform.
    pub x: Option<ChannelTransform>,
    /// Vertical channel transform.
    pub y: Option<ChannelTransform>,
}

impl Transforms {
    /// Read one channel transform.
    pub fn channel(&self, channel: crate::spec::Channel) -> Option<ChannelTransform> {
        match channel {
            crate::spec::Channel::X => self.x,
            crate::spec::Channel::Y => self.y,
            _ => None,
        }
    }

    /// Declare one channel transform.
    pub fn set(&mut self, channel: crate::spec::Channel, transform: ChannelTransform) {
        match channel {
            crate::spec::Channel::X => self.x = Some(transform),
            crate::spec::Channel::Y => self.y = Some(transform),
            _ => {}
        }
    }

    /// Returns `true` when no channel declares a transform.
    pub fn is_empty(&self) -> bool {
        self.x.is_none() && self.y.is_none()
    }
}

/// A transform resolved against the concrete data snapshot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ResolvedTransform {
    Bin(ResolvedBin),
    TimeUnit(TimeUnit),
}

impl ResolvedTransform {
    /// The categorical label a raw value is replaced with.
    pub(crate) fn label(&self, value: &Value) -> Result<Value, ChartError> {
        match self {
            Self::Bin(bin) => {
                let raw = value.as_number().ok_or_else(|| {
                    ChartError::new(codes::TYPE_MISMATCH, "the `bin` transform needs a quantitative field with numeric values")
                })?;
                Ok(Value::text(bin.label(raw)?))
            }
            Self::TimeUnit(unit) => {
                let Value::Timestamp(millis) = value
                else {
                    return Err(ChartError::new(codes::TYPE_MISMATCH, "the `timeUnit` transform needs a temporal field with epoch-millisecond values"));
                };
                Ok(Value::text(unit.label(*millis)?))
            }
        }
    }
}

/// A bin grid after the extent and step are fixed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ResolvedBin {
    origin: f64,
    step: f64,
    count: u64,
}

impl ResolvedBin {
    /// Resolve the declaration against every finite value in the column.
    ///
    /// The data extent is used when the document declares none. An explicit
    /// extent is authoritative: values outside it are refused instead of being
    /// clamped into an edge bucket, because clamping would silently move a
    /// datum into a bucket it does not belong to.
    pub(crate) fn resolve(spec: BinSpec, values: &[f64]) -> Result<Self, ChartError> {
        if let Some(problem) = spec.check() {
            return Err(ChartError::new(codes::INVALID_SPEC, format!("invalid `bin` declaration: {problem}")));
        }
        let (min, max) = match spec.extent {
            Some(extent) => extent,
            None => {
                let min = values.iter().copied().fold(f64::INFINITY, f64::min);
                let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                if !min.is_finite() || !max.is_finite() {
                    return Err(ChartError::new(codes::EMPTY_DOMAIN, "the `bin` extent has no finite data values"));
                }
                if min == max {
                    (min, min + 1.0)
                }
                else {
                    (min, max)
                }
            }
        };
        let span = max - min;
        let step = match spec.step {
            Some(step) => step,
            None => crate::scale::nice_step(span / f64::from(spec.maxbins.unwrap_or(DEFAULT_MAXBINS))),
        };
        if !step.is_finite() || step <= 0.0 {
            return Err(ChartError::new(codes::INVALID_SPEC, "the `bin` step could not be derived from the declared extent"));
        }
        let (origin, upper) = if spec.nice {
            let origin = (min / step).floor() * step;
            let upper = (max / step).ceil() * step;
            (origin, upper)
        }
        else {
            (min, max)
        };
        let count = ((upper - origin) / step).ceil();
        if !count.is_finite() || count <= 0.0 || count > MAX_MAXBINS as f64 {
            return Err(ChartError::new(
                codes::INVALID_SPEC,
                format!("the `bin` declaration produces {} buckets, above the supported maximum of {MAX_MAXBINS}", if count.is_finite() { count as u64 } else { 0 }),
            ));
        }
        Ok(Self { origin, step, count: count as u64 })
    }

    /// The bucket index of a value, using half-open `[lower, upper)` buckets.
    ///
    /// A value exactly on the declared upper bound is folded into the last
    /// bucket within `1e-9 * step` tolerance; everything beyond that tolerance
    /// is refused.
    pub(crate) fn index(&self, value: f64) -> Result<u64, ChartError> {
        let ratio = (value - self.origin) / self.step;
        let tolerance = 1e-9;
        if ratio < -tolerance {
            return Err(ChartError::new(
                codes::INVALID_SPEC,
                format!("value `{value}` lies below the declared `bin` extent and would be clamped"),
            ));
        }
        let mut index = ratio.floor();
        if ratio - index > 1.0 - tolerance {
            index += 1.0;
        }
        let index = index.max(0.0) as u64;
        if index >= self.count {
            let upper = self.origin + self.step * self.count as f64;
            if value <= upper + self.step * tolerance {
                return Ok(self.count - 1);
            }
            return Err(ChartError::new(
                codes::INVALID_SPEC,
                format!("value `{value}` lies above the declared `bin` extent and would be clamped"),
            ));
        }
        Ok(index)
    }

    /// The lower bound label of the bucket that contains `value`.
    pub(crate) fn label(&self, value: f64) -> Result<String, ChartError> {
        let index = self.index(value)?;
        Ok(format_number(self.origin + self.step * index as f64))
    }
}

/// Split epoch milliseconds into UTC calendar parts.
fn utc_parts(millis: i64) -> Result<(i64, u32, u32, u32, u32, u32), ChartError> {
    const MILLIS_PER_DAY: i64 = 86_400_000;
    const MAX_ABS_MILLIS: i64 = 8_640_000_000_000_000;
    if !(-MAX_ABS_MILLIS..=MAX_ABS_MILLIS).contains(&millis) {
        return Err(ChartError::new(codes::INVALID_SPEC, format!("temporal value `{millis}` is outside the supported epoch range")));
    }
    let days = millis.div_euclid(MILLIS_PER_DAY);
    let rest = millis.rem_euclid(MILLIS_PER_DAY);
    let (year, month, date) = civil_from_days(days);
    let hours = (rest / 3_600_000) as u32;
    let minutes = ((rest / 60_000) % 60) as u32;
    let seconds = ((rest / 1_000) % 60) as u32;
    Ok((year, month, date, hours, minutes, seconds))
}

/// Convert days since 1970-01-01 into a proleptic Gregorian date.
///
/// This is the integer civil-date algorithm shared by every calendar-aware
/// toolchain: no floating point, no time zone, no locale.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = (days - era * 146_097) as u64;
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let date = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32;
    let month = if month_index < 10 { month_index + 3 } else { month_index - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, date)
}
