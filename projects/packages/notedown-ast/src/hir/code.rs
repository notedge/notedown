use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CodeNode {
    pub language: String,
    pub code: String,
    pub span: Range<u32>,
}

impl CodeNode {
    pub fn text<T: ToString>(text: T, span: Range<u32>) -> CodeNode {
        CodeNode { language: String::new(), code: text.to_string(), span }
    }
}
