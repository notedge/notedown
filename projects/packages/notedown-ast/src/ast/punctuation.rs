use super::*;
use crate::hir::TextPlainNode;

#[derive(Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CommaNode {
    pub span: Range<u32>,
}

#[derive(Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PeriodNode {
    pub span: Range<u32>,
}

impl Debug for CommaNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "CommaNode({:?})", self.span)
    }
}

impl Debug for PeriodNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "PeriodNode({:?})", self.span)
    }
}
impl CommaNode {
    pub fn as_hir(&self) -> TextPlainNode {
        TextPlainNode::new(",", self.span.clone())
    }
}
impl PeriodNode {
    pub fn as_hir(&self) -> TextPlainNode {
        TextPlainNode::new(".", self.span.clone())
    }
}
