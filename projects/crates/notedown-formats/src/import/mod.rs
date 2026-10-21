//! Format import entry points.

mod common;

pub use common::register_image_assets;

/// Legacy Word `.doc` import.
#[cfg(feature = "doc")]
pub mod doc;
/// Office Open XML WordprocessingML import.
#[cfg(feature = "docx")]
pub mod docx;
/// EPUB OCF and XHTML import.
#[cfg(feature = "epub")]
pub mod epub;
/// HTML import via `oak-html`.
#[cfg(feature = "html")]
pub mod html;
/// Markdown import via `oak-markdown`.
#[cfg(feature = "markdown")]
pub mod markdown;
/// Notedown text import via `oak-notedown`.
#[cfg(feature = "notedown")]
pub mod notedown;
/// PDF import.
#[cfg(feature = "pdf")]
pub mod pdf;
