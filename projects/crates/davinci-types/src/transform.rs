//! Affine two-dimensional transforms.

use crate::geometry::Point;

/// A two-dimensional affine transform stored as the six SVG matrix components
/// `a b c d e f`.
///
/// Constructors cover the operations a scene needs: translation, scaling, and
/// rotation. Composition is exact for any combination of them because the
/// value is kept as a matrix rather than decomposed into separate fields.
///
/// A transform maps a local point `p` to
///
/// ```text
/// x' = a * x + c * y + e
/// y' = b * x + d * y + f
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Transform {
    /// The identity transform.
    pub const IDENTITY: Self = Self { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    /// Create a transform from explicit SVG matrix components.
    pub const fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Self {
        Self { a, b, c, d, e, f }
    }

    /// A pure translation.
    pub const fn translate(x: f64, y: f64) -> Self {
        Self { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: x, f: y }
    }

    /// A pure scale around the local origin.
    pub const fn scale(sx: f64, sy: f64) -> Self {
        Self { a: sx, b: 0.0, c: 0.0, d: sy, e: 0.0, f: 0.0 }
    }

    /// A rotation around the local origin.
    ///
    /// Positive angles rotate clockwise on screen because SVG scene
    /// coordinates point down.
    pub fn rotate_degrees(degrees: f64) -> Self {
        let radians = degrees.to_radians();
        let (sin, cos) = radians.sin_cos();
        Self { a: cos, b: sin, c: -sin, d: cos, e: 0.0, f: 0.0 }
    }

    /// Compose two transforms: apply `self` first, then `next`.
    ///
    /// This is the natural order for nested scene nodes, where a parent
    /// transform is applied after the child transform.
    pub fn then(self, next: Self) -> Self {
        Self {
            a: next.a * self.a + next.c * self.b,
            b: next.b * self.a + next.d * self.b,
            c: next.a * self.c + next.c * self.d,
            d: next.b * self.c + next.d * self.d,
            e: next.a * self.e + next.c * self.f + next.e,
            f: next.b * self.e + next.d * self.f + next.f,
        }
    }

    /// Map a local point through the transform.
    pub fn apply_point(self, point: Point) -> Point {
        Point::new(self.a * point.x + self.c * point.y + self.e, self.b * point.x + self.d * point.y + self.f)
    }

    /// Returns `true` when the transform leaves every point unchanged.
    pub fn is_identity(self) -> bool {
        self == Self::IDENTITY
    }

    /// The `a` matrix component.
    pub fn a(self) -> f64 {
        self.a
    }

    /// The `b` matrix component.
    pub fn b(self) -> f64 {
        self.b
    }

    /// The `c` matrix component.
    pub fn c(self) -> f64 {
        self.c
    }

    /// The `d` matrix component.
    pub fn d(self) -> f64 {
        self.d
    }

    /// The `e` matrix component.
    pub fn e(self) -> f64 {
        self.e
    }

    /// The `f` matrix component.
    pub fn f(self) -> f64 {
        self.f
    }

    /// The signed area scale factor of the linear part.
    pub fn determinant(self) -> f64 {
        self.a * self.d - self.b * self.c
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}
