#![allow(clippy::needless_return)]
#![deny(missing_docs)]

//! # Text utility function library
//!
//! ## Interface
//! This library is biased towards input strings and returns new strings
//! This library uses polymorphic interface
//! - if accept [`String`], use (`impl Into<String>`)
//! - if accept [`&str`], use (`impl AsRef<str>`)

mod errors;
mod utils;

pub use errors::{Result, TextError};
pub use slugify::slugify;
pub use utils::*;
