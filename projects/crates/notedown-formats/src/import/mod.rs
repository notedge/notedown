//! Format import entry points.

mod common;

#[cfg(feature = "docx")]
pub mod docx;
#[cfg(feature = "doc")]
pub mod doc;
#[cfg(feature = "pdf")]
pub mod pdf;
#[cfg(feature = "epub")]
pub mod epub;
#[cfg(feature = "html")]
pub mod html;
#[cfg(feature = "markdown")]
pub mod markdown;
#[cfg(feature = "notedown")]
pub mod notedown;
