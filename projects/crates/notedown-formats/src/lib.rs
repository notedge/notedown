#![warn(missing_docs)]
//! Document format import and export around `notedown-ir`.

mod capability;
mod error;
pub mod export;
pub mod import;

pub use capability::{FormatCapability, FormatDirection, FormatStatus};
pub use error::FormatError;

/// Registered format capabilities for tooling probes.
pub fn capabilities() -> Vec<FormatCapability> {
    vec![
        FormatCapability {
            id: "markdown",
            direction: FormatDirection::Import,
            status: FormatStatus::Ready,
        },
        FormatCapability {
            id: "markdown",
            direction: FormatDirection::Export,
            status: FormatStatus::Ready,
        },
        FormatCapability {
            id: "docx",
            direction: FormatDirection::Import,
            status: FormatStatus::Ready,
        },
        FormatCapability {
            id: "epub",
            direction: FormatDirection::Import,
            status: FormatStatus::Partial,
        },
    ]
}
