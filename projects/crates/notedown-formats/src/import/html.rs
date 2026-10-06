use notedown_ir::{
    Asset, AssetId, AssetKind, Block, DocumentGraph, DocumentId, Inline, LinkId, LossMarker,
    Relation, RelationEndpoint, RelationKind, SemanticStatus,
};
use oak_core::parser::session::ParseSession;
use oak_core::{Builder, SourceText};
use oak_html::ast::{Element, HtmlDocument, HtmlNode};
use oak_html::{HtmlBuilder, HtmlLanguage};

use crate::FormatError;

pub fn import_html_bytes(label: &str, bytes: &[u8]) -> Result<DocumentGraph, FormatError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| FormatError::parse("html", format!("{label}: {error}")))?;
    import_html_text(label, text)
}

pub fn import_html_text(label: &str, text: &str) -> Result<DocumentGraph, FormatError> {
    let source = SourceText::new(text);
    let builder = HtmlBuilder::new(HtmlLanguage::default());
    let mut session = ParseSession::<HtmlLanguage>::default();
    let document = builder
        .build(&source, &[], &mut session)
        .result
        .map_err(|error| FormatError::parse("html", format!("{label}: {error}")))?;
    let mut graph = DocumentGraph::new(DocumentId(1));
    if let Some(title) = document
        .nodes
        .iter()
        .filter_map(as_element)
        .find_map(|element| find_element(element, "title"))
    {
        let title = text_content(title).trim().to_string();
        if !title.is_empty() {
            graph.metadata.title = Some(title);
        }
    }
    let roots = body_children(&document);
    for element in roots {
        for block in lower_blocks(element, &mut graph) {
            graph.push_block(block);
        }
    }
    register_image_assets(&mut graph);
    collect_unsupported_losses(&document, &mut graph);
    if graph.blocks.is_empty() {
        graph.push_loss(LossMarker {
            code: "import.html.empty_document".into(),
            message: "HTML contained no supported block elements".into(),
            status: SemanticStatus::Partial,
        });
    }
    graph.push_loss(LossMarker {
        code: "import.html.partial_coverage".into(),
        message: "HTML import preserves common document blocks and inline links/images, but not CSS layout, scripts, forms, or arbitrary DOM semantics".into(),
        status: SemanticStatus::Partial,
    });
    Ok(graph)
}

fn body_children(document: &HtmlDocument) -> Vec<&Element> {
    let body = document.nodes.iter().filter_map(as_element).find_map(find_body);
    body.map(|element| element.children.iter().filter_map(as_element).collect())
        .unwrap_or_else(|| document.nodes.iter().filter_map(as_element).collect())
}

fn find_body(element: &Element) -> Option<&Element> {
    if element.tag_name.eq_ignore_ascii_case("body") {
        return Some(element);
    }
    element.children.iter().filter_map(as_element).find_map(find_body)
}

fn find_element<'a>(element: &'a Element, tag: &str) -> Option<&'a Element> {
    if element.tag_name.eq_ignore_ascii_case(tag) {
        return Some(element);
    }
    element.children.iter().filter_map(as_element).find_map(|child| find_element(child, tag))
}

fn collect_unsupported_losses(document: &HtmlDocument, graph: &mut DocumentGraph) {
    for root in document.nodes.iter().filter_map(as_element) {
        collect_unsupported_element_losses(root, graph);
    }
}

fn collect_unsupported_element_losses(element: &Element, graph: &mut DocumentGraph) {
    let tag = element.tag_name.to_ascii_lowercase();
    if matches!(tag.as_str(), "script" | "style" | "form" | "input" | "select" | "textarea" | "button" | "canvas" | "video" | "audio" | "iframe") {
        graph.push_loss(LossMarker {
            code: format!("import.html.unsupported_{tag}"),
            message: format!("HTML element `<{tag}>` is not represented in notedown-ir"),
            status: SemanticStatus::Unsupported,
        });
    }
    for child in element.children.iter().filter_map(as_element) {
        collect_unsupported_element_losses(child, graph);
    }
}

fn as_element(node: &HtmlNode) -> Option<&Element> {
    match node {
        HtmlNode::Element(element) => Some(element),
        _ => None,
    }
}

fn attribute(element: &Element, name: &str) -> Option<String> {
    element.attributes.iter().find(|attribute| attribute.name.eq_ignore_ascii_case(name)).and_then(|attribute| attribute.value.clone())
}

fn lower_blocks(element: &Element, graph: &mut DocumentGraph) -> Vec<Block> {
    let tag = element.tag_name.to_ascii_lowercase();
    match tag.as_str() {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => vec![Block::Section {
            level: tag[1..].parse().unwrap_or(1),
            title: lower_inlines(element, graph),
            children: Vec::new(),
        }],
        "p" => vec![Block::Paragraph { content: lower_inlines(element, graph) }],
        "blockquote" => vec![Block::Quote { content: lower_inlines(element, graph) }],
        "pre" => vec![Block::Code {
            language: element.children.iter().filter_map(as_element).find(|child| child.tag_name.eq_ignore_ascii_case("code")).and_then(|child| attribute(child, "class")),
            content: text_content(element),
        }],
        "ul" | "ol" => {
            let items = element.children.iter().filter_map(as_element).filter(|child| child.tag_name.eq_ignore_ascii_case("li")).map(|item| lower_list_item(item, graph)).collect();
            vec![Block::List { ordered: tag == "ol", items }]
        }
        "table" => {
            let rows = descendants(element, "tr").into_iter().map(|row| notedown_ir::TableRow {
                cells: row.children.iter().filter_map(as_element).filter(|cell| matches!(cell.tag_name.to_ascii_lowercase().as_str(), "td" | "th")).map(|cell| lower_inlines(cell, graph)).collect(),
            }).collect();
            vec![Block::Table { rows }]
        }
        "hr" => vec![Block::Opaque { kind: "thematic_break".into(), payload_hint: "---".into(), status: SemanticStatus::Partial }],
        "section" | "article" | "main" | "div" | "figure" | "nav" => {
            let mut blocks = Vec::new();
            for child in element.children.iter().filter_map(as_element) {
                blocks.extend(lower_blocks(child, graph));
            }
            blocks
        }
        _ => Vec::new(),
    }
}

fn lower_list_item(item: &Element, graph: &mut DocumentGraph) -> notedown_ir::ListItem {
    let mut children = Vec::new();
    for child in item.children.iter().filter_map(as_element) {
        if is_block_element(child) {
            for block in lower_blocks(child, graph) {
                children.push(graph.push_block(block));
            }
        }
    }
    notedown_ir::ListItem {
        content: lower_list_item_inlines(item, graph),
        children,
    }
}

fn is_block_element(element: &Element) -> bool {
    matches!(
        element.tag_name.to_ascii_lowercase().as_str(),
        "p" | "blockquote" | "pre" | "ul" | "ol" | "table" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "hr" | "section" | "article" | "main" | "div" | "figure" | "nav"
    )
}

fn lower_list_item_inlines(item: &Element, graph: &mut DocumentGraph) -> Vec<Inline> {
    let mut result = Vec::new();
    for child in &item.children {
        match child {
            HtmlNode::Text(text) => push_text(&mut result, text.content.clone()),
            HtmlNode::Element(element) if !is_block_element(element) && !is_unsupported_element(element) => {
                result.extend(lower_inline_element(element, graph));
            }
            HtmlNode::Element(_) | HtmlNode::Comment(_) => {}
        }
    }
    result
}

fn descendants<'a>(element: &'a Element, tag: &str) -> Vec<&'a Element> {
    let mut result = Vec::new();
    for child in element.children.iter().filter_map(as_element) {
        if child.tag_name.eq_ignore_ascii_case(tag) {
            result.push(child);
        }
        result.extend(descendants(child, tag));
    }
    result
}

fn text_content(element: &Element) -> String {
    let mut text = String::new();
    for child in &element.children {
        match child {
            HtmlNode::Text(text_node) => text.push_str(&text_node.content),
            HtmlNode::Element(child) => text.push_str(&text_content(child)),
            HtmlNode::Comment(_) => {}
        }
    }
    text
}

fn lower_inlines(element: &Element, graph: &mut DocumentGraph) -> Vec<Inline> {
    let mut result = Vec::new();
    for child in &element.children {
        match child {
            HtmlNode::Text(text) => push_text(&mut result, text.content.clone()),
            HtmlNode::Element(child) => {
                if !is_unsupported_element(child) {
                    result.extend(lower_inline_element(child, graph));
                }
            }
            HtmlNode::Comment(_) => {}
        }
    }
    result
}

fn is_unsupported_element(element: &Element) -> bool {
    matches!(
        element.tag_name.to_ascii_lowercase().as_str(),
        "script" | "style" | "form" | "input" | "select" | "textarea" | "button" | "canvas" | "video" | "audio" | "iframe"
    )
}

fn lower_inline_element(element: &Element, graph: &mut DocumentGraph) -> Vec<Inline> {
    let tag = element.tag_name.to_ascii_lowercase();
    let nested = lower_inlines(element, graph);
    match tag.as_str() {
        "strong" | "b" => vec![Inline::Styled { style: "bold".into(), children: nested }],
        "em" | "i" => vec![Inline::Styled { style: "italic".into(), children: nested }],
        "code" => vec![Inline::InlineCode { text: text_content(element) }],
        "a" => {
            let target = attribute(element, "href").unwrap_or_default();
            vec![Inline::Styled { style: "link".into(), children: vec![Inline::Text { text: text_content(element) }, Inline::Text { text: target }] }]
        }
        "img" => {
            let alt = attribute(element, "alt").unwrap_or_default();
            let src = attribute(element, "src").unwrap_or_default();
            vec![Inline::Styled { style: "image".into(), children: vec![Inline::Text { text: alt }, Inline::Text { text: src }] }]
        }
        "br" => vec![Inline::Text { text: "\n".into() }],
        _ => nested,
    }
}

fn push_text(inlines: &mut Vec<Inline>, text: String) {
    if text.is_empty() { return; }
    if let Some(Inline::Text { text: previous }) = inlines.last_mut() {
        previous.push_str(&text);
    } else {
        inlines.push(Inline::Text { text });
    }
}

fn register_image_assets(graph: &mut DocumentGraph) {
    let mut next_asset = 1u64;
    let mut next_link = 1u64;
    let mut occurrences = Vec::new();
    for node in &graph.blocks {
        collect_images(node.id, &node.block, &mut occurrences);
    }
    for (node, source) in occurrences {
        graph.push_asset(Asset { id: AssetId(next_asset), kind: AssetKind::Image, content_identity: None, source: Some(source.clone()), media_type: None, status: SemanticStatus::Unresolved, bytes: None });
        graph.push_relation(Relation { id: LinkId(next_link), kind: RelationKind::Embeds, source: RelationEndpoint::Node(node), target: RelationEndpoint::Asset(AssetId(next_asset)) });
        next_asset += 1;
        next_link += 1;
    }
}

fn collect_images(node: notedown_ir::NodeId, block: &Block, occurrences: &mut Vec<(notedown_ir::NodeId, String)>) {
    let inlines = match block {
        Block::Section { title, .. } => title,
        Block::Paragraph { content } | Block::Quote { content } => content,
        Block::List { items, .. } => { for item in items { collect_inline_images(node, &item.content, occurrences); } return; }
        Block::Table { rows } => { for row in rows { for cell in &row.cells { collect_inline_images(node, cell, occurrences); } } return; }
        _ => return,
    };
    collect_inline_images(node, inlines, occurrences);
}

fn collect_inline_images(node: notedown_ir::NodeId, inlines: &[Inline], occurrences: &mut Vec<(notedown_ir::NodeId, String)>) {
    for inline in inlines {
        if let Inline::Styled { style, children } = inline {
            if style == "image" && children.len() >= 2 {
                if let Inline::Text { text } = &children[1] { occurrences.push((node, text.clone())); }
            }
            collect_inline_images(node, children, occurrences);
        }
    }
}
