//! Light and dark theme tokens for diagram output.
//!
//! Diagram colors are theme references, never literal values scattered
//! through renderers. The SVG writer inlines both palettes and switches on
//! `prefers-color-scheme`, so one artifact serves light and dark hosts
//! without any scripting. Token values follow the documented GitHub-classic
//! look; dark values are deliberate proposals, marked as such in the design
//! document.

use crate::Color;

/// One palette of the diagram theme, keyed by semantic token.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeTokens {
    /// Node body fill.
    pub node_fill: Color,
    /// Node outline stroke.
    pub node_stroke: Color,
    /// Node and cluster label text.
    pub node_text: Color,
    /// Edge line and arrow fill.
    pub edge_stroke: Color,
    /// Edge label text.
    pub edge_label_text: Color,
    /// Edge label backing plate fill, painted at [`EDGE_LABEL_BG_OPACITY`].
    pub edge_label_bg: Color,
    /// Cluster frame fill.
    pub cluster_fill: Color,
}

/// Opacity of the edge label backing plate.
pub const EDGE_LABEL_BG_OPACITY: f64 = 0.85;

/// The diagram theme: light tokens by default, dark tokens under
/// `prefers-color-scheme: dark`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiagramTheme {
    /// Light palette.
    pub light: ThemeTokens,
    /// Dark palette.
    pub dark: ThemeTokens,
}

/// The shared diagram theme.
pub const DIAGRAM_THEME: DiagramTheme = DiagramTheme {
    light: ThemeTokens {
        node_fill: Color::rgb(0xEC, 0xEC, 0xFF),
        node_stroke: Color::rgb(0x93, 0x70, 0xDB),
        node_text: Color::rgb(0x1F, 0x23, 0x28),
        edge_stroke: Color::rgb(0x33, 0x33, 0x33),
        edge_label_text: Color::rgb(0x33, 0x33, 0x33),
        edge_label_bg: Color::rgb(0xFF, 0xFF, 0xFF),
        cluster_fill: Color::rgb(0xFF, 0xFF, 0xDE),
    },
    dark: ThemeTokens {
        node_fill: Color::rgb(0x2D, 0x2A, 0x4A),
        node_stroke: Color::rgb(0x8B, 0x7E, 0xC8),
        node_text: Color::rgb(0xE6, 0xED, 0xF3),
        edge_stroke: Color::rgb(0x8B, 0x94, 0x9E),
        edge_label_text: Color::rgb(0xE6, 0xED, 0xF3),
        edge_label_bg: Color::rgb(0x0D, 0x11, 0x17),
        cluster_fill: Color::rgb(0x2A, 0x2A, 0x1E),
    },
};

/// WCAG relative luminance of an opaque color.
pub fn relative_luminance(color: Color) -> f64 {
    let channel = |raw: u8| {
        let c = raw as f64 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b) = (channel(color.red()), channel(color.green()), channel(color.blue()));
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// WCAG contrast ratio between two opaque colors, in `1.0..=21.0`.
pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (lighter, darker) = if la >= lb { (la, lb) } else { (lb, la) };
    (lighter + 0.05) / (darker + 0.05)
}
