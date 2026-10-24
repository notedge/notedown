use std::{collections::HashMap, path::Path};

use notedown_ir::{
    Block, BreakKind, DocumentGraph, DocumentId, Inline, ListItem, LossMarker, NodeId, RelationEndpoint, SemanticStatus, TableCell, TableRow,
};
use oak_core::{GreenNode, Parser, RedNode, RedTree, SourceText, parser::session::ParseSession};
use oak_markdown::{MarkdownLanguage, MarkdownParser, parser::element_type::MarkdownElementType};

use super::common::{decode_escaped_text, register_image_assets};
use crate::FormatError;

/// Import Markdown from disk into `notedown-ir`.
pub fn import_markdown(path: impl AsRef<Path>) -> Result<DocumentGraph, FormatError> {
    let path = path.as_ref();
    let text =
        std::fs::read_to_string(path).map_err(|error| FormatError::invalid_input(format!("failed to read {}: {error}", path.display())))?;
    import_markdown_bytes(&path.display().to_string(), &text)
}

/// Oak profile for semantic Markdown import: enable the constructs required by
/// `notedown-formats` round-trip and markdown-project reopen probes.
///
/// Math and footnotes are part of the import contract, so the profile has to
/// enable them explicitly instead of leaving every extension at its default.
fn markdown_import_language() -> MarkdownLanguage {
    let mut language = MarkdownLanguage::default();
    language.allow_headings = true;
    language.allow_math = true;
    language.allow_footnotes = true;
    language.allow_strikethrough = true;
    language.allow_lists = true;
    language.allow_blockquotes = true;
    language.allow_fenced_code_blocks = true;
    language.allow_tables = true;
    language.allow_horizontal_rules = true;
    language
}

/// Import Markdown text into `notedown-ir` via `oak-markdown`.
pub fn import_markdown_bytes(label: &str, text: &str) -> Result<DocumentGraph, FormatError> {
    let language = markdown_import_language();
    let source = SourceText::new(text);
    let mut cache = ParseSession::<MarkdownLanguage>::default();
    let parser = MarkdownParser::new(&language);
    let parsed = parser.parse(&source, &[], &mut cache);
    let green_tree = parsed.result.map_err(|error| FormatError::parse("markdown", error.to_string()))?;
    lower_markdown_tree(label, green_tree, &source)
}

fn lower_markdown_tree(label: &str, green_tree: &GreenNode<MarkdownLanguage>, source: &SourceText) -> Result<DocumentGraph, FormatError> {
    let red_root = RedNode::new(green_tree, 0);
    let mut graph = DocumentGraph::new(document_id_for(label));
    let mut context = LinkContext::default();
    let children = red_root.children().collect::<Vec<_>>();
    for child in &children {
        if let RedTree::Node(node) = *child {
            register_link_definition_node(node, source, &mut context);
        }
    }
    let mut index = 0;
    while let Some(child) = children.get(index).copied() {
        index += 1;
        let RedTree::Node(node) = child
        else {
            continue;
        };
        if let Some((offset, heading)) = setext_heading(node, children.get(index..).unwrap_or_default(), source, &mut context) {
            graph.push_block(heading);
            index += offset + 1;
            continue;
        }
        match lower_block_node(node, source, &mut context, &mut graph) {
            BlockOutcome::Block(block) => {
                graph.push_block(block);
            }
            BlockOutcome::Loss(marker) => {
                graph.push_loss(marker);
            }
            BlockOutcome::Skip => {}
        }
    }

    for loss in context.losses {
        graph.push_loss(loss);
    }

    register_image_assets(&mut graph);

    if graph.blocks.is_empty() && graph.coverage.loss.is_empty() {
        graph.push_loss(LossMarker {
            code: "import.markdown.empty_document".into(),
            message: "markdown source produced no semantic blocks".into(),
            status: SemanticStatus::Partial,
        });
    }
    Ok(graph)
}

enum BlockOutcome {
    Block(Block),
    Loss(LossMarker),
    Skip,
}

#[derive(Default)]
struct LinkContext {
    definitions: HashMap<String, String>,
    losses: Vec<LossMarker>,
}

/// Promote a CommonMark setext heading before the underline is lowered as a
/// thematic break.
///
/// `oak-markdown` does not parse setext headings: `Title\n---` becomes a
/// paragraph followed by a horizontal-rule node, exactly like a thematic break.
/// The two differ only by adjacency, so the underline must sit on the very next
/// line of the paragraph; a blank line in between keeps the thematic break.
fn setext_heading(
    node: RedNode<MarkdownLanguage>,
    trailing: &[RedTree<MarkdownLanguage>],
    source: &SourceText,
    context: &mut LinkContext,
) -> Option<(usize, Block)> {
    if node.element_type() != MarkdownElementType::Paragraph || parse_link_definition(node.text(source).trim()).is_some() {
        return None;
    }
    let mut underline = None;
    for (offset, child) in trailing.iter().enumerate() {
        match child {
            RedTree::Node(candidate) => {
                underline = Some((offset, *candidate));
                break;
            }
            RedTree::Leaf(leaf) if leaf.text(source).trim().is_empty() => {}
            RedTree::Leaf(_) => return None,
        }
    }
    let (offset, underline) = underline?;
    if underline.element_type() != MarkdownElementType::HorizontalRule {
        return None;
    }
    let marker = underline.text(source);
    let marker = marker.trim();
    if marker.is_empty() || !marker.bytes().all(|byte| byte == b'-') {
        return None;
    }
    let node_span = node.span();
    let gap = source.slice((node_span.end..underline.span().start).into());
    if !gap.is_empty() && gap != "\n" && gap != "\r\n" {
        return None;
    }
    let title = collect_inlines(node, source, context);
    if title.is_empty() {
        return None;
    }
    Some((offset, Block::Section { level: 2, title, children: Vec::new() }))
}

fn register_link_definition_node(node: RedNode<MarkdownLanguage>, source: &SourceText, context: &mut LinkContext) {
    if node.element_type() != MarkdownElementType::Paragraph {
        return;
    }
    let text = node.text(source);
    let raw = text.trim();
    if let Some((label, destination, has_title)) = parse_link_definition(raw) {
        context.definitions.entry(label).or_insert(destination);
        if has_title {
            context.losses.push(LossMarker {
                code: "import.markdown.reference_definition_title".into(),
                message: "reference-link definition title is not represented in the current link IR".into(),
                status: SemanticStatus::Lossy,
            });
        }
        return;
    }
    if raw.starts_with('[') && raw.contains("]:") {
        context.losses.push(LossMarker {
            code: "import.markdown.invalid_link_definition".into(),
            message: format!("unsupported or invalid link definition: {}", raw),
            status: SemanticStatus::Partial,
        });
    }
}

fn parse_link_definition(raw: &str) -> Option<(String, String, bool)> {
    let close = reference_label_close(raw.strip_prefix('[')?)? + 1;
    if !raw[close..].starts_with("]:") {
        return None;
    }
    let label = raw.get(1..close)?.trim();
    let remainder = raw.get(close + 2..)?.trim();
    let destination = remainder.split_whitespace().next()?;
    if label.is_empty() || destination.is_empty() {
        return None;
    }
    let has_title = remainder.strip_prefix(destination).is_some_and(|tail| !tail.trim().is_empty());
    let destination = destination.strip_prefix('<').and_then(|value| value.strip_suffix('>')).unwrap_or(destination);
    Some((normalize_link_label(label), decode_escaped_text(destination), has_title))
}

fn normalize_link_label(label: &str) -> String {
    decode_escaped_text(label).split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn reference_label_close(content: &str) -> Option<usize> {
    let mut escaped = false;
    for (index, character) in content.char_indices() {
        if escaped {
            escaped = false;
        }
        else if character == '\\' {
            escaped = true;
        }
        else if character == ']' {
            return Some(index);
        }
    }
    None
}

fn split_reference_link(raw: &str) -> Option<(String, String, bool)> {
    let content = raw.strip_prefix('[').or_else(|| raw.strip_prefix("!["))?;
    let close = reference_label_close(content)?;
    let text = decode_escaped_text(&content[..close]);
    let suffix = &content[close + 1..];
    if let Some(reference) = suffix.strip_prefix('[').and_then(|value| value.strip_suffix(']')) {
        let label = if reference.is_empty() { text.clone() } else { decode_escaped_text(reference) };
        return Some((text, label, true));
    }
    if suffix.is_empty() {
        return Some((text.clone(), text, false));
    }
    None
}

fn lower_block_node(
    node: RedNode<MarkdownLanguage>,
    source: &SourceText,
    context: &mut LinkContext,
    graph: &mut DocumentGraph,
) -> BlockOutcome {
    let kind = node.element_type();
    match kind {
        MarkdownElementType::Heading1
        | MarkdownElementType::Heading2
        | MarkdownElementType::Heading3
        | MarkdownElementType::Heading4
        | MarkdownElementType::Heading5
        | MarkdownElementType::Heading6 => BlockOutcome::Block(Block::Section {
            level: heading_level(kind),
            title: collect_heading_inlines(node, source, context),
            children: Vec::new(),
        }),
        MarkdownElementType::Paragraph => {
            let text = node.text(source);
            if parse_link_definition(text.trim()).is_some() {
                BlockOutcome::Skip
            }
            else {
                let content = collect_inlines(node, source, context);
                if content.is_empty() {
                    // Markdown blank lines and container-trailing newlines are not
                    // paragraphs: an empty inline result must never reach the graph.
                    return BlockOutcome::Skip;
                }
                BlockOutcome::Block(Block::Paragraph { content })
            }
        }
        MarkdownElementType::CodeBlock => {
            let (language, content) = extract_code_block(node, source);
            BlockOutcome::Block(Block::Code { language, content })
        }
        MarkdownElementType::MathBlock => {
            BlockOutcome::Block(Block::Math { content: strip_math_delimiters(&node.text(source), "$$"), language: Some("latex".into()) })
        }
        MarkdownElementType::FootnoteDefinition => BlockOutcome::Block(Block::Opaque {
            kind: "footnote_definition".into(),
            payload_hint: node.text(source).into_owned(),
            status: SemanticStatus::Resolved,
        }),
        MarkdownElementType::List => {
            let (ordered, items) = extract_list(node, source, context, graph);
            BlockOutcome::Block(Block::List { ordered, items })
        }
        MarkdownElementType::Blockquote => BlockOutcome::Block(Block::Quote { content: lower_blockquote_inlines(node, source, context) }),
        MarkdownElementType::Table => BlockOutcome::Block(Block::Table { caption: None, rows: extract_table(node, source, context) }),
        MarkdownElementType::HorizontalRule => {
            BlockOutcome::Block(Block::Opaque { kind: "thematic_break".into(), payload_hint: "---".into(), status: SemanticStatus::Partial })
        }
        _ => BlockOutcome::Loss(LossMarker {
            code: "import.markdown.unsupported_block".into(),
            message: format!("unsupported markdown block: {kind:?}"),
            status: SemanticStatus::Unsupported,
        }),
    }
}

fn collect_heading_inlines(node: RedNode<MarkdownLanguage>, source: &SourceText, context: &mut LinkContext) -> Vec<Inline> {
    let mut inlines = collect_inlines(node, source, context);
    let raw = node.text(source);
    let marker_length = raw.chars().take_while(|character| *character == '#').count();
    if !(1..=6).contains(&marker_length) {
        return inlines;
    }

    let Some(Inline::Text { text }) = inlines.first_mut()
    else {
        return inlines;
    };
    let Some(rest) = text.strip_prefix(&"#".repeat(marker_length))
    else {
        return inlines;
    };
    *text = rest.trim_start_matches([' ', '\t']).to_string();
    if text.is_empty() {
        inlines.remove(0);
    }
    inlines
}

fn collect_inlines(node: RedNode<MarkdownLanguage>, source: &SourceText, context: &mut LinkContext) -> Vec<Inline> {
    let mut inlines = Vec::new();
    for child in node.children() {
        match child {
            RedTree::Node(child_node) => {
                let link_like = matches!(child_node.element_type(), MarkdownElementType::Link | MarkdownElementType::Image);
                let trailing = if link_like { trailing_link_whitespace(&child_node.text(source)) } else { String::new() };
                if let Some(inline) = lower_inline_node(child_node, source, context) {
                    push_inline(&mut inlines, inline);
                }
                if !trailing.is_empty() {
                    push_inline(&mut inlines, Inline::Text { text: trailing });
                }
            }
            RedTree::Leaf(_) => {
                let text = child.text(source);
                if !text.is_empty() && !is_markdown_marker(text.as_ref()) {
                    push_inline(&mut inlines, Inline::Text { text: decode_escaped_text(text.as_ref()) });
                }
            }
        }
    }
    if inlines.is_empty() {
        let text = node.text(source);
        if !text.trim().is_empty() {
            // A whole node that carries no element children still has to decode escapes,
            // otherwise `\[` stays a literal backslash on reopen.
            inlines.push(Inline::Text { text: decode_escaped_text(text.as_ref()) });
        }
    }
    inlines
}

fn lower_inline_node(node: RedNode<MarkdownLanguage>, source: &SourceText, context: &mut LinkContext) -> Option<Inline> {
    match node.element_type() {
        MarkdownElementType::Text | MarkdownElementType::HeadingText => Some(Inline::Text { text: decode_escaped_text(&node.text(source)) }),
        MarkdownElementType::Strong => Some(Inline::Styled { style: "bold".into(), children: collect_inlines(node, source, context) }),
        MarkdownElementType::Emphasis => Some(Inline::Styled { style: "italic".into(), children: collect_inlines(node, source, context) }),
        MarkdownElementType::InlineCode => Some(Inline::InlineCode { text: inline_code_text(&node.text(source)) }),
        MarkdownElementType::Link => parse_inline_link(node, source, context),
        MarkdownElementType::Image => parse_inline_image(node, source, context),
        MarkdownElementType::MathInline => {
            Some(Inline::InlineMath { content: strip_math_delimiters(&node.text(source), "$"), language: Some("latex".into()) })
        }
        MarkdownElementType::FootnoteReference => {
            let raw = node.text(source);
            let label = raw.trim().strip_prefix("[^").and_then(|value| value.strip_suffix(']')).unwrap_or_default().to_string();
            Some(Inline::Note { label, display: None })
        }
        MarkdownElementType::Strikethrough => Some(Inline::Styled { style: "strike".into(), children: collect_inlines(node, source, context) }),
        _ => None,
    }
}

fn push_inline(inlines: &mut Vec<Inline>, inline: Inline) {
    if let Inline::Text { text: right } = inline {
        if let Some(Inline::Text { text: left }) = inlines.last_mut() {
            left.push_str(&right);
            return;
        }
        inlines.push(Inline::Text { text: right });
        return;
    }
    inlines.push(inline);
}

fn push_soft_break(inlines: &mut Vec<Inline>) {
    if inlines.is_empty() || matches!(inlines.last(), Some(Inline::Break { .. })) {
        return;
    }
    inlines.push(Inline::Break { kind: BreakKind::Soft });
}

/// Strip CommonMark code-span delimiters: a matching backtick run on both
/// sides plus the optional single space padding inside the span.
fn inline_code_text(raw: &str) -> String {
    let trimmed = raw.trim_end();
    let opening = trimmed.chars().take_while(|character| *character == '`').count();
    let closing = trimmed.chars().rev().take_while(|character| *character == '`').count();
    let delimiter = opening.min(closing);
    if delimiter == 0 {
        return trimmed.to_string();
    }
    let Some(inner) = trimmed.get(delimiter..trimmed.len().saturating_sub(delimiter))
    else {
        return trimmed.to_string();
    };
    if inner.len() >= 2 && inner.starts_with(' ') && inner.ends_with(' ') && !inner.trim().is_empty() {
        return inner[1..inner.len() - 1].to_string();
    }
    inner.to_string()
}

fn parse_inline_link(node: RedNode<MarkdownLanguage>, source: &SourceText, context: &mut LinkContext) -> Option<Inline> {
    let raw = node.text(source);
    if let Some((text, url)) = resolve_link_destination(raw.as_ref(), context) {
        return Some(Inline::Link { target: RelationEndpoint::ExternalUri(url), display: vec![Inline::Text { text }], title: None });
    }
    Some(Inline::Text { text: decode_escaped_text(raw.trim_end()) })
}

fn parse_inline_image(node: RedNode<MarkdownLanguage>, source: &SourceText, context: &mut LinkContext) -> Option<Inline> {
    let raw = node.text(source);
    let Some((alt, url)) = resolve_link_destination(raw.as_ref(), context)
    else {
        return Some(Inline::Text { text: decode_escaped_text(raw.trim_end()) });
    };
    Some(Inline::Image { source: url, alt: vec![Inline::Text { text: alt }], title: None, asset: None })
}

/// Resolve an inline or reference destination, recording explicit unresolved
/// labels.
fn resolve_link_destination(raw: &str, context: &mut LinkContext) -> Option<(String, String)> {
    if let Some((text, url)) = split_markdown_link(raw) {
        return Some((text, url));
    }
    if let Some((text, label, explicit)) = split_reference_link(raw.trim_end()) {
        if let Some(destination) = context.definitions.get(&normalize_link_label(&label)) {
            return Some((text, destination.clone()));
        }
        if explicit {
            context.losses.push(LossMarker {
                code: "import.markdown.unresolved_reference_link".into(),
                message: format!("unresolved reference link label: {label}"),
                status: SemanticStatus::Unresolved,
            });
        }
    }
    None
}

fn split_markdown_link(raw: &str) -> Option<(String, String)> {
    let start = raw.find("](")?;
    let label = raw[..start].trim();
    let text = decode_escaped_text(label.strip_prefix("![").or_else(|| label.strip_prefix('['))?);
    let destination = &raw[start + 2..];
    let close = link_destination_close(destination)?;
    let url = decode_escaped_text(destination[..close].trim());
    if text.is_empty() || url.is_empty() {
        return None;
    }
    Some((text, url))
}

fn strip_math_delimiters(text: &str, delimiter: &str) -> String {
    text.trim().strip_prefix(delimiter).and_then(|value| value.strip_suffix(delimiter)).unwrap_or(text.trim()).to_string()
}

fn trailing_link_whitespace(raw: &str) -> String {
    let Some(start) = raw.find("](")
    else {
        return raw[raw.trim_end_matches([' ', '\t']).len()..].to_string();
    };
    let destination = &raw[start + 2..];
    let Some(close) = link_destination_close(destination)
    else {
        return String::new();
    };
    destination[close + 1..].chars().take_while(|character| character.is_whitespace()).collect()
}

fn link_destination_close(destination: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut escaped = false;
    for (index, character) in destination.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '(' => depth += 1,
            ')' if depth == 0 => {
                return Some(index);
            }
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

fn is_markdown_marker(text: &str) -> bool {
    matches!(text, "*" | "**" | "_" | "__" | "~~" | "`" | "[" | "]" | "(" | ")" | "![")
}

/// Fenced code keeps the raw source slice between the two fences.
///
/// `oak-markdown` re-lexes fence bodies with the inline lexer, which turns
/// punctuation into dedicated tokens or nodes (`A[Start]` becomes `Text "A"`,
/// `Link "["` and `Text "Start"`). Rebuilding the body token by token dropped
/// every element the lowering did not know about, so the content is taken from
/// the source byte range instead.
fn extract_code_block(node: RedNode<MarkdownLanguage>, source: &SourceText) -> (Option<String>, String) {
    let node_span = node.span();
    let mut language = None;
    let mut body_start = None;
    let mut body_end = node_span.end;
    for child in node.children() {
        match child.kind::<MarkdownElementType>() {
            MarkdownElementType::CodeFence if body_start.is_none() => body_start = Some(child.span().end),
            MarkdownElementType::CodeFence => {
                body_end = child.span().start;
                break;
            }
            MarkdownElementType::CodeLanguage if body_start.is_some() && language.is_none() => {
                let text = child.text(source);
                let tag = text.trim();
                if !tag.is_empty() {
                    language = Some(tag.to_string());
                }
                body_start = Some(child.span().end);
            }
            _ => {}
        }
    }
    let start = body_start.unwrap_or(node_span.start).min(body_end);
    let raw = source.slice((start..body_end).into());
    (language, strip_fence_line_remainder(&raw).to_string())
}

/// A fence body starts on the line after the opening fence and its info
/// string, and ends before the line break that precedes the closing fence.
fn strip_fence_line_remainder(text: &str) -> &str {
    let Some(index) = text.find(['\r', '\n'])
    else {
        return "";
    };
    let text = strip_leading_line_break(&text[index..]);
    strip_trailing_line_break(text)
}

fn strip_leading_line_break(text: &str) -> &str {
    text.strip_prefix("\r\n").or_else(|| text.strip_prefix('\n')).or_else(|| text.strip_prefix('\r')).unwrap_or(text)
}

fn strip_trailing_line_break(text: &str) -> &str {
    text.strip_suffix("\r\n").or_else(|| text.strip_suffix('\n')).or_else(|| text.strip_suffix('\r')).unwrap_or(text)
}

fn list_item_marker(node: RedNode<MarkdownLanguage>, source: &SourceText) -> Option<String> {
    node.children()
        .find(|child| child.kind::<MarkdownElementType>() == MarkdownElementType::ListMarker)
        .map(|child| child.text(source).trim().to_string())
}

fn extract_list(
    node: RedNode<MarkdownLanguage>,
    source: &SourceText,
    context: &mut LinkContext,
    graph: &mut DocumentGraph,
) -> (bool, Vec<ListItem>) {
    let mut ordered = false;
    let mut items = Vec::new();
    for child in node.children() {
        let RedTree::Node(item_node) = child
        else {
            continue;
        };
        if item_node.element_type() != MarkdownElementType::ListItem {
            continue;
        }
        let marker = list_item_marker(item_node, source);
        if items.is_empty() {
            ordered = marker.as_deref().and_then(|value| value.chars().next()).is_some_and(|character| character.is_ascii_digit());
        }
        let number = if ordered {
            marker
                .as_deref()
                .map(|value| value.chars().take_while(char::is_ascii_digit).collect::<String>())
                .and_then(|digits| digits.parse::<u32>().ok())
        }
        else {
            None
        };
        let (content, children) = lower_list_item(item_node, source, context, graph);
        items.push(ListItem { content, children, number });
    }
    (ordered, items)
}

fn lower_list_item(
    node: RedNode<MarkdownLanguage>,
    source: &SourceText,
    context: &mut LinkContext,
    graph: &mut DocumentGraph,
) -> (Vec<Inline>, Vec<NodeId>) {
    let mut content = Vec::new();
    let mut children = Vec::new();
    let mut skip_marker_space = false;
    let mut line_start = false;
    for child in node.children() {
        match child {
            RedTree::Leaf(_) => match child.kind::<MarkdownElementType>() {
                MarkdownElementType::ListMarker => skip_marker_space = true,
                MarkdownElementType::Newline => {
                    skip_marker_space = false;
                    line_start = true;
                }
                _ => {
                    let text = child.text(source);
                    if (skip_marker_space || line_start) && text.trim().is_empty() {
                        skip_marker_space = false;
                        line_start = false;
                        continue;
                    }
                    skip_marker_space = false;
                    line_start = false;
                    if !text.is_empty() {
                        push_inline(&mut content, Inline::Text { text: decode_escaped_text(text.as_ref()) });
                    }
                }
            },
            RedTree::Node(child_node) => {
                skip_marker_space = false;
                line_start = false;
                if child_node.element_type() == MarkdownElementType::List {
                    let (nested_ordered, nested_items) = extract_list(child_node, source, context, graph);
                    children.push(graph.push_block(Block::List { ordered: nested_ordered, items: nested_items }));
                }
                else if let Some(inline) = lower_inline_node(child_node, source, context) {
                    push_inline(&mut content, inline);
                }
            }
        }
    }
    (content, children)
}

fn lower_blockquote_inlines(node: RedNode<MarkdownLanguage>, source: &SourceText, context: &mut LinkContext) -> Vec<Inline> {
    let children = node.children().collect::<Vec<_>>();
    let mut inlines = Vec::new();
    let mut skip_marker_space = false;
    let mut line_start = false;
    for (index, child) in children.iter().enumerate() {
        match *child {
            RedTree::Leaf(_) => match child.kind::<MarkdownElementType>() {
                MarkdownElementType::BlockquoteMarker => skip_marker_space = true,
                MarkdownElementType::Newline => {
                    skip_marker_space = false;
                    line_start = true;
                    if index + 1 < children.len() {
                        push_soft_break(&mut inlines);
                    }
                }
                _ => {
                    let text = child.text(source);
                    if (skip_marker_space || line_start) && text.trim().is_empty() {
                        skip_marker_space = false;
                        line_start = false;
                        continue;
                    }
                    skip_marker_space = false;
                    line_start = false;
                    if !text.is_empty() {
                        push_inline(&mut inlines, Inline::Text { text: decode_escaped_text(text.as_ref()) });
                    }
                }
            },
            RedTree::Node(child_node) => {
                skip_marker_space = false;
                line_start = false;
                match child_node.element_type() {
                    MarkdownElementType::Blockquote | MarkdownElementType::Paragraph => {
                        push_soft_break(&mut inlines);
                        let nested = if child_node.element_type() == MarkdownElementType::Blockquote {
                            lower_blockquote_inlines(child_node, source, context)
                        }
                        else {
                            collect_inlines(child_node, source, context)
                        };
                        for inline in nested {
                            push_inline(&mut inlines, inline);
                        }
                    }
                    _ => {
                        if let Some(inline) = lower_inline_node(child_node, source, context) {
                            push_inline(&mut inlines, inline);
                        }
                    }
                }
            }
        }
    }
    inlines
}

fn extract_table(node: RedNode<MarkdownLanguage>, source: &SourceText, context: &mut LinkContext) -> Vec<TableRow> {
    let mut rows = Vec::new();
    let mut header_pending = true;
    for child in node.children() {
        let RedTree::Node(row_node) = child
        else {
            continue;
        };
        if row_node.element_type() != MarkdownElementType::TableRow {
            continue;
        }
        let mut cells = Vec::new();
        for cell_child in row_node.children() {
            let RedTree::Node(cell_node) = cell_child
            else {
                continue;
            };
            if cell_node.element_type() != MarkdownElementType::TableCell {
                continue;
            }
            let content = trim_inline_edges(collect_inlines(cell_node, source, context));
            cells.push(TableCell { content, colspan: 1, rowspan: 1 });
        }
        if cells.last().is_some_and(|cell| inline_text(&cell.content).is_empty()) {
            cells.pop();
        }
        if cells.is_empty() || is_table_separator_row(&cells) {
            continue;
        }
        let is_header = header_pending;
        header_pending = false;
        rows.push(TableRow { is_header, cells });
    }
    rows
}

fn is_table_separator_row(cells: &[TableCell]) -> bool {
    let text = cells.iter().flat_map(|cell| cell.content.iter().map(Inline::plain_text)).collect::<String>();
    let visible = text.trim();
    !visible.is_empty() && visible.contains('-') && visible.chars().all(|character| matches!(character, '-' | ':'))
}

fn trim_inline_edges(mut inlines: Vec<Inline>) -> Vec<Inline> {
    if let Some(Inline::Text { text }) = inlines.first_mut() {
        *text = text.trim_start().to_string();
    }
    if let Some(Inline::Text { text }) = inlines.last_mut() {
        *text = text.trim_end().to_string();
    }
    inlines.retain(|inline| !matches!(inline, Inline::Text { text } if text.is_empty()));
    inlines
}

fn inline_text(inlines: &[Inline]) -> String {
    inlines.iter().map(Inline::plain_text).collect()
}

fn heading_level(kind: MarkdownElementType) -> u8 {
    match kind {
        MarkdownElementType::Heading1 => 1,
        MarkdownElementType::Heading2 => 2,
        MarkdownElementType::Heading3 => 3,
        MarkdownElementType::Heading4 => 4,
        MarkdownElementType::Heading5 => 5,
        MarkdownElementType::Heading6 => 6,
        _ => 1,
    }
}

fn document_id_for(label: &str) -> DocumentId {
    let mut hash = 1u64;
    for byte in label.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u64::from(byte));
    }
    DocumentId(hash)
}
