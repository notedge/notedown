//! Pure scale training for chart channels.
//!
//! Every trainer is a deterministic function of its inputs. Training never
//! reads a scene, a renderer, or a clock. Degenerate domains are expanded so a
//! scale can never divide by zero or produce `NaN`.

use davinci_scene::{Color, MarkerShape};

/// Colors assigned to nominal categories in assignment order.
pub const PALETTE: [Color; 8] = [
    Color::rgb(0x4a, 0x7f, 0xbf),
    Color::rgb(0xe1, 0x84, 0x5c),
    Color::rgb(0x59, 0xa1, 0x4f),
    Color::rgb(0xb0, 0x6a, 0xa0),
    Color::rgb(0xcc, 0x9b, 0x3d),
    Color::rgb(0x3d, 0xa0, 0xb0),
    Color::rgb(0x8c, 0x6d, 0x4f),
    Color::rgb(0xc0, 0x5a, 0x6a),
];

/// Marker shapes assigned to nominal categories in assignment order.
pub const MARKERS: [MarkerShape; 3] = [MarkerShape::Circle, MarkerShape::Triangle, MarkerShape::Diamond];

/// The scale family used by one channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleKind {
    /// A continuous linear scale.
    Linear,
    /// A categorical band scale.
    Band,
    /// A categorical point scale.
    Point,
    /// A categorical color and marker scale.
    Nominal,
}

impl ScaleKind {
    /// A stable lowercase keyword for diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Band => "band",
            Self::Point => "point",
            Self::Nominal => "nominal",
        }
    }
}

/// A continuous linear scale.
#[derive(Clone, Debug, PartialEq)]
pub struct LinearScale {
    /// Inclusive data domain, always `min < max`.
    pub domain: (f64, f64),
    /// Output range. An inverted range, such as bottom-to-top, is supported.
    pub range: (f64, f64),
}

impl LinearScale {
    /// Create a scale, normalizing the domain order and expanding a collapsed
    /// domain.
    pub fn new(domain: (f64, f64), range: (f64, f64)) -> Self {
        Self { domain: normalized_domain(domain), range }
    }

    /// Map a domain value into the output range.
    pub fn map(&self, value: f64) -> f64 {
        let (min, max) = self.domain;
        let (start, end) = self.range;
        start + (value - min) / (max - min) * (end - start)
    }
}

/// A categorical band scale.
///
/// `padding` is both the inner gap between bands and the outer gap before the
/// first and after the last band.
#[derive(Clone, Debug, PartialEq)]
pub struct BandScale {
    /// Categories in declaration order.
    pub domain: Vec<String>,
    /// Output range.
    pub range: (f64, f64),
    /// Padding in the inclusive range `0.0..1.0`.
    pub padding: f64,
    /// Distance between two consecutive band starts.
    pub step: f64,
    /// Width of one band.
    pub band_width: f64,
}

impl BandScale {
    /// Create a band scale over a category domain.
    pub fn new(domain: Vec<String>, range: (f64, f64), padding: f64) -> Self {
        let padding = padding.clamp(0.0, 0.9);
        let count = domain.len() as f64;
        let span = range.1 - range.0;
        let step = if count > 0.0 { span / (count + padding) } else { 0.0 };
        let band_width = step * (1.0 - padding);
        Self { domain, range, padding, step, band_width }
    }

    /// The index of a category.
    pub fn index_of(&self, category: &str) -> Option<usize> {
        self.domain.iter().position(|entry| entry == category)
    }

    /// The left edge of the band at `index`.
    pub fn start_at(&self, index: usize) -> f64 {
        self.range.0 + self.step * (index as f64 + self.padding)
    }

    /// The center of the band at `index`.
    pub fn center_at(&self, index: usize) -> f64 {
        self.start_at(index) + self.band_width * 0.5
    }

    /// The left edge of a category band.
    pub fn position(&self, category: &str) -> Option<f64> {
        self.index_of(category).map(|index| self.start_at(index))
    }

    /// The center of a category band.
    pub fn center(&self, category: &str) -> Option<f64> {
        self.index_of(category).map(|index| self.center_at(index))
    }
}

/// A categorical point scale with no band width.
#[derive(Clone, Debug, PartialEq)]
pub struct PointScale {
    /// Categories in declaration order.
    pub domain: Vec<String>,
    /// Output range.
    pub range: (f64, f64),
    /// Symmetric padding in the inclusive range `0.0..1.0`.
    pub padding: f64,
    /// Distance between two consecutive category positions.
    pub step: f64,
}

impl PointScale {
    /// Create a point scale over a category domain.
    pub fn new(domain: Vec<String>, range: (f64, f64), padding: f64) -> Self {
        let padding = padding.clamp(0.0, 0.9);
        let count = domain.len();
        let span = range.1 - range.0;
        let step = if count > 1 { span / (count as f64 - 1.0 + 2.0 * padding) } else { 0.0 };
        Self { domain, range, padding, step }
    }

    /// The index of a category.
    pub fn index_of(&self, category: &str) -> Option<usize> {
        self.domain.iter().position(|entry| entry == category)
    }

    /// The position of the category at `index`.
    ///
    /// A single category is centered in the range.
    pub fn position_at(&self, index: usize) -> f64 {
        if self.domain.len() <= 1 {
            self.range.0 + (self.range.1 - self.range.0) * 0.5
        }
        else {
            self.range.0 + self.step * (index as f64 + self.padding)
        }
    }

    /// The position of a category.
    pub fn position(&self, category: &str) -> Option<f64> {
        self.index_of(category).map(|index| self.position_at(index))
    }
}

/// A nominal scale for color and marker shape channels.
#[derive(Clone, Debug, PartialEq)]
pub struct NominalScale {
    /// Categories in declaration order.
    pub domain: Vec<String>,
    /// One color per category, cycling through [`PALETTE`].
    pub colors: Vec<Color>,
    /// One marker per category, cycling through [`MARKERS`].
    pub markers: Vec<MarkerShape>,
}

impl NominalScale {
    /// Assign colors and markers to a category domain.
    pub fn new(domain: Vec<String>) -> Self {
        let colors = domain.iter().enumerate().map(|(index, _)| PALETTE[index % PALETTE.len()]).collect();
        let markers = domain.iter().enumerate().map(|(index, _)| MARKERS[index % MARKERS.len()]).collect();
        Self { domain, colors, markers }
    }

    /// The index of a category.
    pub fn index_of(&self, category: &str) -> Option<usize> {
        self.domain.iter().position(|entry| entry == category)
    }

    /// The color assigned to a category.
    pub fn color(&self, category: &str) -> Option<Color> {
        self.index_of(category).map(|index| self.colors[index])
    }

    /// The marker assigned to a category.
    pub fn marker(&self, category: &str) -> Option<MarkerShape> {
        self.index_of(category).map(|index| self.markers[index])
    }
}

/// A trained scale for one channel.
#[derive(Clone, Debug, PartialEq)]
pub enum Scale {
    /// A continuous linear scale.
    Linear(LinearScale),
    /// A categorical band scale.
    Band(BandScale),
    /// A categorical point scale.
    Point(PointScale),
    /// A categorical color and marker scale.
    Nominal(NominalScale),
}

impl Scale {
    /// The scale family.
    pub fn kind(&self) -> ScaleKind {
        match self {
            Self::Linear(_) => ScaleKind::Linear,
            Self::Band(_) => ScaleKind::Band,
            Self::Point(_) => ScaleKind::Point,
            Self::Nominal(_) => ScaleKind::Nominal,
        }
    }

    /// The linear scale, when this is one.
    pub fn linear(&self) -> Option<&LinearScale> {
        match self {
            Self::Linear(scale) => Some(scale),
            _ => None,
        }
    }

    /// The band scale, when this is one.
    pub fn band(&self) -> Option<&BandScale> {
        match self {
            Self::Band(scale) => Some(scale),
            _ => None,
        }
    }

    /// The point scale, when this is one.
    pub fn point(&self) -> Option<&PointScale> {
        match self {
            Self::Point(scale) => Some(scale),
            _ => None,
        }
    }

    /// The nominal scale, when this is one.
    pub fn nominal(&self) -> Option<&NominalScale> {
        match self {
            Self::Nominal(scale) => Some(scale),
            _ => None,
        }
    }

    /// The category position used by band and point scales.
    pub fn position(&self, category: &str) -> Option<f64> {
        match self {
            Self::Band(scale) => scale.center(category),
            Self::Point(scale) => scale.position(category),
            _ => None,
        }
    }
}

/// Train a linear scale from optional explicit domain and observed values.
///
/// Non-finite values are ignored. An empty value list infers the domain
/// `(0.0, 1.0)`. A collapsed domain is expanded symmetrically so mapping never
/// divides by zero.
pub fn train_linear(values: &[f64], domain: Option<(f64, f64)>, range: (f64, f64)) -> LinearScale {
    let domain = domain.unwrap_or_else(|| {
        let finite: Vec<f64> = values.iter().copied().filter(|value| value.is_finite()).collect();
        if finite.is_empty() {
            return (0.0, 1.0);
        }
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for value in finite {
            min = min.min(value);
            max = max.max(value);
        }
        (min, max)
    });
    LinearScale::new(domain, range)
}

/// Train a band scale over a category domain.
pub fn train_band(domain: &[String], range: (f64, f64), padding: f64) -> BandScale {
    BandScale::new(domain.to_vec(), range, padding)
}

/// Train a point scale over a category domain.
pub fn train_point(domain: &[String], range: (f64, f64), padding: f64) -> PointScale {
    PointScale::new(domain.to_vec(), range, padding)
}

/// Train a nominal scale over a category domain.
pub fn train_nominal(domain: &[String]) -> NominalScale {
    NominalScale::new(domain.to_vec())
}

/// Generate deterministic, human-readable linear axis ticks.
///
/// Tick values use a 1-2-5 step, stay inside the domain, and are snapped to
/// the step precision so repeated runs produce identical labels.
pub fn linear_ticks(domain: (f64, f64), count: usize) -> Vec<f64> {
    let (min, max) = normalized_domain(domain);
    let count = count.max(2);
    let span = max - min;
    if !span.is_finite() || span <= 0.0 {
        return vec![min, max];
    }
    let step = nice_step(span / (count as f64 - 1.0));
    if step <= 0.0 || !step.is_finite() {
        return vec![min, max];
    }
    let start = (min / step).ceil() * step;
    let end = (max / step).floor() * step;
    let mut ticks = Vec::new();
    let mut value = start;
    while value <= end + step * 1e-9 && ticks.len() < count * 4 {
        ticks.push(snap(value, step));
        value += step;
    }
    if ticks.len() < 2 { vec![min, max] } else { ticks }
}

fn nice_step(span: f64) -> f64 {
    let exponent = span.log10().floor();
    let magnitude = 10f64.powf(exponent);
    let fraction = span / magnitude;
    let nice = if fraction <= 1.0 {
        1.0
    }
    else if fraction <= 2.0 {
        2.0
    }
    else if fraction <= 5.0 {
        5.0
    }
    else {
        10.0
    };
    nice * magnitude
}

fn snap(value: f64, step: f64) -> f64 {
    let exponent = step.abs().log10().floor();
    let factor = 10f64.powi((-exponent).max(0.0) as i32);
    (value * factor).round() / factor
}

fn normalized_domain(domain: (f64, f64)) -> (f64, f64) {
    let (min, max) = if domain.0.is_finite() && domain.1.is_finite() { (domain.0.min(domain.1), domain.0.max(domain.1)) } else { (0.0, 1.0) };
    if min == max {
        let padding = if min == 0.0 { 1.0 } else { (min.abs() * 0.5).max(0.5) };
        (min - padding, max + padding)
    }
    else {
        (min, max)
    }
}
