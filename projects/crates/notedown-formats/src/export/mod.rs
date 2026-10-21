//! Format export entry points.

/// Legacy Word `.doc` export.
#[cfg(feature = "doc")]
pub mod doc;
/// Office Open XML WordprocessingML export.
#[cfg(feature = "docx")]
pub mod docx;
/// Markdown text export.
#[cfg(feature = "markdown")]
pub mod markdown;
/// Multi-file Markdown project export.
#[cfg(feature = "markdown")]
pub mod markdown_project;
/// PDF export.
#[cfg(feature = "pdf")]
pub mod pdf;

/// HTML export.
pub mod html;
