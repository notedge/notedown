#![deny(missing_docs)]
//! Chart semantics for the DaVinci chart domain.
//!
//! The crate keeps the static chart stages explicit:
//!
//! ```text
//! data -> field semantics -> mark -> encoding -> scale -> statistic
//!      -> ChartCompilePlan
//! ```
//!
//! 1. [`DataSnapshot`] carries columnar observations with explicit
//!    [`FieldKind`] declarations.
//! 2. [`ChartSpec`] binds fields to channels by name, declares one [`Mark`],
//!    the per-channel [`ScaleSpec`], the [`Coordinate`], and optional guides.
//! 3. [`scale`] trains channel domains into mappings and [`statistic`]
//!    collapses observations before scales are trained.
//! 4. [`compile`] validates both inputs, applies the declared statistic,
//!    trains every scale, and produces a deterministic [`ChartCompilePlan`].
//!    The plan holds the canvas, the plot rectangle, trained [`Scale`] results,
//!    final mark geometry, axis ticks, and diagnostics.
//!
//! Chart and diagram are parallel domains: both share `davinci-types` and
//! `davinci-scene`, and neither copies the other's semantic model. A plan
//! never carries an SVG string or a drawing command.

pub mod compile;
pub mod data;
pub mod error;
pub mod plan;
pub mod scale;
pub mod spec;
pub mod statistic;

pub use compile::{ChartOptions, compile};
pub use data::{DataColumn, DataSnapshot, FieldKind, Value};
pub use error::{ChartError, codes};
pub use plan::{AxisPlan, AxisTick, ChartCompilePlan, ChartScale, MarkGeometry, MarkInstance, MarkStyle};
pub use scale::{BandScale, LinearScale, NominalScale, PointScale, Scale, ScaleKind};
pub use spec::{Aggregate, Channel, ChannelEncoding, ChartSpec, Coordinate, Encoding, FieldRef, Guide, Mark, ScaleSpec, Scales};
