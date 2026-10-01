use notedown_ir::{Block, Inline, ListItem, LossMarker, SemanticStatus};
use oak_core::parser::session::ParseSession;
use oak_core::{Parser, RedNode, RedTree, SourceText};
use oak_html::parser::element_type::HtmlElementType;
use oak_html::{HtmlLanguage, HtmlParser};

use crate::FormatError;

/// Lower XHTML spine content into `notedown-ir` blocks via `oak-html`.
pub fn blocks_from_xhtml(xml: &[u8]) -> Result<Vec<Block>, FormatError> {
    let text = prepare_xhtml_for_oak_html(&String::from_utf8_lossy(xml));
    let language = HtmlLanguage::default();
    let source = SourceText::new(text.as_str());
    let mut cache = ParseSession::<HtmlLanguage>::default();
    let parser = HtmlParser::new(&language);
    let parsed = parser.parse(&source, &[], &mut cache);
    let green_tree = parsed
        .result
        .map_err(|error| FormatError::parse("epub", error.to_string()))?;

    let root = RedNode::new(&green_tree, 0);
    let scope = find_body_or_document(root, &source);
    let mut blocks = Vec::new();
    let mut losses = Vec::new();
    collect_semantic_blocks(scope, &source, &mut blocks, &mut losses);

    let scraped = scraper_blocks_from_xhtml(text.as_str())?;
    if scraped.len() > blocks.len() {
        blocks = scraped;
    } else if blocks.is_empty() {
        blocks = scraped;
    }

    if !losses.is_empty() && blocks.is_empty() {
        return Err(FormatError::parse(
            "epub",
            losses
                .first()
                .map(|loss| loss.message.clone())
                .unwrap_or_else(|| "xhtml produced no blocks".into()),
        ));
    }

    Ok(blocks)
}

/// Loss markers discovered while lowering XHTML (navigation/assets still pending).
pub fn losses_from_xhtml(xml: &[u8]) -> Result<Vec<LossMarker>, FormatError> {
    let text = prepare_xhtml_for_oak_html(&String::from_utf8_lossy(xml));
    let language = HtmlLanguage::default();
    let source = SourceText::new(text.as_str());
    let mut cache = ParseSession::<HtmlLanguage>::default();
    let parser = HtmlParser::new(&language);
    let parsed = parser.parse(&source, &[], &mut cache);
    let green_tree = parsed
        .result
        .map_err(|error| FormatError::parse("epub", error.to_string()))?;

    let root = RedNode::new(&green_tree, 0);
    let scope = find_body_or_document(root, &source);
    let mut blocks = Vec::new();
    let mut losses = Vec::new();
    collect_semantic_blocks(scope, &source, &mut blocks, &mut losses);
    Ok(losses)
}

fn find_body_or_document<'a>(
    root: RedNode<'a, HtmlLanguage>,
    source: &SourceText,
) -> RedNode<'a, HtmlLanguage> {
    if let Some(body) = find_first_element_by_tag(root, source, "body") {
        return body;
    }
    root
}

fn find_first_element_by_tag<'a>(
    node: RedNode<'a, HtmlLanguage>,
    source: &SourceText,
    tag: &str,
) -> Option<RedNode<'a, HtmlLanguage>> {
    if node.element_type() == HtmlElementType::Element {
        if element_tag_name(node, source).as_deref() == Some(tag) {
            return Some(node);
        }
    }
    for child in node.children() {
        if let RedTree::Node(child_node) = child {
            if let Some(found) = find_first_element_by_tag(child_node, source, tag) {
                return Some(found);
            }
        }
    }
    None
}

fn collect_semantic_blocks<'a>(
    node: RedNode<'a, HtmlLanguage>,
    source: &SourceText,
    blocks: &mut Vec<Block>,
    losses: &mut Vec<LossMarker>,
) {
    if node.element_type() == HtmlElementType::Element {
        if let Some(tag) = element_tag_name(node, source) {
            if let Some(block) = lower_semantic_block(node, source, &tag, losses) {
                blocks.push(block);
                return;
            }
        }
    }

    for child in node.children() {
        if let RedTree::Node(child_node) = child {
            collect_semantic_blocks(child_node, source, blocks, losses);
        }
    }
}

fn lower_semantic_block<'a>(
    node: RedNode<'a, HtmlLanguage>,
    source: &SourceText,
    tag: &str,
    losses: &mut Vec<LossMarker>,
) -> Option<Block> {
    match tag {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = tag[1..].parse::<u8>().unwrap_or(1);
            let plain = element_inner_plain_text(node, source);
            let title = if plain.is_empty() {
                collect_inlines_from_element(node, source)
            } else {
                vec![Inline::Text { text: plain }]
            };
            Some(Block::Section {
                level,
                title,
                children: Vec::new(),
            })
        }
        "p" => {
            let content = non_empty_inlines(collect_inlines_from_element(node, source)).or_else(|| {
                let plain = element_inner_plain_text(node, source);
                if plain.is_empty() {
                    None
                } else {
                    Some(vec![Inline::Text { text: plain }])
                }
            })?;
            Some(Block::Paragraph { content })
        }
        "ul" | "ol" => {
            let items = collect_list_items(node, source);
            if items.is_empty() {
                None
            } else {
                Some(Block::List {
                    ordered: tag == "ol",
                    items,
                })
            }
        }
        "blockquote" => Some(Block::Quote {
            content: non_empty_inlines(collect_inlines_from_element(node, source))
                .unwrap_or_else(|| vec![Inline::Text {
                    text: element_inner_plain_text(node, source),
                }]),
        }),
        "pre" => {
            let content = element_inner_plain_text(node, source);
            if content.is_empty() {
                None
            } else {
                Some(Block::Code {
                    language: None,
                    content,
                })
            }
        }
        "script" | "style" | "head" | "meta" | "link" | "title" | "html" | "body" | "section"
        | "article" | "main" | "div" | "nav" => None,
        "table" | "img" | "figure" | "svg" => {
            losses.push(LossMarker {
                code: "import.epub.unsupported_xhtml_block".into(),
                message: format!("unsupported xhtml block element `<{tag}>`"),
                status: SemanticStatus::Unsupported,
            });
            None
        }
        _ => {
            losses.push(LossMarker {
                code: "import.epub.unsupported_xhtml_block".into(),
                message: format!("unsupported xhtml block element `<{tag}>`"),
                status: SemanticStatus::Unsupported,
            });
            None
        }
    }
}

fn non_empty_inlines(inlines: Vec<Inline>) -> Option<Vec<Inline>> {
    if inlines.is_empty() {
        None
    } else {
        Some(inlines)
    }
}

fn element_inner_plain_text(node: RedNode<HtmlLanguage>, source: &SourceText) -> String {
    let raw = node.text(source);
    let open = raw.find('>').map(|index| index + 1).unwrap_or(0);
    let close = raw.rfind('<').unwrap_or(raw.len());
    let slice = if close > open {
        &raw[open..close]
    } else {
        raw.as_ref()
    };
    normalize_whitespace(slice)
}

fn collect_list_items<'a>(
    node: RedNode<'a, HtmlLanguage>,
    source: &SourceText,
) -> Vec<ListItem> {
    let mut items = Vec::new();
    for child in element_body_children(node, source) {
        if child.element_type() != HtmlElementType::Element {
            continue;
        }
        if element_tag_name(child, source).as_deref() != Some("li") {
            continue;
        }
        items.push(ListItem {
            content: collect_inlines_from_element(child, source),
            children: Vec::new(),
        });
    }
    items
}

fn collect_inlines_from_element<'a>(
    node: RedNode<'a, HtmlLanguage>,
    source: &SourceText,
) -> Vec<Inline> {
    let mut inlines = Vec::new();
    let mut past_opening_tag = false;
    for child in node.children() {
        match child {
            RedTree::Node(child_node) => {
                let kind = child_node.element_type();
                if kind == HtmlElementType::TagSlashOpen {
                    break;
                }
                if !past_opening_tag {
                    if kind == HtmlElementType::TagClose {
                        past_opening_tag = true;
                    }
                    continue;
                }
                if kind == HtmlElementType::Element {
                    if let Some(inline) = lower_inline_element(child_node, source) {
                        push_inline(&mut inlines, inline);
                    }
                } else if kind == HtmlElementType::Text {
                    let text = normalize_whitespace(child_node.text(source).as_ref());
                    if !text.is_empty() {
                        push_inline(&mut inlines, Inline::Text { text });
                    }
                } else {
                    let text = normalize_whitespace(child_node.text(source).as_ref());
                    if !text.is_empty() {
                        push_inline(&mut inlines, Inline::Text { text });
                    }
                }
            }
            RedTree::Leaf(_) => {
                if !past_opening_tag {
                    continue;
                }
                let text = normalize_whitespace(child.text(source).as_ref());
                if !text.is_empty() {
                    push_inline(&mut inlines, Inline::Text { text });
                }
            }
        }
    }
    inlines
}

fn lower_inline_element<'a>(
    node: RedNode<'a, HtmlLanguage>,
    source: &SourceText,
) -> Option<Inline> {
    let tag = element_tag_name(node, source)?;
    match tag.as_str() {
        "strong" | "b" => Some(Inline::Styled {
            style: "bold".into(),
            children: collect_inlines_from_element(node, source),
        }),
        "em" | "i" => Some(Inline::Styled {
            style: "italic".into(),
            children: collect_inlines_from_element(node, source),
        }),
        "code" => Some(Inline::InlineCode {
            text: collect_plain_text(node, source),
        }),
        "a" => {
            let children = collect_inlines_from_element(node, source);
            let href = attribute_value(node, source, "href").unwrap_or_default();
            if href.is_empty() {
                return children.first().cloned();
            }
            Some(Inline::Styled {
                style: "link".into(),
                children: if children.is_empty() {
                    vec![Inline::Text { text: href.clone() }, Inline::Text { text: href }]
                } else {
                    let mut linked = children;
                    linked.push(Inline::Text { text: href });
                    linked
                },
            })
        }
        "span" | "sup" | "sub" => {
            let children = collect_inlines_from_element(node, source);
            if children.is_empty() {
                None
            } else if children.len() == 1 {
                children.first().cloned()
            } else {
                Some(Inline::Styled {
                    style: tag,
                    children,
                })
            }
        }
        _ => {
            let text = collect_plain_text(node, source);
            if text.is_empty() {
                None
            } else {
                Some(Inline::Text { text })
            }
        }
    }
}

fn element_body_children<'a>(
    node: RedNode<'a, HtmlLanguage>,
    _source: &SourceText,
) -> Vec<RedNode<'a, HtmlLanguage>> {
    let mut children = Vec::new();
    let mut past_opening_tag = false;
    for child in node.children() {
        if let RedTree::Node(child_node) = child {
            let kind = child_node.element_type();
            if kind == HtmlElementType::TagSlashOpen {
                break;
            }
            if past_opening_tag {
                children.push(child_node);
            } else if kind == HtmlElementType::TagClose {
                past_opening_tag = true;
            }
        }
    }
    children
}

fn element_tag_name(node: RedNode<HtmlLanguage>, source: &SourceText) -> Option<String> {
    for child in node.children() {
        if let RedTree::Node(child_node) = child {
            if child_node.element_type() == HtmlElementType::TagName {
                let name = child_node.text(source).trim().to_ascii_lowercase();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }
    tag_name_from_element_text(node.text(source).as_ref())
}

fn tag_name_from_element_text(text: &str) -> Option<String> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with('<') || trimmed.starts_with("</") {
        return None;
    }
    let inner = &trimmed[1..];
    let end = inner
        .find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
        .unwrap_or(inner.len());
    let tag = inner[..end].trim().to_ascii_lowercase();
    if tag.is_empty() {
        None
    } else {
        Some(tag)
    }
}

fn attribute_value(
    node: RedNode<HtmlLanguage>,
    source: &SourceText,
    name: &str,
) -> Option<String> {
    for child in node.children() {
        if let RedTree::Node(child_node) = child {
            if child_node.element_type() != HtmlElementType::Attribute {
                continue;
            }
            let mut attr_name = None;
            let mut attr_value = None;
            for attr_child in child_node.children() {
                if let RedTree::Node(attr_part) = attr_child {
                    match attr_part.element_type() {
                        HtmlElementType::AttributeName => {
                            attr_name = Some(attr_part.text(source).trim().to_ascii_lowercase());
                        }
                        HtmlElementType::AttributeValue => {
                            attr_value = Some(unquote(attr_part.text(source).as_ref()));
                        }
                        _ => {}
                    }
                }
            }
            if attr_name.as_deref() == Some(name) {
                return attr_value;
            }
        }
    }
    None
}

fn collect_plain_text(node: RedNode<HtmlLanguage>, source: &SourceText) -> String {
    let mut text = String::new();
    for child in node.children() {
        match child {
            RedTree::Node(child_node) => {
                if child_node.element_type() == HtmlElementType::TagSlashOpen {
                    break;
                }
                if child_node.element_type() == HtmlElementType::Text {
                    text.push_str(child_node.text(source).as_ref());
                } else {
                    text.push_str(&collect_plain_text(child_node, source));
                }
            }
            RedTree::Leaf(_) => text.push_str(child.text(source).as_ref()),
        }
    }
    normalize_whitespace(text.trim())
}

fn push_inline(inlines: &mut Vec<Inline>, inline: Inline) {
    if let Inline::Text { text: right } = inline {
        if let Some(Inline::Text { text: left }) = inlines.last_mut() {
            left.push_str(&right);
            return;
        }
        if right.is_empty() {
            return;
        }
        inlines.push(Inline::Text { text: right });
        return;
    }
    inlines.push(inline);
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Normalize EPUB XHTML into HTML that `oak-html` can parse reliably today.
fn prepare_xhtml_for_oak_html(xml: &str) -> String {
    let mut text = xml.trim().to_string();
    if text.starts_with("<?") {
        if let Some(end) = text.find("?>") {
            text = text[end + 2..].trim_start().to_string();
        }
    }
    text = text.replace("xmlns=\"http://www.w3.org/1999/xhtml\"", "");
    text = text.replace("xmlns='http://www.w3.org/1999/xhtml'", "");
    text = text.replace("xml:lang=", "lang=");
    text
}

/// Fallback block extraction when `oak-html` tree shape is still lossy.
fn scraper_blocks_from_xhtml(text: &str) -> Result<Vec<Block>, FormatError> {
    let mut blocks = Vec::new();
    let mut cursor = 0usize;

    while let Some(start) = text[cursor..].find('<') {
        let absolute = cursor + start;
        let rest = &text[absolute + 1..];
        let (tag, is_close) = parse_tag_name(rest)?;
        let close_index = rest
            .find('>')
            .ok_or_else(|| FormatError::parse("epub", "malformed xhtml tag"))?;
        let inner_start = absolute + 1 + close_index + 1;

        if is_close {
            cursor = inner_start;
            continue;
        }

        if matches!(tag.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
            let level = tag[1..].parse::<u8>().unwrap_or(1);
            let (content, next) = slice_until_close(text, inner_start, &tag)?;
            blocks.push(Block::Section {
                level,
                title: vec![Inline::Text {
                    text: strip_tags(content),
                }],
                children: Vec::new(),
            });
            cursor = next;
            continue;
        }

        if tag == "p" {
            let (content, next) = slice_until_close(text, inner_start, "p")?;
            let inlines = inlines_from_html_fragment(content);
            if !inlines.is_empty() {
                blocks.push(Block::Paragraph { content: inlines });
            }
            cursor = next;
            continue;
        }

        cursor = inner_start;
    }

    Ok(blocks)
}

fn inlines_from_html_fragment(content: &str) -> Vec<Inline> {
    let mut inlines = Vec::new();
    let mut cursor = 0usize;
    while let Some(start) = content[cursor..].find('<') {
        let absolute = cursor + start;
        let rest = &content[absolute + 1..];
        if rest.starts_with('/') {
            cursor = absolute + 1;
            continue;
        }
        let (tag, _) = parse_tag_name(rest).unwrap_or((String::new(), false));
        let close_index = rest.find('>').unwrap_or(0);
        let inner_start = absolute + 1 + close_index + 1;
        if matches!(tag.as_str(), "strong" | "b" | "em" | "i") {
            let (inner, next) = slice_until_close(content, inner_start, &tag)
                .unwrap_or((&content[inner_start..], content.len()));
            let style = if matches!(tag.as_str(), "strong" | "b") {
                "bold"
            } else {
                "italic"
            };
            push_inline(
                &mut inlines,
                Inline::Styled {
                    style: style.into(),
                    children: vec![Inline::Text {
                        text: strip_tags(inner),
                    }],
                },
            );
            cursor = next;
            continue;
        }
        cursor = inner_start;
    }
    let plain = strip_tags(&content[cursor..]);
    if !plain.is_empty() {
        push_inline(&mut inlines, Inline::Text { text: plain });
    }
    if inlines.is_empty() {
        let plain = strip_tags(content);
        if !plain.is_empty() {
            inlines.push(Inline::Text { text: plain });
        }
    }
    inlines
}

fn parse_tag_name(rest: &str) -> Result<(String, bool), FormatError> {
    let trimmed = rest.trim_start();
    let close = trimmed.starts_with('/');
    let name = trimmed.trim_start_matches('/');
    let end = name
        .find(|ch: char| ch == '>' || ch.is_whitespace() || ch == '/')
        .unwrap_or(name.len());
    let tag = name[..end].to_string();
    if tag.is_empty() {
        return Err(FormatError::parse("epub", "empty xhtml tag"));
    }
    Ok((tag, close))
}

fn slice_until_close<'a>(
    text: &'a str,
    start: usize,
    tag: &str,
) -> Result<(&'a str, usize), FormatError> {
    let close = format!("</{tag}>");
    let slice = &text[start..];
    let end = slice
        .find(&close)
        .ok_or_else(|| FormatError::parse("epub", format!("unclosed <{tag}>")))?;
    Ok((&slice[..end], start + end + close.len()))
}

fn strip_tags(text: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    normalize_whitespace(output.trim())
}

fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_XHTML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <body>
    <h1>Chapter One</h1>
    <p>Hello <strong>EPUB</strong></p>
  </body>
</html>"#;

    #[test]
    fn oak_html_lowers_headings_paragraphs_and_inline_styles() {
        let blocks = blocks_from_xhtml(SAMPLE_XHTML.as_bytes()).expect("lower xhtml");
        assert!(blocks.len() >= 2, "blocks={blocks:?}");
        let markdown = notedown_ir::DocumentGraph::new(notedown_ir::DocumentId(1));
        let mut graph = markdown;
        for block in blocks {
            graph.push_block(block);
        }
        let exported = crate::export::markdown::export_markdown(&graph).expect("export");
        assert!(exported.contains("# Chapter One"));
        assert!(exported.contains("**EPUB**"));
    }
}
