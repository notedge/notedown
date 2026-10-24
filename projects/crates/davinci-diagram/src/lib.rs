#![deny(missing_docs)]
//! Diagram semantics lowered into generic DaVinci vector scenes.
//!
//! The crate starts with Oak Mermaid flowcharts: the typed Oak AST is laid out
//! with a deterministic layered algorithm and emitted as a
//! [`davinci_scene::Scene`]. It does not depend on `notedown-ir`, define
//! document semantics, or render anything itself.
//!
//! Unsupported constructs never disappear silently. Fatal cases return a
//! [`DiagramError`] carrying a structured diagnostic, and approximations such
//! as an ellipse standing in for a circle shape are recorded on
//! [`LoweredDiagram::diagnostics`].

mod error;
mod flowchart;
mod layout;

#[cfg(feature = "mermaid")]
mod parse;

pub use error::DiagramError;
pub use flowchart::{LoweredDiagram, lower_flowchart};

#[cfg(feature = "mermaid")]
pub use flowchart::lower_mermaid;
#[cfg(feature = "mermaid")]
pub use parse::{mermaid_to_scene, parse_mermaid};
