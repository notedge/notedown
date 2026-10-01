use super::*;

/// `\command: rest of the line`
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CommandLineSpan {
    pub command: IdentifierNode,
    pub rest: TextPlainNode,
    pub span: Range<u32>,
}

impl CommandLineSpan {
    pub fn as_hir(&self) -> CommandNode {
        CommandNode {
            command: self.command.name.clone(),
            arguments: vec![],
            text: self.rest.text.clone(),
            body: "".to_string(),
            span: self.span.clone(),
        }
    }
}
