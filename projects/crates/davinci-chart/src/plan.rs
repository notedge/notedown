//! Deterministic compile results.
//!
//! A plan contains final geometry only: canvas size, the plot rectangle, the
//! trained scales, one instance per mark or series, axis ticks, and non-fatal
//! diagnostics. It carries no SVG strings, drawing commands, field bindings,
//! statistics, or renderer instructions.

use davinci_scene::{Color, MarkerShape, Point, Rect, Size};
use davinci_types::Diagnostic;

use crate::{
    scale::Scale,
    spec::{Channel, Mark},
};

/// One trained scale attached to its channel.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartScale {
    /// The channel this scale serves.
    pub channel: Channel,
    /// The trained scale result.
    pub scale: Scale,
}

/// The final geometry of one mark instance.
#[derive(Clone, Debug, PartialEq)]
pub enum MarkGeometry {
    /// A bar or other axis-aligned rectangle.
    Rect(Rect),
    /// A point marker with its diameter and shape.
    Point {
        /// Marker center in scene coordinates.
        center: Point,
        /// Full marker diameter in scene units.
        size: f64,
        /// Marker shape resolved from the shape channel.
        shape: MarkerShape,
    },
    /// An open polyline for a line series.
    Polyline(Vec<Point>),
    /// A closed polygon for an area series.
    Polygon(Vec<Point>),
    /// A straight reference segment.
    Segment {
        /// Segment start.
        from: Point,
        /// Segment end.
        to: Point,
    },
    /// A text anchor with its formatted content.
    Label {
        /// Anchor on the text baseline.
        anchor: Point,
        /// Formatted text content.
        text: String,
    },
}

/// Paint attributes resolved for one mark instance.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkStyle {
    /// Interior paint, or `None` for an unfilled shape.
    pub fill: Option<Color>,
    /// Outline paint, or `None` for no outline.
    pub stroke: Option<Color>,
    /// Outline width in scene units.
    pub stroke_width: f64,
    /// Opacity in the inclusive range `0.0..=1.0`.
    pub opacity: f64,
}

impl MarkStyle {
    /// A filled shape without an outline.
    pub fn filled(fill: Color) -> Self {
        Self { fill: Some(fill), stroke: None, stroke_width: 0.0, opacity: 1.0 }
    }

    /// An outlined shape without a fill.
    pub fn stroked(stroke: Color, stroke_width: f64) -> Self {
        Self { fill: None, stroke: Some(stroke), stroke_width, opacity: 1.0 }
    }
}

/// One final mark instance in declaration order.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkInstance {
    /// Stable scene identity.
    pub id: String,
    /// The chart mark that produced this instance.
    pub mark: Mark,
    /// Semantic source descriptor, such as a category label or series name.
    pub source: String,
    /// Final geometry in scene coordinates.
    pub geometry: MarkGeometry,
    /// Resolved paint attributes.
    pub style: MarkStyle,
}

/// One axis tick with its position and label anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct AxisTick {
    /// Formatted tick label.
    pub label: String,
    /// Tick position on the axis line.
    pub position: Point,
    /// Text anchor for the label.
    pub label_position: Point,
}

/// One axis guide with its line and ticks.
#[derive(Clone, Debug, PartialEq)]
pub struct AxisPlan {
    /// The guided channel.
    pub channel: Channel,
    /// Axis line start.
    pub from: Point,
    /// Axis line end.
    pub to: Point,
    /// Ticks in domain declaration order.
    pub ticks: Vec<AxisTick>,
}

/// A deterministic compile result for one chart.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartCompilePlan {
    /// Canvas size.
    pub canvas: Size,
    /// Plot rectangle that all mark geometry stays inside.
    pub plot: Rect,
    /// Font size used for axis label positions.
    pub font_size: f64,
    /// Trained scales in channel declaration order.
    pub scales: Vec<ChartScale>,
    /// Mark instances in deterministic order.
    pub marks: Vec<MarkInstance>,
    /// Axis guides in spec declaration order.
    pub axes: Vec<AxisPlan>,
    /// Non-fatal loss and approximation records.
    pub diagnostics: Vec<Diagnostic>,
}

impl ChartCompilePlan {
    /// Find the trained scale for a channel.
    pub fn scale(&self, channel: Channel) -> Option<&Scale> {
        self.scales.iter().find(|entry| entry.channel == channel).map(|entry| &entry.scale)
    }
}
