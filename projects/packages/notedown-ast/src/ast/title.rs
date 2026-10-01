use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HeadingSpan {
    pub level: usize,
    pub text: ParagraphSpan,
    pub span: Range<u32>,
}

/// need in start of line
///
/// ```note
/// 
/// ===
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HorizontalRuleNode {
    pub marks: usize,
    pub span: Range<u32>,
}

impl HeadingSpan {
    pub fn as_hir(&self) -> HeadingNode {
        HeadingNode { level: HeadingLevel::from(self.level), id: "".to_string(), terms: self.text.as_hir(), span: self.span.clone() }
    }
}
