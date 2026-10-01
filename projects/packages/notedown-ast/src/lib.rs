// lints
#![warn(missing_docs)]
#![allow(clippy::needless_return)]
// documentation
#![doc = include_str!("../Readme.md")]

pub mod ast;
mod atomics;
pub mod hir;
mod link;
mod traits;

pub use crate::{
    ast::{NotedownAST, NotedownTerm},
    atomics::{
        command::{CommandArguments, CommandBody},
        identifier::{AlignNode, LigatureNode, NumberLiteralNode, NumberValueNode},
    },
    hir::command::CommandNode,
    traits::{CodeEngine, MathEngine, NoteGenerator, NoteTransformer},
};

#[cfg(feature = "html-ast")]
pub use crate::traits::html::HtmlBuilder;
#[cfg(feature = "html-ast")]
pub use html_ast::HtmlElement;
