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
/// TeX/LaTeX import via `oak-tex`.
#[cfg(feature = "tex")]
pub mod tex;
/// TeX math AST lowering into `notedown-ir::math`.
#[cfg(feature = "tex")]
pub(crate) mod tex_math;
/// Lower a standalone TeX formula source into `notedown-ir::math`.
#[cfg(feature = "tex")]
pub use tex_math::lower_tex_formula_source;
/// Typst import via `oak-typst`.
#[cfg(feature = "typst")]
pub mod typst;
