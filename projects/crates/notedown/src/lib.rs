//! Unified entry for the Notedown document IR and format import/export family.
//!
//! Enable capabilities through crate features. Each module re-exports the
//! corresponding `notedown-*` crate without copying types or adding global
//! state.

#![deny(missing_docs)]

#[cfg(feature = "ir")]
pub mod ir {
    //! Document semantic IR — graph, relations, assets, and provenance.
    pub use notedown_ir::*;
}

#[cfg(feature = "formats")]
pub mod formats {
    //! Document format import and export around `notedown-ir`.
    pub use notedown_formats::*;
}
