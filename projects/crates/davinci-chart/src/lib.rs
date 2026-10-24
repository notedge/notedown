#![deny(missing_docs)]
//! Chart semantics for the DaVinci chart domain.
//!
//! [`DataSnapshot`] carries columnar observations with explicit [`FieldKind`]
//! declarations. Every later stage reads it, and it validates itself before
//! anything else consumes it.
//!
//! Chart and diagram are parallel domains: both share `davinci-types` and
//! `davinci-scene`, and neither copies the other's semantic model.

pub mod data;
pub mod error;

pub use data::{DataColumn, DataSnapshot, FieldKind, Value};
pub use error::{ChartError, codes};
