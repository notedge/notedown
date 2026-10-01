use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CommandNode {
    pub command: String,
    pub arguments: Vec<String>,
    pub text: String,
    pub body: String,
    pub span: Range<u32>,
}

impl CommandNode {
    pub fn text<T: ToString>(command: T, span: Range<u32>) -> CommandNode {
        CommandNode { command: command.to_string(), arguments: Vec::new(), text: String::new(), body: "".to_string(), span }
    }
    pub fn with_text<T: ToString>(self, text: T) -> CommandNode {
        CommandNode { text: text.to_string(), ..self }
    }
}
