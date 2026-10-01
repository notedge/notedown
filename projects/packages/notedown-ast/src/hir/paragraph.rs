use super::*;
use crate::ast::TextSpaceNode;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BadNode {
    pub text: String,
    pub span: Range<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParagraphNode {
    pub terms: Vec<ParagraphKind>,
    pub span: Range<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, From)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ParagraphKind {
    /// Normal ast with no white space
    Plain(Box<TextPlainNode>),
    /// Normal ast with white space
    Style(Box<TextStyleNode>),
    /// White space
    Space(Box<TextSpaceNode>),
    /// Inline code
    Code(Box<CodeNode>),
    /// `https://example.com`
    Uri(Box<UriNode>),
    /// Command with text
    Command(Box<CommandNode>),
}
impl ParagraphKind {
    pub fn text<S: ToString>(body: S, span: Range<u32>) -> Self {
        Self::Plain(Box::new(TextPlainNode::new(body, span)))
    }
}
