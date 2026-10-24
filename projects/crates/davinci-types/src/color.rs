//! RGBA colors that serialize directly to CSS and SVG.

/// An 8-bit-per-channel RGBA color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Color {
    /// Opaque black.
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    /// Opaque white.
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);

    /// Create an opaque color from red, green, and blue channels.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create a color from red, green, blue, and alpha channels.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// The red channel.
    pub const fn red(self) -> u8 {
        self.r
    }

    /// The green channel.
    pub const fn green(self) -> u8 {
        self.g
    }

    /// The blue channel.
    pub const fn blue(self) -> u8 {
        self.b
    }

    /// The alpha channel.
    pub const fn alpha(self) -> u8 {
        self.a
    }

    /// Return the same color with a different alpha channel.
    pub const fn with_alpha(self, alpha: u8) -> Self {
        Self { a: alpha, ..self }
    }

    /// Serialize as a CSS/SVG color value.
    ///
    /// Opaque colors use the compact `#rrggbb` form. Translucent colors use
    /// `rgba(r, g, b, a)` with a fixed three-decimal alpha so output stays
    /// byte-stable.
    pub fn to_css(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        }
        else {
            format!("rgba({}, {}, {}, {:.3})", self.r, self.g, self.b, f64::from(self.a) / 255.0)
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}
