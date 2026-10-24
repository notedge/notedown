//! Path command data for arbitrary polylines and curves.

use davinci_types::{Bounds, Point};

/// A single absolute path command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathCommand {
    /// Start a new subpath at the point.
    MoveTo(Point),
    /// Draw a straight segment to the point.
    LineTo(Point),
    /// Draw a quadratic Bézier segment with one control point.
    QuadraticTo {
        /// Control point.
        control: Point,
        /// Segment end point.
        to: Point,
    },
    /// Draw a cubic Bézier segment with two control points.
    CubicTo {
        /// First control point.
        control1: Point,
        /// Second control point.
        control2: Point,
        /// Segment end point.
        to: Point,
    },
    /// Close the current subpath.
    Close,
}

/// An ordered sequence of absolute path commands.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathData {
    /// The commands in drawing order.
    pub commands: Vec<PathCommand>,
}

impl PathData {
    /// Create an empty path.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a move command.
    pub fn move_to(mut self, point: Point) -> Self {
        self.commands.push(PathCommand::MoveTo(point));
        self
    }

    /// Append a line command.
    pub fn line_to(mut self, point: Point) -> Self {
        self.commands.push(PathCommand::LineTo(point));
        self
    }

    /// Append a quadratic curve command.
    pub fn quadratic_to(mut self, control: Point, to: Point) -> Self {
        self.commands.push(PathCommand::QuadraticTo { control, to });
        self
    }

    /// Append a cubic curve command.
    pub fn cubic_to(mut self, control1: Point, control2: Point, to: Point) -> Self {
        self.commands.push(PathCommand::CubicTo { control1, control2, to });
        self
    }

    /// Close the current subpath.
    pub fn close(mut self) -> Self {
        self.commands.push(PathCommand::Close);
        self
    }

    /// All points referenced by the commands, including control points.
    ///
    /// Curve bounds are computed from this conservative hull, not from exact
    /// curve extrema.
    pub fn points(&self) -> Vec<Point> {
        let mut points = Vec::new();
        for command in &self.commands {
            match *command {
                PathCommand::MoveTo(point) | PathCommand::LineTo(point) => points.push(point),
                PathCommand::QuadraticTo { control, to } => {
                    points.push(control);
                    points.push(to);
                }
                PathCommand::CubicTo { control1, control2, to } => {
                    points.push(control1);
                    points.push(control2);
                    points.push(to);
                }
                PathCommand::Close => {}
            }
        }
        points
    }

    /// Axis-aligned bounds of the referenced points.
    pub fn bounds(&self) -> Option<Bounds> {
        let points = self.points();
        let first = *points.first()?;
        Some(points.iter().skip(1).fold(Bounds::from_point(first), |bounds, point| bounds.union(Bounds::from_point(*point))))
    }
}
