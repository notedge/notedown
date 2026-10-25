#![deny(missing_docs)]
//! Diagram semantics compiled into generic DaVinci vector scenes.
//!
//! The crate keeps the DaVinci three-stage contract explicit:
//!
//! 1. [`spec::DiagramSpec`] describes *what* a diagram means: nodes, edges,
//!    clusters, ports, and ordering constraints. Source frontends lower into
//!    this model and never build scene nodes or SVG directly.
//! 2. [`plan::DiagramLayoutPlan`] is the deterministic compile result: canvas
//!    size, placed rectangles, orthogonal edge routes, cluster frames, and
//!    diagnostics. A plan carries geometry only, never SVG strings, drawing
//!    commands, or renderer instructions.
//! 3. [`davinci_scene::SemanticScene`] is the unresolved domain identity tree
//!    produced by [`render::render`] from a plan. It keeps stable ids, semantic
//!    roles, source references, hit regions, and local transforms.
//! 4. [`davinci_scene::DisplayScene`] is the resolved display tree produced by
//!    [`davinci_scene::SceneResolver`]. It holds flattened geometry and display
//!    attributes only and is the sole input to SVG or another renderer.
//!
//! The Mermaid frontend produces a [`spec::DiagramSpec`] and never assembles
//! SVG. Flowcharts and class diagrams use layered layout; sequence diagrams use
//! a time-ordered layout that reuses the same nodes, edges, plan, and scene
//! surface. Fatal failures return a [`DiagramError`] carrying a structured
//! diagnostic. Non-fatal loss and approximation are recorded in
//! [`DiagramLayoutPlan::diagnostics`] and copied into
//! [`LoweredDiagram::diagnostics`]; nothing is dropped silently.
//!
//! The crate does not depend on `notedown-ir`, define document semantics, or
//! render anything itself.

pub mod compile;
pub mod error;
pub mod plan;
pub mod render;
pub mod spec;

#[cfg(feature = "mermaid")]
pub mod parse;

pub use error::DiagramError;

#[cfg(feature = "mermaid")]
pub use parse::{mermaid_to_scene, mermaid_to_spec, parse_mermaid};

use davinci_scene::DisplayScene;
use davinci_types::Diagnostic;

/// A complete lowering result: the layout plan, its resolved display scene,
/// and diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct LoweredDiagram {
    /// The deterministic compile result.
    pub plan: plan::DiagramLayoutPlan,
    /// The display scene resolved from [`LoweredDiagram::plan`].
    pub scene: DisplayScene,
    /// Non-fatal loss and approximation records from compilation.
    pub diagnostics: Vec<Diagnostic>,
}
