use super::*;
mod display;

#[derive(Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum IgnoreNode {
    /// Whitespace
    WS(TextSpaceNode),
    /// Newline
    NL(NewlineSpan),
}

/// A period of whitespace longer than two newlines, terminated by a newline
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParagraphBreakSpan {
    pub span: Range<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextSpaceNode {
    pub width: usize,
    pub span: Range<u32>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HSpaceNode {
    pub width: f32,
    pub span: Range<u32>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VSpaceNode {
    pub height: f32,
    pub span: Range<u32>,
}

#[derive(Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NewlineSpan {
    pub span: Range<u32>,
}

#[derive(Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlignNode {
    pub span: Range<u32>,
}

impl TextSpaceNode {
    /// Create a new whitespace node with the given width and span.
    pub fn new(width: usize, span: Range<u32>) -> Self {
        Self { width, span }
    }
}

impl NewlineSpan {
    pub fn as_hir(&self) -> TextSpaceNode {
        TextSpaceNode { width: 0, span: self.span.clone() }
    }
}
