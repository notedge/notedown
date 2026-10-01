use super::*;

/// `$italic$`
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InlineMathSpan {
    pub text: ParagraphSpan,
    pub span: Range<u32>,
}

/// `$$italic$$`
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DisplayMathSpan {
    pub text: ParagraphSpan,
    pub span: Range<u32>,
}

impl InlineMathSpan {
    pub fn as_hir(&self) -> CodeNode {
        CodeNode { language: "tex".to_string(), span: self.span.clone(), code: "".to_string() }
    }
}

impl DisplayMathSpan {
    pub fn as_hir(&self) -> CodeNode {
        CodeNode { language: "".to_string(), span: self.span.clone(), code: "".to_string() }
    }
}
