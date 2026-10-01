pub mod command;
pub mod identifier;
pub mod number;

use crate::hir::IdentifierNode;
use std::{
    fmt::Debug,
    ops::Range,
};

// /// Aka. Macro.
// /// It can be understood as a mark that attempts to expand in a given context
// /// until the position cannot be expanded
// #[derive(Clone, Eq, PartialEq)]
// pub enum Command {
//     /// Command in normal form
//     Normal(Box<NormalCommand>),
//     /// Command in escape form
//     Escaped(Box<EscapedCommand>),
//     /// Command in XML form
//     XML(Box<XMLCommand>),
//     /// Command in external form
//     External(Box<ExternalCommand>),
// }
//

// /// Available arguments for the atomics
// /// - positional: `\cmd(a, b, c)`
// /// - optional: `\cmd(a = 1, b = 2)`
// #[derive(Clone, Debug, Default, Eq, PartialEq)]
// pub struct CommandArguments {
//     /// Arguments which depends on position
//     pub positional: Vec<NodeLocation<NotedownValue>>,
//     /// Arguments forms of key-value pairs
//     pub optional: Dict<NotedownValue>,
// }
//
// impl IntoNotedown for Command {
//     fn into_node(self, span: &Span, file: &FileID) -> NotedownNode {
//         NotedownKind::Command(self).into_node(span, file)
//     }
// }
//
// impl Debug for Command {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         match self {
//             Self::Normal(v) => Debug::fmt(v, f),
//             Self::Escaped(v) => Debug::fmt(v, f),
//             Self::XML(v) => Debug::fmt(v, f),
//             Self::External(v) => Debug::fmt(v, f),
//         }
//     }
// }
//
// impl Command {
//     /// Check if the atomics is some given name
//     #[inline]
//     pub fn is(&self, rhs: impl AsRef<str>) -> bool {
//         self.atomics().eq(rhs.as_ref())
//     }
//     /// Get the name of this atomics
//     #[inline]
//     pub fn atomics(&self) -> &str {
//         match self {
//             Self::Normal(v) => v.cmd.as_str(),
//             Self::Escaped(v) => v.cmd.as_str(),
//             Self::XML(v) => v.cmd.as_str(),
//             Self::External(v) => v.cmd.as_str(),
//         }
//     }
// }
//
// impl CommandArguments {
//     /// Returns true if theres no arguments given
//     pub fn is_empty(&self) -> bool {
//         self.positional.is_empty() && self.optional.is_empty()
//     }
// }
