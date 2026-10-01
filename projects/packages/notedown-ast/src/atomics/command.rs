use super::*;
use crate::ast::TextSpaceNode;
use std::fmt::{Debug, Display, Formatter};



/// CommandNode
///
/// ```note
/// ()
/// ```
#[derive(Debug)]
pub struct CommandArguments {
    prefill: Option<TextSpaceNode>,
    span: Range<u32>,
}

/// CommandNode
///
/// ```note
/// { }
/// ```
#[derive(Debug)]
pub struct CommandBody {}

impl Display for CommandArguments {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(ref prefill) = self.prefill {
            write!(f, "({})", prefill)
        } else {
            write!(f, "()")
        }
    }
}





impl CommandArguments {
    pub fn with_prefill(self, space: Option<TextSpaceNode>) -> Self {
        Self { prefill: space, ..self }
    }
}
