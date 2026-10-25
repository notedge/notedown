//! Typst import via `oak-typst`.
//!
//! The lowering keeps markup semantics the IR can express: headings,
//! paragraphs, strong/emphasis spans, lists, math source, raw code, and links.
//! Dynamic content (`#` expressions, function calls, set/show rules) and any
//! structure the AST cannot express stay explicit opaque blocks or recorded
//! loss markers.
//!
//! Lowering is line-oriented: heading, list, and enum spans can extend past the
//! line they start on, so the lowerer resumes at the first meaningful line
//! boundary and reparses the remaining source rather than treating the whole
//! tail as heading or list content.

use std::path::Path;

use notedown_ir::{
    Block, DocumentGraph, DocumentId, Inline, ListItem, LossMarker, RelationEndpoint, SemanticStatus,
};
use oak_core::{Builder, SourceText, parser::session::ParseSession};
use oak_typst::{
    TypstBuilder, TypstLanguage,
    ast::{TypstHeading, TypstItem, TypstLink, TypstRoot},
};

use super::common::register_image_assets;
use crate::FormatError;

/// Import a Typst file from disk into `notedown-ir`.
pub fn import_typst(path: impl AsRef<Path>) -> Result<DocumentGraph, FormatError> {
    let path = path.as_ref();
    let text =
        std::fs::read_to_string(path).map_err(|error| FormatError::invalid_input(format!("failed to read {}: {error}", path.display())))?;
    import_typst_bytes(&path.display().to_string(), &text)
}

/// Import Typst source text into `notedown-ir`.
pub fn import_typst_bytes(label: &str, text: &str) -> Result<DocumentGraph, FormatError> {
    let root = parse_typst(label, text)?;
    Ok(lower_typst_root(&root, text, document_id_for(label)))
}

fn lower_typst_root(root: &TypstRoot, text: &str, id: DocumentId) -> DocumentGraph {
    let mut lowerer = TypstLowerer { graph: DocumentGraph::new(id), source: text, cursor: 0, line_breaks: 0 };
    let mut sink = Sink::document();
    lowerer.lower_root(&root.items, text.len(), &mut sink);
    sink.flush(&mut lowerer.graph);

    if lowerer.graph.blocks.is_empty() {
        lowerer.loss("import.typst.empty_document", "Typst source produced no supported semantic blocks", SemanticStatus::Partial);
    }
    lowerer.loss(
        "import.typst.partial_coverage",
        "Typst import preserves headings, paragraphs, strong/emphasis spans, lists, math source, raw code, and links, but not dynamic content, function calls, set/show rules, or free layout",
        SemanticStatus::Partial,
    );
    register_image_assets(&mut lowerer.graph);
    lowerer.graph
}

fn parse_typst(label: &str, text: &str) -> Result<TypstRoot, FormatError> {
    let source = SourceText::new(text);
    let language = TypstLanguage::default();
    let builder = TypstBuilder::new(&language);
    let mut session = ParseSession::<TypstLanguage>::default();
    builder.build(&source, &[], &mut session).result.map_err(|error| FormatError::parse("typst", format!("{label}: {error}")))
}

fn document_id_for(label: &str) -> DocumentId {
    let mut hash = 1u64;
    for byte in label.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u64::from(byte));
    }
    DocumentId(hash)
}

struct Sink {
    inlines: Vec<Inline>,
}

impl Sink {
    fn document() -> Self {
        Self { inlines: Vec::new() }
    }

    fn flush(&mut self, graph: &mut DocumentGraph) {
        trim_inlines(&mut self.inlines);
        if self.inlines.is_empty() {
            return;
        }
        let inlines = std::mem::take(&mut self.inlines);
        graph.push_block(Block::Paragraph { content: inlines });
    }
}

struct TypstLowerer<'a> {
    graph: DocumentGraph,
    source: &'a str,
    cursor: usize,
    line_breaks: usize,
}

impl<'a> TypstLowerer<'a> {
    fn raw(&self, start: usize, end: usize) -> &'a str {
        self.source.get(start.min(self.source.len())..end.min(self.source.len())).unwrap_or("")
    }

    fn loss(&mut self, code: &str, message: impl Into<String>, status: SemanticStatus) {
        self.graph.push_loss(LossMarker { code: code.into(), message: message.into(), status });
    }

    fn push_block(&mut self, sink: &mut Sink, block: Block) {
        sink.flush(&mut self.graph);
        self.graph.push_block(block);
    }

    fn lower_root(&mut self, items: &[TypstItem], region_end: usize, sink: &mut Sink) {
        let mut index = 0;
        while index < items.len() {
            if let Some((lines, ordered, end)) = self.list_ahead() {
                self.lower_list_items(lines, ordered, sink);
                self.cursor = end;
                self.lower_tail(end, region_end, sink);
                return;
            }
            match &items[index] {
                TypstItem::Heading(heading) => {
                    if self.lower_heading(heading, sink) {
                        return;
                    }
                    index += 1;
                }
                TypstItem::ListItem(_) | TypstItem::EnumItem(_) => {
                    let ordered = matches!(items[index], TypstItem::EnumItem(_));
                    let mut collected = Vec::new();
                    let mut tail = None;
                    while index < items.len() && matches!(items[index], TypstItem::EnumItem(_)) == ordered
                        && matches!(items[index], TypstItem::ListItem(_) | TypstItem::EnumItem(_))
                    {
                        let span = typst_span(&items[index]);
                        if let Some(span) = span {
                            let raw = self.raw(span.start, span.end);
                            let (mut lines, line_tail) = split_list_lines(raw, ordered);
                            collected.append(&mut lines);
                            tail = line_tail.map(|offset| span.start + offset);
                            self.cursor = span.end;
                        }
                        index += 1;
                        if tail.is_some() {
                            break;
                        }
                    }
                    self.lower_list_items(collected, ordered, sink);
                    if let Some(tail_start) = tail {
                        self.lower_tail(tail_start, region_end, sink);
                        return;
                    }
                }
                TypstItem::Quote(quote) => {
                    if self.lower_expression(quote, sink) {
                        return;
                    }
                    index += 1;
                }
                TypstItem::Raw(_) => {
                    let mut raw = String::new();
                    while index < items.len() {
                        let TypstItem::Raw(chunk) = &items[index] else {
                            break;
                        };
                        raw.push_str(chunk);
                        index += 1;
                    }
                    self.lower_raw(&raw, sink);
                }
                _ => {
                    self.lower_inline_item(&items[index], sink);
                    index += 1;
                }
            }
        }
    }

    fn lower_inline_item(&mut self, item: &TypstItem, sink: &mut Sink) {
        match item {
            TypstItem::Text(text) => {
                if is_comment(text) {
                    return;
                }
                self.advance_text(text);
                self.line_breaks = 0;
                self.push_text(text, sink);
                self.recover_line_comment_url(sink);
            }
            TypstItem::Space => {
                let newlines = self.advance_space();
                if newlines > 0 {
                    self.line_breaks += newlines;
                }
                if self.line_breaks >= 2 {
                    sink.flush(&mut self.graph);
                    self.line_breaks = 0;
                }
                else if !sink.inlines.is_empty() {
                    push_text_inlines(&mut sink.inlines, " ");
                }
            }
            TypstItem::Parbreak => sink.flush(&mut self.graph),
            TypstItem::Strong(root) => {
                let (children, trailing_space) = self.styled_children(root, '*');
                self.cursor = root.span.end;
                sink.inlines.push(Inline::Styled { style: "bold".into(), children });
                if trailing_space {
                    push_text_inlines(&mut sink.inlines, " ");
                }
                self.line_breaks = 0;
            }
            TypstItem::Emphasis(root) => {
                let (children, trailing_space) = self.styled_children(root, '_');
                self.cursor = root.span.end;
                sink.inlines.push(Inline::Styled { style: "italic".into(), children });
                if trailing_space {
                    push_text_inlines(&mut sink.inlines, " ");
                }
                self.line_breaks = 0;
            }
            TypstItem::Math(root) => {
                let (content, display) = math_content(self.raw(root.span.start, root.span.end));
                self.cursor = root.span.end;
                self.line_breaks = 0;
                if display {
                    self.push_block(sink, Block::Math { content, language: Some("typst".into()) });
                }
                else {
                    sink.inlines.push(Inline::InlineMath { content, language: Some("typst".into()) });
                }
            }
            TypstItem::Link(link) => {
                self.line_breaks = 0;
                sink.inlines.push(self.link_inline(link));
            }
            TypstItem::Block(root) => self.lower_root(&root.items, root.span.end, sink),
            TypstItem::Raw(_) | TypstItem::Heading(_) | TypstItem::ListItem(_) | TypstItem::EnumItem(_) | TypstItem::Quote(_) => {
                unreachable!()
            }
        }
    }

    fn lower_raw(&mut self, raw: &str, sink: &mut Sink) {
        self.advance_text(raw);
        self.line_breaks = 0;
        let ticks = raw.trim().chars().take_while(|character| *character == '`').count();
        let (content, language) = raw_content(raw);
        if content.trim().is_empty() {
            return;
        }
        if ticks < 3 {
            sink.inlines.push(Inline::InlineCode { text: content });
            return;
        }
        self.push_block(sink, Block::Code { language, content });
    }

    fn styled_children(&mut self, root: &TypstRoot, delimiter: char) -> (Vec<Inline>, bool) {
        let raw = self.raw(root.span.start, root.span.end);
        let trailing_space = raw.ends_with(char::is_whitespace);
        let inner = raw.trim();
        let inner = inner.strip_prefix(delimiter).unwrap_or(inner);
        let inner = inner.strip_suffix(delimiter).unwrap_or(inner);
        (self.lower_inline_fragment(inner.trim()), trailing_space)
    }

    fn recover_line_comment_url(&mut self, sink: &mut Sink) {
        let scheme_terminated = matches!(sink.inlines.last(), Some(Inline::Text { text }) if text.ends_with("http:") || text.ends_with("https:"));
        if !scheme_terminated {
            return;
        }
        let rest = self.source.get(self.cursor..).unwrap_or("");
        if !rest.starts_with("//") {
            return;
        }
        let end = rest.find('\n').unwrap_or(rest.len());
        let tail = rest[..end].trim_end_matches('\r');
        if tail.len() <= 2 {
            return;
        }
        push_inline_text(&mut sink.inlines, tail);
        self.cursor += tail.len();
    }

    fn lower_heading(&mut self, heading: &TypstHeading, sink: &mut Sink) -> bool {
        let span = heading.content.span;
        let raw = self.raw(span.start, span.end);
        let line_end = raw.find('\n').unwrap_or(raw.len());
        let line = raw[..line_end].trim_end_matches('\r');
        let marker = line.chars().take_while(|character| *character == '=').count();
        let level = marker.max(1) as u8;
        let title_source = if marker == 0 { line.trim() } else { line[marker..].trim() };
        let title = self.lower_inline_fragment(title_source);
        self.push_block(sink, Block::Section { level, title, children: Vec::new() });
        self.line_breaks = 0;
        self.cursor = span.start + line_end;
        if line_end < raw.len() && !raw[line_end..].trim().is_empty() {
            self.lower_tail(span.start + line_end + 1, span.end, sink);
            return true;
        }
        self.cursor = span.end;
        false
    }

    fn lower_expression(&mut self, quote: &TypstRoot, sink: &mut Sink) -> bool {
        let span = quote.span;
        let raw = self.raw(span.start, span.end);
        let line_end = raw.find('\n').unwrap_or(raw.len());
        let line = raw[..line_end].trim();
        let kind = expression_kind(line);
        self.loss(
            "import.typst.dynamic_content",
            format!("Typst dynamic expression kept opaque: {line}"),
            SemanticStatus::Unsupported,
        );
        self.push_block(sink, Block::Opaque { kind, payload_hint: line.to_string(), status: SemanticStatus::Partial });
        self.line_breaks = 0;
        self.cursor = span.start + line_end;
        if line_end < raw.len() && !raw[line_end..].trim().is_empty() {
            self.lower_tail(span.start + line_end + 1, span.end, sink);
            return true;
        }
        self.cursor = span.end;
        false
    }

    fn lower_list_items(&mut self, lines: Vec<ListLine>, ordered: bool, sink: &mut Sink) {
        let mut items = Vec::new();
        for line in lines {
            let content = self.lower_inline_fragment(&line.content);
            items.push(ListItem { content, children: Vec::new(), number: line.number });
        }
        if items.is_empty() {
            self.loss("import.typst.empty_list", "list markup contains no item line", SemanticStatus::Partial);
            return;
        }
        self.push_block(sink, Block::List { ordered, items });
    }

    fn lower_tail(&mut self, start: usize, end: usize, sink: &mut Sink) {
        let tail = self.raw(start, end);
        if tail.trim().is_empty() {
            return;
        }
        let Ok(root) = parse_typst("<tail>", tail) else {
            self.loss("import.typst.fragment_parse_failed", "reparsed Typst tail could not be parsed", SemanticStatus::Partial);
            return;
        };
        let mut inner =
            TypstLowerer { graph: std::mem::replace(&mut self.graph, DocumentGraph::new(DocumentId(0))), source: tail, cursor: 0, line_breaks: 0 };
        inner.lower_root(&root.items, tail.len(), sink);
        self.graph = inner.graph;
    }

    fn lower_inline_fragment(&mut self, fragment: &str) -> Vec<Inline> {
        if fragment.trim().is_empty() {
            return Vec::new();
        }
        let Ok(root) = parse_typst("<inline>", fragment) else {
            self.loss("import.typst.fragment_parse_failed", "inline Typst fragment could not be parsed", SemanticStatus::Partial);
            return Vec::new();
        };
        let mut inner =
            TypstLowerer { graph: std::mem::replace(&mut self.graph, DocumentGraph::new(DocumentId(0))), source: fragment, cursor: 0, line_breaks: 0 };
        let mut inlines = inner.collect_inlines(&root.items);
        trim_inlines(&mut inlines);
        self.graph = inner.graph;
        inlines
    }

    fn collect_inlines(&mut self, items: &[TypstItem]) -> Vec<Inline> {
        let mut inlines = Vec::new();
        let mut index = 0;
        while index < items.len() {
            match &items[index] {
                TypstItem::Text(text) => {
                    if !is_comment(text) {
                        push_text_inlines(&mut inlines, text);
                    }
                }
                TypstItem::Space | TypstItem::Parbreak => push_text_inlines(&mut inlines, " "),
                TypstItem::Strong(root) => {
                    let (children, trailing_space) = self.styled_children(root, '*');
                    inlines.push(Inline::Styled { style: "bold".into(), children });
                    if trailing_space {
                        push_text_inlines(&mut inlines, " ");
                    }
                }
                TypstItem::Emphasis(root) => {
                    let (children, trailing_space) = self.styled_children(root, '_');
                    inlines.push(Inline::Styled { style: "italic".into(), children });
                    if trailing_space {
                        push_text_inlines(&mut inlines, " ");
                    }
                }
                TypstItem::Math(root) => {
                    let (content, _) = math_content(self.raw(root.span.start, root.span.end));
                    inlines.push(Inline::InlineMath { content, language: Some("typst".into()) });
                }
                TypstItem::Raw(_) => {
                    let mut raw = String::new();
                    while index < items.len() {
                        let TypstItem::Raw(chunk) = &items[index] else {
                            break;
                        };
                        raw.push_str(chunk);
                        index += 1;
                    }
                    index -= 1;
                    let (content, _) = raw_content(&raw);
                    if !content.is_empty() {
                        inlines.push(Inline::InlineCode { text: content });
                    }
                }
                TypstItem::Link(link) => inlines.push(self.link_inline(link)),
                TypstItem::Quote(_) => self.loss(
                    "import.typst.dynamic_content",
                    "dynamic Typst expression cannot be represented as inline content",
                    SemanticStatus::Unsupported,
                ),
                TypstItem::Heading(_) | TypstItem::ListItem(_) | TypstItem::EnumItem(_) | TypstItem::Block(_) => self.loss(
                    "import.typst.block_in_inline_context",
                    "block Typst structure cannot be represented as inline content",
                    SemanticStatus::Partial,
                ),
            }
            index += 1;
        }
        inlines
    }

    fn link_inline(&mut self, link: &TypstLink) -> Inline {
        let display = link.content.as_ref().map(|content| self.collect_inlines(&content.items)).filter(|display| !display.is_empty()).unwrap_or_else(|| {
            vec![Inline::Text { text: link.url.clone() }]
        });
        Inline::Link { target: RelationEndpoint::ExternalUri(link.url.clone()), display, title: None }
    }

    fn list_ahead(&self) -> Option<(Vec<ListLine>, bool, usize)> {
        if !(self.cursor == 0 || self.source.as_bytes().get(self.cursor - 1) == Some(&b'\n')) {
            return None;
        }
        let mut offset = self.cursor;
        let mut lines = Vec::new();
        let mut ordered = None;
        while offset < self.source.len() {
            let end = self.source[offset..].find('\n').map(|index| offset + index).unwrap_or(self.source.len());
            let line = self.source[offset..end].trim_end_matches('\r');
            let Some((line_ordered, number, content)) = list_marker(line) else {
                break;
            };
            if ordered.is_some_and(|ordered| ordered != line_ordered) {
                break;
            }
            ordered = Some(line_ordered);
            lines.push(ListLine { number, content: content.to_string() });
            offset = if end < self.source.len() { end + 1 } else { end };
        }
        let ordered = ordered?;
        Some((lines, ordered, offset))
    }

    fn advance_text(&mut self, text: &str) {
        let rest = self.source.get(self.cursor..).unwrap_or("");
        if rest.starts_with(text) {
            self.cursor += text.len();
            return;
        }
        if let Some(offset) = rest.find(text) {
            self.cursor += offset + text.len();
            return;
        }
        self.loss(
            "import.typst.text_span_mismatch",
            format!("text item could not be located in source order: {text}"),
            SemanticStatus::Partial,
        );
    }

    fn advance_space(&mut self) -> usize {
        let bytes = self.source.as_bytes();
        let mut newlines = 0;
        if bytes.get(self.cursor) == Some(&b'\r') {
            self.cursor += 1;
        }
        if bytes.get(self.cursor) == Some(&b'\n') {
            self.cursor += 1;
            newlines += 1;
        }
        else {
            while matches!(bytes.get(self.cursor), Some(b' ' | b'\t')) {
                self.cursor += 1;
            }
        }
        newlines
    }

    fn push_text(&mut self, text: &str, sink: &mut Sink) {
        for (index, chunk) in split_paragraphs(text).into_iter().enumerate() {
            if index > 0 {
                sink.flush(&mut self.graph);
            }
            push_text_inlines(&mut sink.inlines, chunk);
        }
    }
}

struct ListLine {
    number: Option<u32>,
    content: String,
}

fn typst_span(item: &TypstItem) -> Option<oak_core::Range<usize>> {
    match item {
        TypstItem::Heading(heading) => Some(heading.content.span),
        TypstItem::Strong(root) | TypstItem::Emphasis(root) | TypstItem::Math(root) | TypstItem::Quote(root) => Some(root.span),
        TypstItem::ListItem(root) | TypstItem::EnumItem(root) => Some(root.span),
        TypstItem::Block(root) => Some(root.span),
        _ => None,
    }
}

fn split_list_lines(raw: &str, ordered: bool) -> (Vec<ListLine>, Option<usize>) {
    let mut lines = Vec::new();
    let mut offset = 0usize;
    let mut saw_item = false;
    for segment in raw.split_inclusive('\n') {
        let line = segment.trim_end_matches('\n').trim_end_matches('\r');
        if let Some((line_ordered, number, content)) = list_marker(line) {
            if line_ordered == ordered {
                lines.push(ListLine { number, content: content.to_string() });
                saw_item = true;
                offset += segment.len();
                continue;
            }
        }
        if !saw_item {
            return (lines, None);
        }
        if line.trim().is_empty() {
            offset += segment.len();
            continue;
        }
        return (lines, Some(offset));
    }
    (lines, None)
}

fn list_marker(line: &str) -> Option<(bool, Option<u32>, &str)> {
    if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("+ ")).or_else(|| line.strip_prefix("-\t")) {
        return Some((false, None, rest.trim_end_matches('\r')));
    }
    let digits: String = line.chars().take_while(|character| character.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let rest = &line[digits.len()..];
    let rest = rest.strip_prefix('.')?;
    let rest = rest.strip_prefix(' ').or_else(|| rest.strip_prefix('\t'))?;
    Some((true, digits.parse().ok(), rest.trim_end_matches('\r')))
}

fn expression_kind(line: &str) -> String {
    let rest = line.strip_prefix('#').unwrap_or(line);
    let keyword: String = rest.chars().take_while(|character| character.is_ascii_alphabetic()).collect();
    if keyword.is_empty() { "typst.expression".into() } else { format!("typst.expression.{keyword}") }
}

fn is_comment(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("//") || trimmed.starts_with("/*")
}

fn math_content(raw: &str) -> (String, bool) {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('$') && trimmed.ends_with('$') {
        let inner = &trimmed[1..trimmed.len() - 1];
        let display = inner.starts_with(char::is_whitespace) || inner.ends_with(char::is_whitespace);
        return (inner.trim().to_string(), display);
    }
    (trimmed.to_string(), false)
}

fn raw_content(raw: &str) -> (String, Option<String>) {
    let trimmed = raw.trim();
    let ticks = trimmed.chars().take_while(|character| *character == '`').count();
    if ticks == 0 {
        return (trimmed.to_string(), None);
    }
    if trimmed.len() < ticks * 2 {
        return (String::new(), None);
    }
    let inner = &trimmed[ticks..trimmed.len() - ticks];
    if ticks >= 3 {
        if let Some(newline) = inner.find('\n') {
            let first = inner[..newline].trim();
            let is_language = !first.is_empty() && first.chars().all(|character| character.is_alphanumeric() || matches!(character, '+' | '-' | '_'));
            if is_language {
                return (inner[newline + 1..].trim_matches('\n').to_string(), Some(first.to_string()));
            }
        }
        return (inner.trim_matches('\n').to_string(), None);
    }
    (inner.to_string(), None)
}

fn split_paragraphs(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'\n' {
            let mut next = index + 1;
            while next < bytes.len() && matches!(bytes[next], b' ' | b'\t' | b'\r') {
                next += 1;
            }
            if next < bytes.len() && bytes[next] == b'\n' {
                chunks.push(&text[start..index]);
                index = next + 1;
                start = index;
                continue;
            }
        }
        index += 1;
    }
    chunks.push(&text[start..]);
    chunks
}

fn push_text_inlines(inlines: &mut Vec<Inline>, text: &str) {
    let normalized = normalize_text(text);
    if normalized.is_empty() {
        return;
    }
    if inlines.is_empty() {
        let trimmed = normalized.trim_start();
        if !trimmed.is_empty() {
            push_inline_text(inlines, trimmed);
        }
        return;
    }
    if last_inline_ends_with_space(inlines) && normalized.starts_with(' ') {
        push_inline_text(inlines, &normalized[1..]);
        return;
    }
    push_inline_text(inlines, &normalized);
}

fn normalize_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut pending_space = false;
    for character in text.chars() {
        if character.is_whitespace() {
            pending_space = !normalized.is_empty();
            continue;
        }
        if pending_space {
            normalized.push(' ');
            pending_space = false;
        }
        normalized.push(character);
    }
    if pending_space {
        normalized.push(' ');
    }
    if normalized.is_empty() && !text.is_empty() {
        return " ".into();
    }
    normalized
}

fn push_inline_text(inlines: &mut Vec<Inline>, text: &str) {
    if let Some(Inline::Text { text: previous }) = inlines.last_mut() {
        previous.push_str(text);
        return;
    }
    inlines.push(Inline::Text { text: text.into() });
}

fn last_inline_ends_with_space(inlines: &[Inline]) -> bool {
    matches!(inlines.last(), Some(Inline::Text { text }) if text.ends_with(' '))
}

fn trim_inlines(inlines: &mut Vec<Inline>) {
    if let Some(Inline::Text { text }) = inlines.last_mut() {
        let trimmed = text.trim_end().len();
        text.truncate(trimmed);
    }
    if matches!(inlines.last(), Some(Inline::Text { text }) if text.is_empty()) {
        inlines.pop();
    }
    if let Some(Inline::Text { text }) = inlines.first_mut() {
        if text.starts_with(' ') {
            *text = text.trim_start().to_string();
        }
    }
    if matches!(inlines.first(), Some(Inline::Text { text }) if text.is_empty()) {
        inlines.remove(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oak_typst::ast::TypstLink;

    #[test]
    fn link_items_become_link_inlines() {
        let display = TypstRoot { span: (0..0).into(), items: vec![TypstItem::Text("example".into())] };
        let link = TypstLink { url: "https://example.com".into(), content: Some(display) };
        let root = TypstRoot { span: (0..0).into(), items: vec![TypstItem::Link(link)] };
        let graph = lower_typst_root(&root, "", DocumentId(1));
        let inline = graph
            .blocks
            .iter()
            .find_map(|node| match &node.block {
                Block::Paragraph { content } => content.iter().find(|inline| matches!(inline, Inline::Link { .. })),
                _ => None,
            })
            .expect("link inline");
        let Inline::Link { target, display, title } = inline else {
            unreachable!()
        };
        assert_eq!(target, &RelationEndpoint::ExternalUri("https://example.com".into()));
        assert_eq!(display.iter().map(Inline::plain_text).collect::<String>(), "example");
        assert!(title.is_none());
    }
}

