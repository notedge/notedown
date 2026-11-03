//! Byte ranges for JSON paths in an authored Vega-Lite document.
//!
//! `serde_json` validates and deserializes the document but discards positions.
//! Rejections therefore carry a JSON path, and this scanner maps that same
//! path back onto the authored bytes so a diagnostic names both the construct
//! and the text that produced it. The scanner only records ranges: it never
//! validates, evaluates, or re-parses to a second model, and malformed input
//! simply stops recording instead of producing a competing error.

use std::{collections::HashMap, ops::Range};

/// JSON paths mapped onto half-open byte ranges in the authored source.
#[derive(Debug, Default)]
pub(crate) struct JsonSpans {
    spans: HashMap<String, Range<usize>>,
    document: Range<usize>,
}

impl JsonSpans {
    /// Record every value path and the document range.
    pub(crate) fn scan(source: &str) -> Self {
        let mut scanner = Scanner { bytes: source.as_bytes(), cursor: 0, spans: HashMap::new() };
        let start = scanner.skip_whitespace(0);
        scanner.scan_value("$");
        let end = scanner.trimmed_end(start);
        let mut spans = scanner.spans;
        if start >= end {
            spans.remove("$");
            return Self { spans, document: 0..0 };
        }
        spans.insert("$".to_string(), start..end);
        Self { spans, document: start..end }
    }

    /// The byte range recorded for `path`.
    pub(crate) fn span(&self, path: &str) -> Option<Range<usize>> {
        self.spans.get(path).cloned()
    }

    /// The whole-document byte range, or `0..0` for empty input.
    pub(crate) fn document(&self) -> Range<usize> {
        self.document.clone()
    }
}

struct Scanner<'a> {
    bytes: &'a [u8],
    cursor: usize,
    spans: HashMap<String, Range<usize>>,
}

impl Scanner<'_> {
    fn skip_whitespace(&self, mut index: usize) -> usize {
        while index < self.bytes.len() && matches!(self.bytes[index], b' ' | b'\t' | b'\n' | b'\r') {
            index += 1;
        }
        index
    }

    fn trimmed_end(&self, start: usize) -> usize {
        let mut end = self.bytes.len();
        while end > start && matches!(self.bytes[end - 1], b' ' | b'\t' | b'\n' | b'\r') {
            end -= 1;
        }
        end
    }

    fn scan_value(&mut self, path: &str) {
        let start = self.skip_whitespace(self.cursor);
        if start >= self.bytes.len() {
            return;
        }
        self.cursor = start;
        match self.bytes[start] {
            b'{' => self.scan_object(path),
            b'[' => self.scan_array(path),
            b'"' => self.cursor = self.read_string(start).map(|(_, end)| end).unwrap_or(start + 1),
            _ => {
                let mut end = start;
                while end < self.bytes.len() && !matches!(self.bytes[end], b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r') {
                    end += 1;
                }
                self.cursor = end;
            }
        }
        let end = self.skip_whitespace(self.cursor);
        self.cursor = end;
        if start < end {
            self.spans.insert(path.to_string(), start..end);
        }
    }

    fn scan_object(&mut self, path: &str) {
        self.cursor = self.skip_whitespace(self.cursor) + 1;
        loop {
            self.cursor = self.skip_whitespace(self.cursor);
            if self.cursor >= self.bytes.len() || self.bytes[self.cursor] == b'}' {
                self.cursor = (self.cursor + 1).min(self.bytes.len());
                return;
            }
            if self.bytes[self.cursor] != b'"' {
                return;
            }
            let Some((key, end)) = self.read_string(self.cursor)
            else {
                return;
            };
            self.cursor = self.skip_whitespace(end);
            if self.cursor >= self.bytes.len() || self.bytes[self.cursor] != b':' {
                return;
            }
            self.cursor = self.skip_whitespace(self.cursor + 1);
            self.scan_value(&format!("{path}.{key}"));
            self.cursor = self.skip_whitespace(self.cursor);
            match self.bytes.get(self.cursor) {
                Some(b',') => self.cursor += 1,
                Some(b'}') => {
                    self.cursor += 1;
                    return;
                }
                _ => return,
            }
        }
    }

    fn scan_array(&mut self, path: &str) {
        self.cursor = self.skip_whitespace(self.cursor) + 1;
        let mut index = 0usize;
        loop {
            self.cursor = self.skip_whitespace(self.cursor);
            if self.cursor >= self.bytes.len() || self.bytes[self.cursor] == b']' {
                self.cursor = (self.cursor + 1).min(self.bytes.len());
                return;
            }
            self.scan_value(&format!("{path}[{index}]"));
            index += 1;
            self.cursor = self.skip_whitespace(self.cursor);
            match self.bytes.get(self.cursor) {
                Some(b',') => self.cursor += 1,
                Some(b']') => {
                    self.cursor += 1;
                    return;
                }
                _ => return,
            }
        }
    }

    /// Read a JSON string starting at `start` and return its decoded text and
    /// the index just past the closing quote.
    fn read_string(&self, start: usize) -> Option<(String, usize)> {
        let mut index = start + 1;
        let mut text = String::new();
        let mut chunk_start = index;
        while index < self.bytes.len() {
            match self.bytes[index] {
                b'"' => {
                    text.push_str(&self.source_slice(chunk_start, index));
                    return Some((text, index + 1));
                }
                b'\\' => {
                    text.push_str(&self.source_slice(chunk_start, index));
                    let escaped = *self.bytes.get(index + 1)?;
                    index += 2;
                    match escaped {
                        b'"' | b'\\' | b'/' => text.push(escaped as char),
                        b'b' => text.push('\u{8}'),
                        b'f' => text.push('\u{c}'),
                        b'n' => text.push('\n'),
                        b'r' => text.push('\r'),
                        b't' => text.push('\t'),
                        b'u' => {
                            let digits = self.source_slice(index, index + 4);
                            let code = u32::from_str_radix(&digits, 16).ok()?;
                            index += 4;
                            if let Some(ch) = char::from_u32(code) {
                                text.push(ch);
                            }
                        }
                        _ => text.push(escaped as char),
                    }
                    chunk_start = index;
                }
                _ => index += 1,
            }
        }
        None
    }

    /// Slice the authored source between byte indices.
    fn source_slice(&self, start: usize, end: usize) -> String {
        // The scanner only slices at positions it reached by walking UTF-8
        // text, so the range is always on character boundaries.
        let bytes = &self.bytes[start.min(self.bytes.len())..end.min(self.bytes.len())];
        String::from_utf8_lossy(bytes).into_owned()
    }
}
