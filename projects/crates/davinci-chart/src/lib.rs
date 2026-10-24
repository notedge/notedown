#![deny(missing_docs)]
//! Chart semantics compiled into generic DaVinci vector scenes.
//!
//! The crate keeps the chart pipeline explicit:
//!
//! ```text
//! data -> field semantics -> mark -> encoding -> scale -> statistic
//!      -> ChartCompilePlan -> SemanticScene -> DisplayScene
//! ```
//!
//! 1. [`DataSnapshot`] carries columnar observations with explicit
//!    [`FieldKind`] declarations.
//! 2. [`ChartSpec`] binds fields to channels by name, declares one [`Mark`],
//!    the per-channel [`ScaleSpec`], the [`Coordinate`], and optional guides.
//! 3. [`compile`] validates both inputs, applies the declared statistic, trains
//!    every scale, and produces a deterministic [`ChartCompilePlan`]. The plan
//!    holds the canvas, the plot rectangle, trained [`Scale`] results, final
//!    mark geometry, axis ticks, and diagnostics.
//! 4. [`render`] lowers a plan into a [`SemanticScene`]. It keeps mark instance
//!    ids and source references, and never assembles SVG.
//! 5. [`SemanticScene::resolve`] produces the backend-ready
//!    [`DisplayScene`](davinci_scene::DisplayScene), which is the only scene
//!    that serializes through
//!    [`DisplayScene::to_svg`](davinci_scene::DisplayScene::to_svg).
//!
//! [`lower`] chains compilation and rendering into a [`LoweredChart`].
//!
//! Chart and diagram are parallel domains: both share `davinci-types` and
//! `davinci-scene`, and neither copies the other's semantic model. A scene
//! never carries field bindings, encodings, statistics, or scales.

pub mod compile;
pub mod data;
pub mod error;
pub mod plan;
pub mod render;
pub mod scale;
pub mod spec;
pub mod statistic;

pub use davinci_types::Diagnostic;

pub use compile::{ChartOptions, compile};
pub use data::{DataColumn, DataSnapshot, FieldKind, Value};
pub use error::{ChartError, codes};
pub use plan::{AxisPlan, AxisTick, ChartCompilePlan, ChartScale, MarkGeometry, MarkInstance, MarkStyle};
pub use render::render;
pub use scale::{BandScale, LinearScale, NominalScale, PointScale, Scale, ScaleKind};
pub use spec::{Aggregate, Channel, ChannelEncoding, ChartSpec, Coordinate, Encoding, FieldRef, Guide, Mark, ScaleSpec, Scales};

use davinci_scene::SemanticScene;

/// A complete lowering result: the compile plan, its semantic scene, and
/// diagnostics.
///
/// The scene is semantic only. Resolve it with
/// [`davinci_scene::SceneResolver`] to obtain the display scene consumed by
/// `to_svg` or another renderer.
#[derive(Clone, Debug, PartialEq)]
pub struct LoweredChart {
    /// The deterministic compile result.
    pub plan: ChartCompilePlan,
    /// The semantic scene rendered from [`LoweredChart::plan`].
    pub scene: SemanticScene,
    /// Non-fatal loss and approximation records from compilation.
    pub diagnostics: Vec<Diagnostic>,
}

/// Compile a chart spec and a snapshot, then render the semantic scene.
pub fn lower(spec: &ChartSpec, data: &DataSnapshot, options: &ChartOptions) -> Result<LoweredChart, ChartError> {
    let plan = compile(spec, data, options)?;
    let scene = render::render(&plan);
    let diagnostics = plan.diagnostics.clone();
    Ok(LoweredChart { plan, scene, diagnostics })
}
