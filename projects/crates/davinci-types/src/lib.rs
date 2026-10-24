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
mod transform;

pub use color::Color;
pub use diagnostics::{Diagnostic, DiagnosticOrigin, DiagnosticSeverity, NAMESPACE, codes, diagnostic, origin};
pub use geometry::{Bounds, Point, Rect, Size};
pub use transform::Transform;
