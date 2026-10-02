//! EPUB XHTML block lowering via typed `oak-html` AST and CSS selectors.

use notedown_ir::{
    Block, DocumentGraph, Inline, ListItem, LossMarker, SemanticStatus, TableRow,
};
use oak_html::ast::{Element, HtmlDocument, HtmlNode};

use super::assets::{image_inline, register_image_from_src};
use super::oak_html_util::{
    attribute_value, child_elements, descendant_text, direct_child_elements, element_is,
    first_descendant_element, select_css,
};
use super::xhtml::{
    language_from_class, normalize_whitespace, push_inline, quote_block_from_blocks,
    register_and_image_inline,
};

/// Lower a parsed HTML document into semantic blocks and import losses.
pub fn blocks_from_html_document(
    document: &HtmlDocument,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
) -> (Vec<Block>, Vec<LossMarker>) {
    let mut blocks = Vec::new();
    let mut losses = Vec::new();
    let scope = body_elements(document);
    for element in scope {
        collect_semantic_blocks(
            element,
            &mut blocks,
            &mut losses,
            graph,
            member_path,
            false,
        );
    }
    (blocks, losses)
}

fn body_elements(document: &HtmlDocument) -> Vec<&Element> {
    if let Ok(bodies) = select_css(document, "body") {
        if let Some(body) = bodies.first() {
            return direct_child_elements(body);
        }
    }
    document
        .nodes
        .iter()
        .filter_map(|node| match node {
            HtmlNode::Element(element) => Some(element),
            _ => None,
        })
        .collect()
}

fn collect_semantic_blocks(
    element: &Element,
    blocks: &mut Vec<Block>,
    losses: &mut Vec<LossMarker>,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
    inside_list_item: bool,
) {
    let tag = element.tag_name.to_ascii_lowercase();
    if tag == "li" {
        for child in direct_child_elements(element) {
            collect_semantic_blocks(child, blocks, losses, graph, member_path, true);
        }
        return;
    }
    if inside_list_item && (tag == "ul" || tag == "ol") {
        return;
    }
    if matches!(tag.as_str(), "section" | "article" | "main" | "div" | "figure") {
        blocks.extend(collect_child_blocks(element, graph, member_path, losses));
        return;
    }
    if let Some(block) = lower_semantic_block(element, &tag, losses, graph, member_path) {
        blocks.push(block);
        return;
    }
    for child in direct_child_elements(element) {
        collect_semantic_blocks(child, blocks, losses, graph, member_path, inside_list_item);
    }
}

fn collect_child_blocks(
    element: &Element,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
    losses: &mut Vec<LossMarker>,
) -> Vec<Block> {
    let mut blocks = Vec::new();
    for child in direct_child_elements(element) {
        collect_semantic_blocks(child, &mut blocks, losses, graph, member_path, false);
    }
    blocks
}

fn lower_semantic_block(
    element: &Element,
    tag: &str,
    losses: &mut Vec<LossMarker>,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
) -> Option<Block> {
    note_inline_style_loss(element, losses);
    match tag {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = tag[1..].parse::<u8>().unwrap_or(1);
            let plain = descendant_text(element);
            let title = if plain.is_empty() {
                collect_inlines_from_element(element, graph, member_path, losses)
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
            let content = non_empty_inlines(
                collect_inlines_from_element(element, graph, member_path, losses),
            )
            .or_else(|| {
                let plain = descendant_text(element);
                if plain.is_empty() {
                    None
                } else {
                    Some(vec![Inline::Text { text: plain }])
                }
            })?;
            Some(Block::Paragraph { content })
        }
        "figcaption" => lower_figcaption_block(element, graph, member_path, losses),
        "ul" | "ol" => {
            let items = collect_list_items(element, graph, member_path, losses);
            if items.is_empty() {
                None
            } else {
                Some(Block::List {
                    ordered: tag == "ol",
                    items,
                })
            }
        }
        "blockquote" => {
            let inner = collect_child_blocks(element, graph, member_path, losses);
            if inner.is_empty() {
                let plain = descendant_text(element);
                if plain.is_empty() {
                    None
                } else {
                    Some(Block::Quote {
                        content: vec![Inline::Text { text: plain }],
                    })
                }
            } else {
                Some(quote_block_from_blocks(inner))
            }
        }
        "pre" => {
            let (language, content) = extract_pre_code_content(element);
            if content.is_empty() {
                None
            } else {
                Some(Block::Code { language, content })
            }
        }
        "hr" => Some(Block::Opaque {
            kind: "thematic_break".into(),
            payload_hint: "---".into(),
            status: SemanticStatus::Partial,
        }),
        "script" | "head" | "meta" | "title" | "html" | "body" | "section"
        | "article" | "main" | "div" | "nav" | "figure" => None,
        "style" => {
            losses.push(LossMarker {
                code: "import.epub.css_inline_block_unsupported".into(),
                message: "embedded <style> blocks are not lowered into Notedown IR yet".into(),
                status: SemanticStatus::Unsupported,
            });
            None
        }
        "link" => {
            let rel = attribute_value(element, "rel").unwrap_or_default();
            if rel.eq_ignore_ascii_case("stylesheet") {
                losses.push(LossMarker {
                    code: "import.epub.css_link_unsupported".into(),
                    message: "linked stylesheets are registered as package assets but not applied during import".into(),
                    status: SemanticStatus::Unsupported,
                });
            }
            None
        }
        "img" => {
            let src = attribute_value(element, "src").unwrap_or_default();
            let alt = attribute_value(element, "alt").unwrap_or_default();
            register_and_image_inline(&src, &alt, graph, member_path).map(|inline| {
                Block::Paragraph {
                    content: vec![inline],
                }
            })
        }
        "svg" => extract_svg_image_href(element)
            .and_then(|(href, alt)| {
                register_and_image_inline(&href, &alt, graph, member_path).map(|inline| {
                    Block::Paragraph {
                        content: vec![inline],
                    }
                })
            })
            .or_else(|| {
                losses.push(LossMarker {
                    code: "import.epub.svg_inline_unsupported".into(),
                    message: "inline <svg> without an external <image> reference is not lowered yet"
                        .into(),
                    status: SemanticStatus::Unsupported,
                });
                None
            }),
        "table" => {
            let rows = collect_table_rows(element, graph, member_path, losses);
            if rows.is_empty() {
                None
            } else {
                Some(Block::Table { rows })
            }
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

fn lower_figcaption_block(
    element: &Element,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
    losses: &mut Vec<LossMarker>,
) -> Option<Block> {
    let content = non_empty_inlines(collect_inlines_from_element(element, graph, member_path, losses))
        .or_else(|| {
            let plain = descendant_text(element);
            if plain.is_empty() {
                None
            } else {
                Some(vec![Inline::Text { text: plain }])
            }
        })?;
    Some(Block::Paragraph {
        content: vec![Inline::Styled {
            style: "figcaption".into(),
            children: content,
        }],
    })
}

fn collect_list_items(
    element: &Element,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
    losses: &mut Vec<LossMarker>,
) -> Vec<ListItem> {
    child_elements(element, "li")
        .into_iter()
        .map(|li| lower_list_item(li, graph, member_path, losses))
        .collect()
}

fn lower_list_item(
    li: &Element,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
    losses: &mut Vec<LossMarker>,
) -> ListItem {
    let mut content = Vec::new();
    let mut children = Vec::new();
    for child in &li.children {
        match child {
            HtmlNode::Text(text) => {
                let normalized = normalize_whitespace(&text.content);
                if !normalized.is_empty() {
                    push_inline(&mut content, Inline::Text { text: normalized });
                }
            }
            HtmlNode::Element(child_element) => {
                note_inline_style_loss(child_element, losses);
                let tag = child_element.tag_name.to_ascii_lowercase();
                match tag.as_str() {
                    "ul" | "ol" => {
                        let nested = collect_list_items(child_element, graph, member_path, losses);
                        if !nested.is_empty() {
                            let id = graph.push_block(Block::List {
                                ordered: tag == "ol",
                                items: nested,
                            });
                            children.push(id);
                        }
                    }
                    "p" => {
                        for inline in collect_inlines_from_element(
                            child_element,
                            graph,
                            member_path,
                            losses,
                        ) {
                            push_inline(&mut content, inline);
                        }
                    }
                    _ => {
                        if let Some(inline) =
                            lower_inline_element(child_element, graph, member_path, losses)
                        {
                            push_inline(&mut content, inline);
                        }
                    }
                }
            }
            HtmlNode::Comment(_) => {}
        }
    }
    ListItem { content, children }
}

fn collect_table_rows(
    element: &Element,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
    losses: &mut Vec<LossMarker>,
) -> Vec<TableRow> {
    let mut rows = Vec::new();
    walk_table_rows(element, graph, member_path, losses, &mut rows);
    rows
}

fn walk_table_rows(
    element: &Element,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
    losses: &mut Vec<LossMarker>,
    rows: &mut Vec<TableRow>,
) {
    if element_is(element, "tr") {
        rows.push(collect_table_row(element, graph, member_path, losses));
        return;
    }
    for child in direct_child_elements(element) {
        walk_table_rows(child, graph, member_path, losses, rows);
    }
}

fn collect_table_row(
    row: &Element,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
    losses: &mut Vec<LossMarker>,
) -> TableRow {
    let mut cells = Vec::new();
    for child in direct_child_elements(row) {
        let tag = child.tag_name.to_ascii_lowercase();
        if tag == "th" || tag == "td" {
            cells.push(collect_inlines_from_element(child, graph, member_path, losses));
        }
    }
    TableRow { cells }
}

fn collect_inlines_from_element(
    element: &Element,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
    losses: &mut Vec<LossMarker>,
) -> Vec<Inline> {
    let mut inlines = Vec::new();
    for child in &element.children {
        match child {
            HtmlNode::Text(text) => {
                let normalized = normalize_whitespace(&text.content);
                if !normalized.is_empty() {
                    push_inline(&mut inlines, Inline::Text { text: normalized });
                }
            }
            HtmlNode::Element(child_element) => {
                note_inline_style_loss(child_element, losses);
                if let Some(inline) =
                    lower_inline_element(child_element, graph, member_path, losses)
                {
                    push_inline(&mut inlines, inline);
                }
            }
            HtmlNode::Comment(_) => {}
        }
    }
    inlines
}

fn lower_inline_element(
    element: &Element,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
    losses: &mut Vec<LossMarker>,
) -> Option<Inline> {
    let tag = element.tag_name.to_ascii_lowercase();
    match tag.as_str() {
        "img" => {
            let src = attribute_value(element, "src").unwrap_or_default();
            let alt = attribute_value(element, "alt").unwrap_or_default();
            register_and_image_inline(&src, &alt, graph, member_path)
        }
        "strong" | "b" => Some(Inline::Styled {
            style: "bold".into(),
            children: collect_inlines_from_element(element, graph, member_path, losses),
        }),
        "em" | "i" => Some(Inline::Styled {
            style: "italic".into(),
            children: collect_inlines_from_element(element, graph, member_path, losses),
        }),
        "code" => Some(Inline::InlineCode {
            text: descendant_text(element),
        }),
        "a" => {
            let children = collect_inlines_from_element(element, graph, member_path, losses);
            let href = attribute_value(element, "href").unwrap_or_default();
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
            let children = collect_inlines_from_element(element, graph, member_path, losses);
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
            let text = descendant_text(element);
            if text.is_empty() {
                None
            } else {
                Some(Inline::Text { text })
            }
        }
    }
}

fn note_inline_style_loss(element: &Element, losses: &mut Vec<LossMarker>) {
    if attribute_value(element, "style").is_some() {
        losses.push(LossMarker {
            code: "import.epub.inline_style_unsupported".into(),
            message: "inline style attributes are not lowered into Notedown IR yet".into(),
            status: SemanticStatus::Unsupported,
        });
    }
}

fn extract_svg_image_href(element: &Element) -> Option<(String, String)> {
    if element_is(element, "image") {
        let href = attribute_value(element, "href")
            .or_else(|| attribute_value(element, "xlink:href"))?;
        if href.is_empty() {
            return None;
        }
        let alt = attribute_value(element, "alt")
            .or_else(|| attribute_value(element, "aria-label"))
            .unwrap_or_default();
        return Some((href, alt));
    }
    for child in direct_child_elements(element) {
        if let Some(found) = extract_svg_image_href(child) {
            return Some(found);
        }
    }
    None
}

fn extract_pre_code_content(element: &Element) -> (Option<String>, String) {
    if let Some(code) = first_descendant_element(element, "code") {
        let language = attribute_value(code, "class")
            .as_deref()
            .map(language_from_class);
        return (language, descendant_text(code));
    }
    (None, descendant_text(element))
}

/// Register embedded image assets from an already parsed HTML document.
pub fn register_spine_images_from_document(
    document: &HtmlDocument,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
) -> Result<(), crate::FormatError> {
    let Some(member_path) = member_path else {
        return Ok(());
    };
    for img in select_css(document, "img[src]")? {
        if let Some(src) = attribute_value(img, "src") {
            register_image_from_src(graph, member_path, &src);
        }
    }
    for image in select_css(document, "svg image")? {
        let href = attribute_value(image, "href")
            .or_else(|| attribute_value(image, "xlink:href"));
        if let Some(href) = href {
            register_image_from_src(graph, member_path, &href);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::oak_html_util::parse_html_bytes;

    #[test]
    fn body_selector_scopes_block_lowering() {
        const XHTML: &str = r#"<html><head><title>Skip</title></head><body><p>Body</p></body></html>"#;
        let document = parse_html_bytes(XHTML.as_bytes()).expect("parse");
        let mut graph = notedown_ir::DocumentGraph::new(notedown_ir::DocumentId(0));
        let (blocks, _) = blocks_from_html_document(&document, &mut graph, None);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0], Block::Paragraph { .. }));
    }
}
