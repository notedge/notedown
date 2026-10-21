use notedown_ir::{
    Attributes, Block, BreakKind, DocumentGraph, DocumentId, Inline, LossMarker, NodeId, RelationEndpoint, SemanticStatus, SourceKind,
    SourceRef,
};
use oak_core::{Builder, SourceText, parser::session::ParseSession};
use oak_html::{
    HtmlBuilder, HtmlLanguage,
    ast::{Element, HtmlDocument, HtmlNode},
};

use super::common::register_image_assets;
use crate::FormatError;

/// Attribute role that produced an HTML link candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlLinkRole {
    /// Anchor `href`.
    Href,
    /// Form `action`.
    FormAction,
    /// Image or source `src`.
    Src,
    /// Image or source `srcset` entry.
    SrcSet,
}

/// Navigation or asset candidate discovered during HTML import.
///
/// The target stays exactly as authored. Resolving it against the page URL,
/// applying navigation policy, and classifying the relation belong to the
/// consumer, not to the format layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlLink {
    /// Raw target value from the attribute.
    pub target: String,
    /// Attribute role that produced this candidate.
    pub role: HtmlLinkRole,
    /// Byte range start of the attribute in the imported text.
    pub start: usize,
    /// Byte range end of the attribute in the imported text.
    pub end: usize,
}

/// HTML import result with semantic graph and link candidates.
#[derive(Debug, Clone)]
pub struct HtmlImport {
    /// Imported document graph.
    pub document: DocumentGraph,
    /// `<base href>` value when the document declares one.
    pub base_href: Option<String>,
    /// Link and asset candidates in document order.
    pub links: Vec<HtmlLink>,
}

/// Import HTML from UTF-8 bytes.
pub fn import_html_bytes(label: &str, bytes: &[u8]) -> Result<DocumentGraph, FormatError> {
    import_html_bytes_with_links(label, bytes).map(|import| import.document)
}

/// Import HTML from UTF-8 bytes with link candidates.
pub fn import_html_bytes_with_links(label: &str, bytes: &[u8]) -> Result<HtmlImport, FormatError> {
    let text = std::str::from_utf8(bytes).map_err(|error| FormatError::parse("html", format!("{label}: {error}")))?;
    import_html_with_links(label, text)
}

/// Import HTML from a UTF-8 string.
pub fn import_html_text(label: &str, text: &str) -> Result<DocumentGraph, FormatError> {
    import_html_with_links(label, text).map(|import| import.document)
}

/// Import HTML from a UTF-8 string with link candidates.
pub fn import_html_with_links(label: &str, text: &str) -> Result<HtmlImport, FormatError> {
    let source = SourceText::new(text);
    let builder = HtmlBuilder::new(HtmlLanguage::default());
    let mut session = ParseSession::<HtmlLanguage>::default();
    let document = builder.build(&source, &[], &mut session).result.map_err(|error| FormatError::parse("html", format!("{label}: {error}")))?;
    let mut graph = DocumentGraph::new(DocumentId(1));
    if let Some(title) = document.nodes.iter().filter_map(as_element).find_map(|element| find_element(element, "title")) {
        let title = text_content(title).trim().to_string();
        if !title.is_empty() {
            graph.metadata.title = Some(title);
        }
    }
    let base_href = document
        .nodes
        .iter()
        .filter_map(as_element)
        .find_map(|element| find_element(element, "base"))
        .and_then(|element| attribute(element, "href"))
        .map(|href| href.trim().to_string())
        .filter(|href| !href.is_empty());
    let links = collect_links(&document);
    for (block, span) in lower_children(body_children(&document), &mut graph) {
        push_block_with_source(&mut graph, block, span);
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
    Ok(HtmlImport { document: graph, base_href, links })
}

/// Collect link and asset candidates from the document in source order.
fn collect_links(document: &HtmlDocument) -> Vec<HtmlLink> {
    let mut links = Vec::new();
    for node in &document.nodes {
        if let HtmlNode::Element(element) = node {
            collect_element_links(element, &mut links);
        }
    }
    links
}

fn collect_element_links(element: &Element, links: &mut Vec<HtmlLink>) {
    let tag = element.tag_name.to_ascii_lowercase();
    if is_skipped_subtree_tag(&tag) {
        return;
    }
    match tag.as_str() {
        "a" => push_link(element, "href", HtmlLinkRole::Href, links),
        "form" => push_link(element, "action", HtmlLinkRole::FormAction, links),
        "img" | "source" => {
            push_link(element, "src", HtmlLinkRole::Src, links);
            if let Some(srcset) = element.attributes.iter().find(|attribute| attribute.name.eq_ignore_ascii_case("srcset")) {
                for target in parse_srcset(srcset.value.as_deref().unwrap_or_default()) {
                    links.push(HtmlLink { target, role: HtmlLinkRole::SrcSet, start: srcset.span.start, end: srcset.span.end });
                }
            }
        }
        _ => {}
    }
    for child in element.children.iter().filter_map(as_element) {
        collect_element_links(child, links);
    }
}

fn push_link(element: &Element, attribute_name: &str, role: HtmlLinkRole, links: &mut Vec<HtmlLink>) {
    let Some(attribute) = element.attributes.iter().find(|attribute| attribute.name.eq_ignore_ascii_case(attribute_name))
    else {
        return;
    };
    let target = attribute.value.as_deref().unwrap_or_default().trim();
    if target.is_empty() {
        return;
    }
    links.push(HtmlLink { target: target.to_string(), role, start: attribute.span.start, end: attribute.span.end });
}

fn is_skipped_subtree_tag(tag: &str) -> bool {
    matches!(tag, "script" | "style" | "noscript" | "template")
}

fn parse_srcset(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter_map(|part| {
            let url = part.trim().split_whitespace().next().unwrap_or("").trim();
            if url.is_empty() { None } else { Some(url.to_string()) }
        })
        .collect()
}

fn body_children(document: &HtmlDocument) -> &[HtmlNode] {
    let body = document.nodes.iter().filter_map(as_element).find_map(find_body);
    body.map(|element| element.children.as_slice()).unwrap_or_else(|| document.nodes.as_slice())
}

/// Lower sibling nodes in document order without dropping authored inline
/// content that sits directly under a container element.
fn lower_children(children: &[HtmlNode], graph: &mut DocumentGraph) -> Vec<(Block, std::ops::Range<usize>)> {
    let mut result = Vec::new();
    let mut pending = Vec::new();
    let mut pending_span: Option<std::ops::Range<usize>> = None;
    for child in children {
        match child {
            HtmlNode::Element(element) if is_block_element(element) => {
                flush_inline_run(&mut result, &mut pending, &mut pending_span);
                result.extend(lower_blocks(element, graph));
            }
            HtmlNode::Element(element) if is_unsupported_element(element) => {}
            HtmlNode::Element(element) => {
                if pending_span.is_none() {
                    pending_span = Some(element.span.clone().into());
                }
                pending.extend(lower_inline_element(element, graph));
            }
            HtmlNode::Text(text) => {
                if pending_span.is_none() {
                    pending_span = Some(text.span.clone().into());
                }
                push_text(&mut pending, text.content.clone());
            }
            HtmlNode::Comment(_) => {}
        }
    }
    flush_inline_run(&mut result, &mut pending, &mut pending_span);
    result
}

fn flush_inline_run(result: &mut Vec<(Block, std::ops::Range<usize>)>, pending: &mut Vec<Inline>, span: &mut Option<std::ops::Range<usize>>) {
    let Some(span) = span.take()
    else {
        return;
    };
    if pending.iter().all(|inline| matches!(inline, Inline::Text { text } if text.trim().is_empty())) {
        pending.clear();
        return;
    }
    result.push((Block::Paragraph { content: std::mem::take(pending) }, span));
}

/// Push a lowered block and record its byte range in the imported text.
fn push_block_with_source(graph: &mut DocumentGraph, block: Block, span: std::ops::Range<usize>) -> NodeId {
    let node = graph.push_block(block);
    graph.attach_source(SourceRef {
        node,
        kind: SourceKind::TextSpan { start: span.start as u32, end: span.end as u32 },
        precision: SemanticStatus::Resolved,
    });
    node
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
    if matches!(
        tag.as_str(),
        "script" | "style" | "form" | "input" | "select" | "textarea" | "button" | "canvas" | "video" | "audio" | "iframe"
    ) {
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

fn lower_blocks(element: &Element, graph: &mut DocumentGraph) -> Vec<(Block, std::ops::Range<usize>)> {
    let tag = element.tag_name.to_ascii_lowercase();
    let span: std::ops::Range<usize> = element.span.clone().into();
    match tag.as_str() {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            vec![(Block::Section { level: tag[1..].parse().unwrap_or(1), title: lower_inlines(element, graph), children: Vec::new() }, span)]
        }
        "p" => vec![(Block::Paragraph { content: lower_inlines(element, graph) }, span)],
        "blockquote" => vec![(Block::Quote { content: lower_inlines(element, graph) }, span)],
        "pre" => vec![(
            Block::Code {
                language: element
                    .children
                    .iter()
                    .filter_map(as_element)
                    .find(|child| child.tag_name.eq_ignore_ascii_case("code"))
                    .and_then(|child| attribute(child, "class")),
                content: text_content(element),
            },
            span,
        )],
        "ul" | "ol" => {
            let items = element
                .children
                .iter()
                .filter_map(as_element)
                .filter(|child| child.tag_name.eq_ignore_ascii_case("li"))
                .map(|item| lower_list_item(item, graph))
                .collect();
            vec![(Block::List { ordered: tag == "ol", items }, span)]
        }
        "table" => {
            let rows = descendants(element, "tr")
                .into_iter()
                .map(|row| notedown_ir::TableRow {
                    cells: row
                        .children
                        .iter()
                        .filter_map(as_element)
                        .filter(|cell| matches!(cell.tag_name.to_ascii_lowercase().as_str(), "td" | "th"))
                        .map(|cell| lower_inlines(cell, graph))
                        .collect(),
                })
                .collect();
            vec![(Block::Table { rows }, span)]
        }
        "hr" => vec![(Block::Opaque { kind: "thematic_break".into(), payload_hint: "---".into(), status: SemanticStatus::Partial }, span)],
        "section" | "article" | "main" | "div" | "figure" | "nav" => lower_children(&element.children, graph),
        _ => Vec::new(),
    }
}

fn lower_list_item(item: &Element, graph: &mut DocumentGraph) -> notedown_ir::ListItem {
    let mut children = Vec::new();
    for child in item.children.iter().filter_map(as_element) {
        if is_block_element(child) {
            for (block, span) in lower_blocks(child, graph) {
                children.push(push_block_with_source(graph, block, span));
            }
        }
    }
    notedown_ir::ListItem { content: lower_list_item_inlines(item, graph), children }
}

fn is_block_element(element: &Element) -> bool {
    matches!(
        element.tag_name.to_ascii_lowercase().as_str(),
        "p" | "blockquote"
            | "pre"
            | "ul"
            | "ol"
            | "table"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "hr"
            | "section"
            | "article"
            | "main"
            | "div"
            | "figure"
            | "nav"
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
    let inlines = match tag.as_str() {
        "strong" | "b" => vec![Inline::Styled { style: "bold".into(), children: nested }],
        "em" | "i" => vec![Inline::Styled { style: "italic".into(), children: nested }],
        "s" | "del" | "strike" => vec![Inline::Styled { style: "strike".into(), children: nested }],
        "u" => vec![Inline::Styled { style: "underline".into(), children: nested }],
        "sup" => vec![Inline::Styled { style: "superscript".into(), children: nested }],
        "sub" => vec![Inline::Styled { style: "subscript".into(), children: nested }],
        "code" => vec![Inline::InlineCode { text: text_content(element) }],
        "a" => {
            let target = attribute(element, "href").unwrap_or_default();
            if target.trim().is_empty() {
                nested
            }
            else {
                vec![Inline::Link { target: RelationEndpoint::ExternalUri(target), display: nested, title: attribute(element, "title") }]
            }
        }
        "img" => {
            let alt = attribute(element, "alt").unwrap_or_default();
            let src = attribute(element, "src").unwrap_or_default();
            vec![Inline::Image {
                source: src,
                alt: if alt.is_empty() { Vec::new() } else { vec![Inline::Text { text: alt }] },
                title: attribute(element, "title"),
                asset: None,
            }]
        }
        "br" => {
            let mut inlines = vec![Inline::Break { kind: BreakKind::Hard }];
            inlines.extend(nested);
            inlines
        }
        _ => nested,
    };
    let attributes = inline_attributes(element);
    if attributes.is_empty() { inlines } else { vec![Inline::Attributed { attributes, children: inlines }] }
}

/// Collect the identity and class attributes of an inline element so authored
/// anchors and style hooks survive into the semantic layer.
fn inline_attributes(element: &Element) -> Attributes {
    let mut attributes = Attributes::default();
    for attribute in &element.attributes {
        if attribute.name.eq_ignore_ascii_case("id") {
            if let Some(value) = attribute.value.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
                attributes.id = Some(value.to_string());
            }
        }
        else if attribute.name.eq_ignore_ascii_case("class") {
            attributes.classes = attribute.value.as_deref().unwrap_or_default().split_whitespace().map(str::to_string).collect();
        }
    }
    attributes
}

fn push_text(inlines: &mut Vec<Inline>, text: String) {
    if text.is_empty() {
        return;
    }
    if let Some(Inline::Text { text: previous }) = inlines.last_mut() {
        if previous.ends_with(char::is_whitespace) && text.starts_with(char::is_whitespace) {
            previous.push_str(text.trim_start());
        }
        else {
            previous.push_str(&text);
        }
    }
    else {
        inlines.push(Inline::Text { text });
    }
}
