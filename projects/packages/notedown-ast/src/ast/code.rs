use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CodeInlineSpan {
    pub level: usize,
    pub code: String,
    pub span: Range<u32>,
}

impl CodeInlineSpan {
    pub fn as_hir(&self) -> CodeNode {
        CodeNode::text(self.code.trim(), self.span.clone())
    }
}
