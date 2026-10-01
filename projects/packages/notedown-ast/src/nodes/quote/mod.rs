use super::*;

/// ## Quote List
/// ```note
/// > part1
/// > part2
///   part2
/// > part3
///
/// > part4
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuoteBlock {
    /// Quote style name
    pub style: Option<String>,
    /// Head part
    pub head: NotedownNodes,
    /// Body part
    pub body: NotedownNodes,
    /// Last quote
    pub quote: Option<String>,
}

impl QuoteBlock {
    /// Constructor of [`QuoteBlock`]
    #[inline]
    pub fn quote(body: NotedownNodes) -> Self {
        Self { style: None, head: vec![], body, quote: None }
    }
    /// Constructor of [`QuoteBlock`]
    #[inline]
    pub fn quote_styled(body: NotedownNodes, style: String) -> Self {
        Self { style: Some(style), head: vec![], body, quote: None }
    }
}

impl NotedownKind {
    /// Constructor of [`QuoteBlock`]
    #[inline]
    pub fn quote(body: NotedownNodes, span: &Span, file: &FileID) -> NotedownNode {
        QuoteBlock::quote(body).into_node(span, file)
    }
    /// Constructor of [`QuoteBlock`]
    #[inline]
    pub fn quote_styled(body: NotedownNodes, style: impl Into<String>, span: &Span, file: &FileID) -> NotedownNode {
        QuoteBlock::quote_styled(body, style.into()).into_node(span, file)
    }
}
