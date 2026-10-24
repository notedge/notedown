//! Source-independent chart semantics.
//!
//! A [`ChartSpec`] describes what a chart *means*: declared fields, one mark,
//! name-based channel bindings, optional aggregates, scale declarations, a
//! cartesian coordinate, and optional guides. It carries no positions, sizes,
//! drawing commands, or source syntax. [`crate::compile`] turns it into a
//! compile plan.

use davinci_types::Diagnostic;

use crate::{
    data::FieldKind,
    error::{chart_error, codes},
};

/// The minimum mark set understood by this crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// One point per observation.
    Point,
    /// One rectangle per observation, from the axis baseline to the value.
    Bar,
    /// One polyline per series, ordered along the x channel.
    Line,
    /// One filled polygon per series, closed on the baseline.
    Area,
    /// One reference line across the plot.
    Rule,
    /// One text label per observation.
    Text,
}

impl Mark {
    /// A stable lowercase keyword for diagnostics and scene ids.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Point => "point",
            Self::Bar => "bar",
            Self::Line => "line",
            Self::Area => "area",
            Self::Rule => "rule",
            Self::Text => "text",
        }
    }
}

/// One visual channel that a field can be bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Channel {
    /// Horizontal position.
    X,
    /// Vertical position.
    Y,
    /// Categorical color.
    Color,
    /// Mark size.
    Size,
    /// Marker shape.
    Shape,
}

impl Channel {
    /// Every channel in declaration order.
    pub const ALL: [Self; 5] = [Self::X, Self::Y, Self::Color, Self::Size, Self::Shape];

    /// A stable lowercase keyword for diagnostics and scene ids.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Color => "color",
            Self::Size => "size",
            Self::Shape => "shape",
        }
    }
}

/// A statistic applied to one channel during compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aggregate {
    /// The number of observations in the group.
    Count,
    /// The sum of the measured values.
    Sum,
    /// The arithmetic mean of the measured values.
    Mean,
    /// The smallest measured value.
    Min,
    /// The largest measured value.
    Max,
}

impl Aggregate {
    /// A stable lowercase keyword for diagnostics and source references.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Sum => "sum",
            Self::Mean => "mean",
            Self::Min => "min",
            Self::Max => "max",
        }
    }
}

/// A named field with its declared kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldRef {
    /// Field name shared with the data snapshot and the encodings.
    pub name: String,
    /// Declared semantic kind of the field.
    pub kind: FieldKind,
}

impl FieldRef {
    /// Create a field reference from its name and kind.
    pub fn new(name: impl Into<String>, kind: FieldKind) -> Self {
        Self { name: name.into(), kind }
    }
}

/// The binding of one channel to one field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelEncoding {
    /// The bound field. Bindings are always explicit names, never positions.
    pub field: FieldRef,
    /// Optional statistic executed during compilation.
    pub aggregate: Option<Aggregate>,
}

impl ChannelEncoding {
    /// Bind a channel to a field without an aggregate.
    pub fn new(field: FieldRef) -> Self {
        Self { field, aggregate: None }
    }

    /// Bind a channel to a named field without an aggregate.
    pub fn field(name: impl Into<String>, kind: FieldKind) -> Self {
        Self::new(FieldRef::new(name, kind))
    }

    /// Declare the statistic applied to this channel.
    pub fn with_aggregate(mut self, aggregate: Aggregate) -> Self {
        self.aggregate = Some(aggregate);
        self
    }
}

/// The full set of channel bindings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Encoding {
    /// Horizontal position binding.
    pub x: Option<ChannelEncoding>,
    /// Vertical position binding.
    pub y: Option<ChannelEncoding>,
    /// Color binding.
    pub color: Option<ChannelEncoding>,
    /// Size binding.
    pub size: Option<ChannelEncoding>,
    /// Marker shape binding.
    pub shape: Option<ChannelEncoding>,
}

impl Encoding {
    /// Read one channel binding.
    pub fn channel(&self, channel: Channel) -> Option<&ChannelEncoding> {
        match channel {
            Channel::X => self.x.as_ref(),
            Channel::Y => self.y.as_ref(),
            Channel::Color => self.color.as_ref(),
            Channel::Size => self.size.as_ref(),
            Channel::Shape => self.shape.as_ref(),
        }
    }

    /// Replace one channel binding.
    pub fn set(&mut self, channel: Channel, encoding: ChannelEncoding) {
        match channel {
            Channel::X => self.x = Some(encoding),
            Channel::Y => self.y = Some(encoding),
            Channel::Color => self.color = Some(encoding),
            Channel::Size => self.size = Some(encoding),
            Channel::Shape => self.shape = Some(encoding),
        }
    }

    /// Returns `true` when the encoding binds at least one positional channel.
    pub fn has_position(&self) -> bool {
        self.x.is_some() || self.y.is_some()
    }
}

/// How one channel is scaled.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ScaleSpec {
    /// Pick the scale from the field kind, channel, and mark.
    #[default]
    Auto,
    /// A continuous linear scale with an inferred or explicit domain.
    Linear {
        /// Explicit `(min, max)` domain, or `None` to infer it from the data.
        domain: Option<(f64, f64)>,
    },
    /// A categorical band scale.
    Band {
        /// Inner and outer padding in the inclusive range `0.0..1.0`.
        padding: f64,
    },
    /// A categorical point scale with no band width.
    Point {
        /// Symmetric padding in the inclusive range `0.0..1.0`.
        padding: f64,
    },
    /// A nominal scale for color and shape channels.
    Nominal,
}

impl ScaleSpec {
    /// A linear scale with an inferred domain.
    pub fn linear() -> Self {
        Self::Linear { domain: None }
    }

    /// A linear scale with an explicit domain.
    pub fn linear_domain(min: f64, max: f64) -> Self {
        Self::Linear { domain: Some((min, max)) }
    }

    /// A band scale with explicit padding.
    pub fn band(padding: f64) -> Self {
        Self::Band { padding }
    }

    /// A point scale with explicit padding.
    pub fn point(padding: f64) -> Self {
        Self::Point { padding }
    }
}

/// Scale declarations per channel. A missing declaration means
/// [`ScaleSpec::Auto`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scales {
    /// Horizontal scale declaration.
    pub x: Option<ScaleSpec>,
    /// Vertical scale declaration.
    pub y: Option<ScaleSpec>,
    /// Color scale declaration.
    pub color: Option<ScaleSpec>,
    /// Size scale declaration.
    pub size: Option<ScaleSpec>,
    /// Shape scale declaration.
    pub shape: Option<ScaleSpec>,
}

impl Scales {
    /// Read one scale declaration.
    pub fn channel(&self, channel: Channel) -> Option<&ScaleSpec> {
        match channel {
            Channel::X => self.x.as_ref(),
            Channel::Y => self.y.as_ref(),
            Channel::Color => self.color.as_ref(),
            Channel::Size => self.size.as_ref(),
            Channel::Shape => self.shape.as_ref(),
        }
    }

    /// Declare the scale for one channel.
    pub fn set(&mut self, channel: Channel, spec: ScaleSpec) {
        match channel {
            Channel::X => self.x = Some(spec),
            Channel::Y => self.y = Some(spec),
            Channel::Color => self.color = Some(spec),
            Channel::Size => self.size = Some(spec),
            Channel::Shape => self.shape = Some(spec),
        }
    }
}

/// The coordinate system of a chart.
///
/// Only cartesian coordinates are implemented. Polar and geographic
/// coordinates are later slices and cannot be expressed here yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Coordinate {
    /// Two perpendicular linear axes.
    #[default]
    Cartesian,
}

/// One optional guide declared by the spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guide {
    /// An axis line with ticks for one positional channel.
    Axis {
        /// The guided channel.
        channel: Channel,
    },
    /// A legend for one non-positional channel.
    Legend {
        /// The guided channel.
        channel: Channel,
    },
}

impl Guide {
    /// An axis guide for a positional channel.
    pub fn axis(channel: Channel) -> Self {
        Self::Axis { channel }
    }

    /// A legend guide for a non-positional channel.
    pub fn legend(channel: Channel) -> Self {
        Self::Legend { channel }
    }
}

/// A complete, source-independent chart description.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartSpec {
    /// Declared fields. Encoding bindings must reference these by name.
    pub fields: Vec<FieldRef>,
    /// The chart mark.
    pub mark: Mark,
    /// Name-based channel bindings.
    pub encoding: Encoding,
    /// Scale declarations. Missing entries fall back to [`ScaleSpec::Auto`].
    pub scales: Scales,
    /// Chart coordinate system.
    pub coordinate: Coordinate,
    /// Guides in declaration order. An empty list suppresses axes.
    pub guides: Vec<Guide>,
}

impl ChartSpec {
    /// Create a spec for a mark with the default cartesian coordinates and
    /// both positional axis guides.
    pub fn new(mark: Mark) -> Self {
        Self {
            fields: Vec::new(),
            mark,
            encoding: Encoding::default(),
            scales: Scales::default(),
            coordinate: Coordinate::Cartesian,
            guides: vec![Guide::axis(Channel::X), Guide::axis(Channel::Y)],
        }
    }

    /// Find a declared field by name.
    pub fn field(&self, name: &str) -> Option<&FieldRef> {
        self.fields.iter().find(|field| field.name == name)
    }

    /// Declare a field.
    pub fn with_field(mut self, field: FieldRef) -> Self {
        self.fields.push(field);
        self
    }

    /// Bind one channel.
    pub fn with_encoding(mut self, channel: Channel, encoding: ChannelEncoding) -> Self {
        self.encoding.set(channel, encoding);
        self
    }

    /// Declare one channel scale.
    pub fn with_scale(mut self, channel: Channel, spec: ScaleSpec) -> Self {
        self.scales.set(channel, spec);
        self
    }

    /// Replace the guide list.
    pub fn with_guides(mut self, guides: Vec<Guide>) -> Self {
        self.guides = guides;
        self
    }

    /// Validate the spec without touching data.
    ///
    /// Checks duplicate or blank field names, undeclared encoding fields,
    /// kind mismatches between declarations and bindings, unsupported
    /// mark-channel pairs, aggregate eligibility, and scale compatibility.
    pub fn validate(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        let mut names: Vec<&str> = Vec::new();
        for field in &self.fields {
            if field.name.is_empty() {
                diagnostics.push(chart_error(codes::INVALID_SPEC, "chart field has an empty name"));
                continue;
            }
            if names.contains(&field.name.as_str()) {
                diagnostics.push(chart_error(codes::DUPLICATE_FIELD, format!("duplicate chart field `{}`", field.name)));
            }
            else {
                names.push(field.name.as_str());
            }
        }

        for channel in Channel::ALL {
            let Some(encoding) = self.encoding.channel(channel)
            else {
                continue;
            };
            if !supports(self.mark, channel) {
                diagnostics.push(chart_error(
                    codes::INVALID_SPEC,
                    format!("mark `{}` does not support the `{}` channel", self.mark.as_str(), channel.as_str()),
                ));
                continue;
            }
            let Some(field) = self.field(&encoding.field.name)
            else {
                diagnostics.push(chart_error(
                    codes::UNKNOWN_FIELD,
                    format!(
                        "encoding field `{}` on the `{}` channel is not declared in `ChartSpec::fields`",
                        encoding.field.name,
                        channel.as_str()
                    ),
                ));
                continue;
            };
            if field.kind != encoding.field.kind {
                diagnostics.push(chart_error(
                    codes::TYPE_MISMATCH,
                    format!(
                        "encoding field `{}` is declared as {} but bound as {}",
                        field.name,
                        field.kind.as_str(),
                        encoding.field.kind.as_str()
                    ),
                ));
            }
            if let Some(aggregate) = encoding.aggregate {
                if aggregate != Aggregate::Count && !field.kind.is_quantitative() {
                    diagnostics.push(chart_error(
                        codes::INVALID_SPEC,
                        format!(
                            "aggregate `{}` on the `{}` channel needs a quantitative or temporal field, but `{}` is {}",
                            aggregate.as_str(),
                            channel.as_str(),
                            field.name,
                            field.kind.as_str()
                        ),
                    ));
                }
            }
            if let Some(spec) = self.scales.channel(channel) {
                if let Some(record) = scale_conflict(channel, field.kind, encoding.aggregate, *spec) {
                    diagnostics.push(record);
                }
            }
        }

        if !self.encoding.has_position() {
            diagnostics.push(chart_error(codes::INVALID_SPEC, "chart encoding must bind at least one of `x` or `y`"));
        }
        else if self.mark != Mark::Rule && (self.encoding.x.is_none() || self.encoding.y.is_none()) {
            diagnostics.push(chart_error(codes::INVALID_SPEC, format!("mark `{}` needs both `x` and `y` bindings", self.mark.as_str())));
        }
        diagnostics
    }
}

fn supports(mark: Mark, channel: Channel) -> bool {
    match channel {
        Channel::X | Channel::Y | Channel::Color => true,
        Channel::Size => matches!(mark, Mark::Point),
        Channel::Shape => matches!(mark, Mark::Point | Mark::Line | Mark::Area),
    }
}

fn scale_conflict(channel: Channel, kind: FieldKind, aggregate: Option<Aggregate>, spec: ScaleSpec) -> Option<Diagnostic> {
    let effective = if aggregate.is_some() { FieldKind::Quantitative } else { kind };
    match spec {
        ScaleSpec::Auto => None,
        ScaleSpec::Linear { domain } => {
            if let Some((min, max)) = domain {
                if !min.is_finite() || !max.is_finite() || min >= max {
                    return Some(chart_error(
                        codes::INVALID_SCALE,
                        format!("linear scale on the `{}` channel needs a finite domain with min < max", channel.as_str()),
                    ));
                }
            }
            match channel {
                Channel::Color | Channel::Shape => {
                    Some(chart_error(codes::INVALID_SCALE, format!("the `{}` channel uses a nominal scale in this slice", channel.as_str())))
                }
                Channel::Size | Channel::X | Channel::Y => {
                    if effective.is_quantitative() {
                        None
                    }
                    else {
                        Some(chart_error(
                            codes::INVALID_SCALE,
                            format!("the `{}` channel is categorical and cannot use a linear scale", channel.as_str()),
                        ))
                    }
                }
            }
        }
        ScaleSpec::Band { padding } | ScaleSpec::Point { padding } => {
            if !(0.0..1.0).contains(&padding) {
                return Some(chart_error(
                    codes::INVALID_SCALE,
                    format!("the padding on the `{}` channel must be within 0.0..1.0", channel.as_str()),
                ));
            }
            match channel {
                Channel::Color | Channel::Shape | Channel::Size => {
                    Some(chart_error(codes::INVALID_SCALE, format!("only `x` and `y` use band or point scales, not `{}`", channel.as_str())))
                }
                Channel::X | Channel::Y => {
                    if effective.is_categorical() {
                        None
                    }
                    else {
                        Some(chart_error(
                            codes::INVALID_SCALE,
                            format!("the `{}` channel is quantitative and cannot use a categorical scale", channel.as_str()),
                        ))
                    }
                }
            }
        }
        ScaleSpec::Nominal => match channel {
            Channel::Color | Channel::Shape => None,
            _ => Some(chart_error(
                codes::INVALID_SCALE,
                format!("a nominal scale is only valid on `color` and `shape`, not `{}`", channel.as_str()),
            )),
        },
    }
}
