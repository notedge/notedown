use std::{collections::HashMap, path::Path};

use notedown_ir::{Block, DocumentGraph, DocumentId, Inline, ListItem, LossMarker, SemanticStatus};
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

/// Import Markdown text into `notedown-ir` via `oak-markdown`.
pub fn import_markdown_bytes(label: &str, text: &str) -> Result<DocumentGraph, FormatError> {
    let language = MarkdownLanguage::default();
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
    for child in red_root.children() {
        if let RedTree::Node(node) = child {
            if node.element_type() == MarkdownElementType::LinkDefinition {
                if let Some((label, destination, has_title)) = parse_link_definition(&node.text(source)) {
                    context.definitions.entry(label).or_insert(destination);
                    if has_title {
                        context.losses.push(LossMarker {
                            code: "import.markdown.reference_definition_title".into(),
                            message: "reference-link definition title is not represented in the current link IR".into(),
                            status: SemanticStatus::Lossy,
                        });
                    }
                } else {
                    context.losses.push(LossMarker {
                        code: "import.markdown.invalid_link_definition".into(),
                        message: format!("unsupported or invalid link definition: {}", node.text(source)),
                        status: SemanticStatus::Partial,
                    });
                }
            }
        }
    }
    for child in red_root.children() {
        if let RedTree::Node(node) = child {
            match lower_block_node(node, source, &mut context) {
                BlockOutcome::Block(block) => {
                    graph.push_block(block);
                }
                BlockOutcome::Loss(marker) => {
                    graph.push_loss(marker);
                }
                BlockOutcome::Skip => {}
            }
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
        } else if character == '\\' {
            escaped = true;
        } else if character == ']' {
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

fn lower_block_node(node: RedNode<MarkdownLanguage>, source: &SourceText, context: &mut LinkContext) -> BlockOutcome {
    let kind = node.element_type();
    match kind {
        MarkdownElementType::Heading1
        | MarkdownElementType::Heading2
        | MarkdownElementType::Heading3
        | MarkdownElementType::Heading4
        | MarkdownElementType::Heading5
        | MarkdownElementType::Heading6 => {
            BlockOutcome::Block(Block::Section { level: heading_level(kind), title: collect_heading_inlines(node, source, context), children: Vec::new() })
        }
        MarkdownElementType::Paragraph => BlockOutcome::Block(Block::Paragraph { content: collect_inlines(node, source, context) }),
        MarkdownElementType::LinkDefinition => BlockOutcome::Skip,
        MarkdownElementType::CodeBlock => {
            let (language, content) = extract_code_block(node, source);
            BlockOutcome::Block(Block::Code { language, content })
        }
        MarkdownElementType::MathBlock => BlockOutcome::Block(Block::Math {
            content: strip_math_delimiters(&node.text(source), "$$"),
            language: Some("latex".into()),
        }),
        MarkdownElementType::FootnoteDefinition => BlockOutcome::Block(Block::Opaque {
            kind: "footnote_definition".into(),
            payload_hint: node.text(source).into_owned(),
            status: SemanticStatus::Resolved,
        }),
        MarkdownElementType::List => {
            let (ordered, items) = extract_list(node, source, context);
            BlockOutcome::Block(Block::List { ordered, items })
        }
        MarkdownElementType::Blockquote => {
            BlockOutcome::Block(Block::Quote { content: vec![Inline::Text { text: extract_blockquote_text(node, source) }] })
        }
        MarkdownElementType::HorizontalRule => BlockOutcome::Loss(LossMarker {
            code: "import.markdown.horizontal_rule".into(),
            message: "horizontal rule lowered to loss marker".into(),
            status: SemanticStatus::Lossy,
        }),
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

    let Some(Inline::Text { text }) = inlines.first_mut() else {
        return inlines;
    };
    let Some(rest) = text.strip_prefix(&"#".repeat(marker_length)) else {
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
                let link_like = matches!(
                    child_node.element_type(),
                    MarkdownElementType::Link | MarkdownElementType::Image
                );
                let trailing = if link_like {
                    trailing_link_whitespace(&child_node.text(source))
                } else {
                    String::new()
                };
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
            inlines.push(Inline::Text { text: text.into_owned() });
        }
    }
    inlines
}

fn lower_inline_node(node: RedNode<MarkdownLanguage>, source: &SourceText, context: &mut LinkContext) -> Option<Inline> {
    match node.element_type() {
        MarkdownElementType::Text | MarkdownElementType::HeadingText => Some(Inline::Text { text: decode_escaped_text(&node.text(source)) }),
        MarkdownElementType::Strong => Some(Inline::Styled { style: "bold".into(), children: collect_inlines(node, source, context) }),
        MarkdownElementType::Emphasis => Some(Inline::Styled { style: "italic".into(), children: collect_inlines(node, source, context) }),
        MarkdownElementType::InlineCode => Some(Inline::InlineCode { text: collect_plain_text(node, source) }),
        MarkdownElementType::Link => parse_inline_link(node, source, context),
        MarkdownElementType::Image => parse_inline_link(node, source, context),
        MarkdownElementType::MathInline => Some(Inline::InlineMath {
            content: strip_math_delimiters(&node.text(source), "$"),
            language: Some("latex".into()),
        }),
        MarkdownElementType::FootnoteReference => {
            let raw = node.text(source);
            let label = raw
                .trim()
                .strip_prefix("[^")
                .and_then(|value| value.strip_suffix(']'))
                .unwrap_or_default()
                .to_string();
            Some(Inline::Styled { style: "footnote_reference".into(), children: vec![Inline::Text { text: label }] })
        }
        MarkdownElementType::Strikethrough => Some(Inline::Text { text: collect_plain_text(node, source) }),
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

fn collect_plain_text(node: RedNode<MarkdownLanguage>, source: &SourceText) -> String {
    let mut text = String::new();
    for child in node.children() {
        match child {
            RedTree::Node(child_node) => text.push_str(&collect_plain_text(child_node, source)),
            RedTree::Leaf(_) => text.push_str(&child.text(source)),
        }
    }
    if text.is_empty() {
        text = node.text(source).into_owned();
    }
    text
}

fn parse_inline_link(node: RedNode<MarkdownLanguage>, source: &SourceText, context: &mut LinkContext) -> Option<Inline> {
    let raw = node.text(source);
    let style = if node.element_type() == MarkdownElementType::Image { "image" } else { "link" };
    if let Some((text, url)) = split_markdown_link(raw.as_ref()) {
        return Some(Inline::Styled { style: style.into(), children: vec![Inline::Text { text }, Inline::Text { text: url }] });
    }
    if let Some((text, label, explicit)) = split_reference_link(raw.trim_end()) {
        if let Some(destination) = context.definitions.get(&normalize_link_label(&label)) {
            return Some(Inline::Styled { style: style.into(), children: vec![Inline::Text { text }, Inline::Text { text: destination.clone() }] });
        }
        if explicit {
            context.losses.push(LossMarker {
                code: "import.markdown.unresolved_reference_link".into(),
                message: format!("unresolved reference link label: {label}"),
                status: SemanticStatus::Unresolved,
            });
        }
    }
    Some(Inline::Text { text: decode_escaped_text(raw.trim_end()) })
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
    text.trim()
        .strip_prefix(delimiter)
        .and_then(|value| value.strip_suffix(delimiter))
        .unwrap_or(text.trim())
        .to_string()
}

fn trailing_link_whitespace(raw: &str) -> String {
    let Some(start) = raw.find("](") else {
        return raw[raw.trim_end_matches([' ', '\t']).len()..].to_string();
    };
    let destination = &raw[start + 2..];
    let Some(close) = link_destination_close(destination) else {
        return String::new();
    };
    destination[close + 1..]
        .chars()
        .take_while(|character| character.is_whitespace())
        .collect()
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
    matches!(text, "*" | "**" | "_" | "__" | "`" | "[" | "]" | "(" | ")" | "![")
}

fn extract_code_block(node: RedNode<MarkdownLanguage>, source: &SourceText) -> (Option<String>, String) {
    let mut language = None;
    let mut content = String::new();
    let mut in_content = false;
    for child in node.children() {
        if let RedTree::Node(child_node) = child {
            match child_node.element_type() {
                MarkdownElementType::CodeLanguage => {
                    language = Some(collect_plain_text(child_node, source).trim().to_string());
                }
                MarkdownElementType::CodeFence => in_content = !in_content,
                MarkdownElementType::Text | MarkdownElementType::Whitespace | MarkdownElementType::Newline if in_content => {
                    content.push_str(&child_node.text(source));
                }
                _ => {}
            }
        }
    }
    (language.filter(|value| !value.is_empty()), content.trim().to_string())
}

fn extract_list(node: RedNode<MarkdownLanguage>, source: &SourceText, context: &mut LinkContext) -> (bool, Vec<ListItem>) {
    let mut ordered = false;
    let mut items = Vec::new();
    for child in node.children() {
        if let RedTree::Node(child_node) = child {
            if child_node.element_type() == MarkdownElementType::ListItem {
                if items.is_empty() {
                    let marker = child_node.text(source);
                    ordered = marker.trim_start().chars().next().map(|ch| ch.is_ascii_digit()).unwrap_or(false);
                }
                items.push(ListItem { content: collect_inlines(child_node, source, context), children: Vec::new() });
            }
        }
    }
    (ordered, items)
}

fn extract_blockquote_text(node: RedNode<MarkdownLanguage>, source: &SourceText) -> String {
    let mut parts = Vec::new();
    for child in node.children() {
        if let RedTree::Node(child_node) = child {
            if child_node.element_type() == MarkdownElementType::Paragraph {
                parts.push(collect_plain_text(child_node, source));
            }
        }
    }
    parts.join("\n")
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
