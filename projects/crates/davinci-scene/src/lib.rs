#![deny(missing_docs)]
//! Generic vector scene model for the DaVinci diagram subsystem.
//!
//! A [`Scene`] is an ordered tree of [`SceneNode`] values with explicit
//! transforms, optional axis-aligned clips, and a small drawing style surface.
//! It is deliberately independent from diagram semantics, document IRs, and
//! renderers: `davinci-diagram` lowers diagram nodes into this model, and a
//! host layout engine places the resulting canvas.
//!
//! The scene does not perform layout, text measurement, wrapping, or overlap
//! avoidance. Text bounds use a documented approximation so downstream layout
//! can reserve space without pretending to be a font shaper.

mod bounds;
mod error;
mod path;
mod scene;
mod svg;

pub use davinci_types::{Bounds, Color, Point, Rect, Size, Transform, codes};
pub use error::SceneError;
pub use path::{PathCommand, PathData};
pub use scene::{MarkerShape, NodeKind, Scene, SceneNode, ShapeStyle, TextAnchor, TextStyle};
