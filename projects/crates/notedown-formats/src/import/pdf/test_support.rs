//! PDF import internals exposed for integration tests.

use std::collections::HashMap;

use super::{
    encoding::{
        decode_pdf_string as decode_pdf_string_impl, decode_text_bytes as decode_text_bytes_impl, decode_with_font as decode_with_font_impl,
    },
    fonts::{
        ToUnicodeMap as InnerToUnicodeMap, increment_unicode as increment_unicode_impl, parse_to_unicode_cmap as parse_to_unicode_cmap_impl,
    },
    images::parse_inline_jpeg as parse_inline_jpeg_impl,
    metadata::split_pdf_paragraphs as split_pdf_paragraphs_impl,
};

/// Wrapper around an internal ToUnicode CMap.
pub struct ToUnicodeMap(InnerToUnicodeMap);

impl ToUnicodeMap {
    /// Decode bytes using the CMap.
    pub fn decode(&self, bytes: &[u8]) -> String {
        self.0.decode(bytes)
    }
}

/// Inline JPEG payload extracted from a content stream.
pub struct InlineJpeg {
    /// Raw JPEG bytes.
    pub bytes: Vec<u8>,
}

/// Split PDF paragraph text on newlines.
pub fn split_pdf_paragraphs(text: &str) -> Vec<&str> {
    split_pdf_paragraphs_impl(text).collect()
}

/// Parse a ToUnicode CMap stream.
pub fn parse_to_unicode_cmap(bytes: &[u8]) -> Option<ToUnicodeMap> {
    parse_to_unicode_cmap_impl(bytes).map(ToUnicodeMap)
}

/// Decode text using an optional font CMap.
pub fn decode_with_font(bytes: &[u8], font: Option<&str>, maps: &HashMap<String, ToUnicodeMap>, literal: bool) -> String {
    let inner = maps.iter().map(|(name, map)| (name.clone(), map.0.clone())).collect::<HashMap<String, InnerToUnicodeMap>>();
    decode_with_font_impl(bytes, font, &inner, literal)
}

/// Decode a PDF parenthesized string.
pub fn decode_pdf_string(value: &[u8]) -> String {
    decode_pdf_string_impl(value)
}

/// Decode PDF text bytes with DocEncoding / UTF-16BE heuristics.
pub fn decode_text_bytes(decoded: &[u8]) -> String {
    decode_text_bytes_impl(decoded)
}

/// Parse an inline JPEG image operator sequence.
pub fn parse_inline_jpeg(stream: &[u8], cursor: usize) -> Option<(InlineJpeg, usize)> {
    parse_inline_jpeg_impl(stream, cursor).map(|(image, end)| (InlineJpeg { bytes: image.bytes }, end))
}

/// Increment a UTF-16BE code unit sequence.
pub fn increment_unicode(value: &mut Vec<u8>) -> bool {
    increment_unicode_impl(value)
}
