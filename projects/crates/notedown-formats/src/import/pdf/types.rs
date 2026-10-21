pub(crate) struct PdfStream<'a> {
    pub(crate) object_id: Option<u32>,
    pub(crate) dictionary: &'a [u8],
    pub(crate) data: &'a [u8],
}

/// Decoded or referenced image payload extracted from a PDF.
#[derive(Clone, Debug)]
pub(crate) struct PdfImage {
    pub(crate) bytes: Vec<u8>,
    pub(crate) media_type: &'static str,
    pub(crate) source: String,
}

/// Semantic event produced while walking a content stream.
#[derive(Debug)]
pub(crate) enum ContentEvent {
    /// Extracted text run.
    Text(String),
    /// Named XObject reference from a `Do` operator.
    NamedImage(String),
    /// Inline image operator sequence.
    InlineImage(PdfImage),
}

/// Result of walking PDF text operators in a decoded stream.
#[derive(Default)]
pub(crate) struct ExtractedText {
    pub(crate) events: Vec<ContentEvent>,
    pub(crate) spacing_adjustments: bool,
}
