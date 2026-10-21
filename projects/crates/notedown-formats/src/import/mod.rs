//! Format import entry points.

mod common;

pub use common::register_image_assets;

#[cfg(feature = "doc")]
pub mod doc;
#[cfg(feature = "docx")]
pub mod docx;
#[cfg(feature = "epub")]
pub mod epub;
#[cfg(feature = "html")]
pub mod html;
#[cfg(feature = "markdown")]
pub mod markdown;
#[cfg(feature = "notedown")]
pub mod notedown;
#[cfg(feature = "pdf")]
pub mod pdf;
