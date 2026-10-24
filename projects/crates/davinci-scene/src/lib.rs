#![deny(missing_docs)]
//! Generic vector scene model for the DaVinci diagram subsystem.
//!
//! The crate keeps the two scene stages explicit:
//!
//! - [`SemanticScene`] is the unresolved domain identity tree. It carries
//!   stable ids, [`SemanticRole`], [`SourceRef`], hit regions, resource
//!   references, and local transforms and clips, but no composed display
//!   geometry.
//! - [`DisplayScene`] is the resolved, backend-ready tree. It is an ordered
//!   tree of [`SceneNode`] values with flattened transforms, inherited clips,
//!   and a small drawing style surface.
//!
//! Semantic resolution is the only bridge between them. It is pure and
//! deterministic, and it never performs layout, text measurement, wrapping, or
//! overlap avoidance. A host layout engine places the resolved canvas.
//!
//! Text bounds use a documented approximation so downstream layout can reserve
//! space without pretending to be a font shaper.

mod bounds;
mod error;
mod path;
mod resolve;
mod scene;
mod semantic;
mod svg;

pub use davinci_types::{Bounds, Color, Point, Rect, Size, Transform, codes};
pub use error::SceneError;
pub use path::{PathCommand, PathData};
pub use resolve::SceneResolver;
pub use scene::{DisplayScene, MarkerShape, NodeKind, ResourceRef, SceneNode, SemanticRole, ShapeStyle, SourceRef, TextAnchor, TextStyle};
pub use semantic::{SemanticKind, SemanticNode, SemanticScene};
