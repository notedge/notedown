//! Leaf geometry values shared by scenes and diagram lowering.

/// A position or two-dimensional vector in scene coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}

impl Point {
    /// The origin `(0, 0)`.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Create a point from its coordinates.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Move the point by the given offsets.
    pub fn offset(self, dx: f64, dy: f64) -> Self {
        Self { x: self.x + dx, y: self.y + dy }
    }

    /// Euclidean distance to another point.
    pub fn distance(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx.hypot(dy)
    }
}

/// A non-negative two-dimensional extent.
///
/// Width and height are not clamped by the constructor. Scene validation
/// rejects negative or non-finite extents before serialization.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    /// Horizontal extent.
    pub width: f64,
    /// Vertical extent.
    pub height: f64,
}

impl Size {
    /// The empty extent `(0, 0)`.
    pub const ZERO: Self = Self { width: 0.0, height: 0.0 };

    /// Create a size from its extents.
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    /// Returns `true` when either extent is zero or negative.
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

/// An axis-aligned rectangle described by its top-left corner and extents.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Horizontal extent.
    pub width: f64,
    /// Vertical extent.
    pub height: f64,
}

impl Rect {
    /// Create a rectangle from its origin and extents.
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    /// Create the smallest rectangle covering two points.
    pub fn from_points(a: Point, b: Point) -> Self {
        let min_x = a.x.min(b.x);
        let min_y = a.y.min(b.y);
        Self { x: min_x, y: min_y, width: (a.x - b.x).abs(), height: (a.y - b.y).abs() }
    }

    /// Create a rectangle from axis-aligned bounds.
    pub fn from_bounds(bounds: Bounds) -> Self {
        Self::new(bounds.min.x, bounds.min.y, bounds.width(), bounds.height())
    }

    /// The right edge coordinate.
    pub fn right(self) -> f64 {
        self.x + self.width
    }

    /// The bottom edge coordinate.
    pub fn bottom(self) -> f64 {
        self.y + self.height
    }

    /// The rectangle center.
    pub fn center(self) -> Point {
        Point::new(self.x + self.width * 0.5, self.y + self.height * 0.5)
    }

    /// Returns `true` when the point is inside or on the rectangle edge.
    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x && point.x <= self.right() && point.y >= self.y && point.y <= self.bottom()
    }

    /// The axis-aligned bounds covered by the rectangle.
    pub fn to_bounds(self) -> Bounds {
        Bounds { min: Point::new(self.x, self.y), max: Point::new(self.right(), self.bottom()) }
    }
}

/// Axis-aligned minimum and maximum corners.
///
/// Construction normalizes the corners, so `min` is always component-wise less
/// than or equal to `max`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// Component-wise minimum corner.
    pub min: Point,
    /// Component-wise maximum corner.
    pub max: Point,
}

impl Bounds {
    /// Create normalized bounds from two arbitrary corners.
    pub fn new(a: Point, b: Point) -> Self {
        Self { min: Point::new(a.x.min(b.x), a.y.min(b.y)), max: Point::new(a.x.max(b.x), a.y.max(b.y)) }
    }

    /// Create bounds from the top-left corner of a rectangle.
    pub fn from_rect(rect: Rect) -> Self {
        rect.to_bounds()
    }

    /// Bounds covering a single point.
    pub fn from_point(point: Point) -> Self {
        Self { min: point, max: point }
    }

    /// Horizontal extent.
    pub fn width(self) -> f64 {
        self.max.x - self.min.x
    }

    /// Vertical extent.
    pub fn height(self) -> f64 {
        self.max.y - self.min.y
    }

    /// Center of the bounds.
    pub fn center(self) -> Point {
        Point::new((self.min.x + self.max.x) * 0.5, (self.min.y + self.max.y) * 0.5)
    }

    /// Returns `true` when the bounds have no area.
    pub fn is_empty(self) -> bool {
        self.width() <= 0.0 || self.height() <= 0.0
    }

    /// Returns `true` when the point is inside or on the bounds edge.
    pub fn contains(self, point: Point) -> bool {
        point.x >= self.min.x && point.x <= self.max.x && point.y >= self.min.y && point.y <= self.max.y
    }

    /// Smallest bounds covering both inputs.
    pub fn union(self, other: Self) -> Self {
        Self {
            min: Point::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: Point::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    /// Intersection with another bounds, or `None` when they are disjoint.
    pub fn intersection(self, other: Self) -> Option<Self> {
        let min = Point::new(self.min.x.max(other.min.x), self.min.y.max(other.min.y));
        let max = Point::new(self.max.x.min(other.max.x), self.max.y.min(other.max.y));
        (min.x <= max.x && min.y <= max.y).then_some(Self { min, max })
    }

    /// The rectangle with the same origin and extents as these bounds.
    pub fn to_rect(self) -> Rect {
        Rect::from_bounds(self)
    }
}
