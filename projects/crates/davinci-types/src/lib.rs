#![deny(missing_docs)]
//! Shared geometry, color, transform, and diagnostic primitives for the
//! DaVinci diagram subsystem.
//!
//! The crate has no dependency on `notedown-ir`, Oak, VuTeX, or a renderer.
//! `davinci-scene` builds the generic vector scene model on top of these
//! primitives, and `davinci-diagram` lowers diagram semantics into that scene.

mod color;
mod diagnostics;
mod geometry;
mod rejections;
mod text_metrics;
mod theme;
mod transform;

pub use color::Color;
pub use diagnostics::{
    ByteRange, Diagnostic, DiagnosticLabel, DiagnosticLocation, DiagnosticOrigin, DiagnosticSeverity, LabelRole, Message, NAMESPACE, SourceRef,
    codes, diagnostic, origin,
};
pub use geometry::{Bounds, Point, Rect, Size};
pub use rejections::{CATALOG_JSON, CodeDoc, MatrixRow, RejectionCase, cases, cases_for, code_docs, matrix};
pub use text_metrics::{measure_line, wrap_label};
pub use theme::{DIAGRAM_THEME, DiagramTheme, EDGE_LABEL_BG_OPACITY, ThemeTokens, contrast_ratio, relative_luminance};
pub use transform::Transform;
