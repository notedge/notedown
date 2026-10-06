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
    let mut capabilities = Vec::new();
    #[cfg(feature = "notedown")]
    capabilities.push(FormatCapability { id: "notedown", direction: FormatDirection::Import, status: FormatStatus::Partial });
    #[cfg(feature = "markdown")]
    {
        capabilities.push(FormatCapability { id: "markdown", direction: FormatDirection::Import, status: FormatStatus::Partial });
        capabilities.push(FormatCapability { id: "markdown", direction: FormatDirection::Export, status: FormatStatus::Partial });
    }
    capabilities.push(FormatCapability { id: "html", direction: FormatDirection::Export, status: FormatStatus::Partial });
    #[cfg(feature = "html")]
    capabilities.push(FormatCapability { id: "html", direction: FormatDirection::Import, status: FormatStatus::Partial });
    #[cfg(feature = "docx")]
    {
        capabilities.push(FormatCapability { id: "docx", direction: FormatDirection::Import, status: FormatStatus::Partial });
        capabilities.push(FormatCapability { id: "docx", direction: FormatDirection::Export, status: FormatStatus::Partial });
    }
    #[cfg(feature = "doc")]
    {
        capabilities.push(FormatCapability { id: "doc", direction: FormatDirection::Import, status: FormatStatus::Partial });
        capabilities.push(FormatCapability { id: "doc", direction: FormatDirection::Export, status: FormatStatus::Unavailable });
    }
    #[cfg(feature = "pdf")]
    {
        capabilities.push(FormatCapability { id: "pdf", direction: FormatDirection::Import, status: FormatStatus::Partial });
        capabilities.push(FormatCapability { id: "pdf", direction: FormatDirection::Export, status: FormatStatus::Partial });
    }
    #[cfg(feature = "epub")]
    capabilities.push(FormatCapability { id: "epub", direction: FormatDirection::Import, status: FormatStatus::Partial });
    capabilities
}
