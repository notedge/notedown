use notedown_ir::{
    AttributePair, Attributes, Block, BreakKind, DocumentGraph, DocumentId, Inline, LossMarker, NodeId, RelationEndpoint, SemanticStatus,
    SourceKind, SourceRef,
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
    for (block, span) in lower_children(body_children(&document), &mut graph, text) {
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
fn lower_children(children: &[HtmlNode], graph: &mut DocumentGraph, source: &str) -> Vec<(Block, std::ops::Range<usize>)> {
    let mut result = Vec::new();
    let mut pending = Vec::new();
    let mut pending_span: Option<std::ops::Range<usize>> = None;
    for child in children {
        match child {
            HtmlNode::Element(element) if is_block_element(element) => {
                flush_inline_run(&mut result, &mut pending, &mut pending_span);
                result.extend(lower_blocks(element, graph, source));
            }
            HtmlNode::Element(element) if is_unsupported_element(element) => {}
            HtmlNode::Element(element) => {
                if pending_span.is_none() {
                    pending_span = Some(element.span.clone().into());
                }
                pending.extend(lower_inline_element(element, graph, source));
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

/// Read a table cell span attribute.
///
/// The IR carries spans as counts, so they map directly when the authored value
/// is a positive integer. Anything else cannot be represented and is refused
/// with a loss marker instead of silently collapsing the cell to a single slot.
fn cell_span(attribute_name: &str, cell: &Element, graph: &mut DocumentGraph) -> u32 {
    let Some(raw) = attribute(cell, attribute_name) else {
        return 1;
    };
    if let Ok(span) = raw.trim().parse::<u32>() {
        if span >= 1 {
            return span;
        }
    }
    graph.push_loss(LossMarker {
        code: "import.html.invalid_cell_span".into(),
        message: format!("`{attribute_name}` value `{raw}` is not a positive integer, so the cell keeps a span of 1"),
        status: SemanticStatus::Partial,
    });
    1
}

fn decode_hex(text: &str) -> Option<String> {
    if text.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(text.len() / 2);
    for pair in text.as_bytes().chunks_exact(2) {
        bytes.push(u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?);
    }
    String::from_utf8(bytes).ok()
}

fn lower_blocks(element: &Element, graph: &mut DocumentGraph, source: &str) -> Vec<(Block, std::ops::Range<usize>)> {
    let tag = element.tag_name.to_ascii_lowercase();
    let span: std::ops::Range<usize> = element.span.clone().into();
    match tag.as_str() {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            vec![(
                Block::Section { level: tag[1..].parse().unwrap_or(1), title: lower_inlines(element, graph, source), children: Vec::new() },
                span,
            )]
        }
        "p" => vec![(Block::Paragraph { content: lower_inlines(element, graph, source) }, span)],
        "blockquote" => {
            let mut content = Vec::new();
            let mut children = Vec::new();
            for (child, child_span) in lower_children(&element.children, graph, source) {
                match child {
                    Block::Paragraph { content: paragraph } => {
                        if !content.is_empty() {
                            content.push(Inline::Break { kind: BreakKind::Soft });
                        }
                        content.extend(paragraph);
                    }
                    block => children.push(push_block_with_source(graph, block, child_span)),
                }
            }
            vec![(Block::Quote { content, children }, span)]
        }
        "pre" => {
            if let Some((language, chart)) = explicit_chart_marker(element, source) {
                vec![(Block::Chart { language, source: chart }, span)]
            }
            else if let Some((language, diagram)) = explicit_diagram_marker(element, source) {
                vec![(Block::Diagram { language, source: diagram }, span)]
            }
            else {
                match hosted_diagram_source(element, graph, source) {
                    HostedDiagram::Source(language, diagram) => vec![(Block::Diagram { language, source: diagram }, span)],
                    HostedDiagram::Empty => lower_children(&element.children, graph, source),
                    HostedDiagram::Absent => {
                        vec![(
                            Block::Code {
                                language: element
                                    .children
                                    .iter()
                                    .filter_map(as_element)
                                    .find(|child| child.tag_name.eq_ignore_ascii_case("code"))
                                    .and_then(|child| attribute(child, "class")),
                                content: preformatted_source(element, source),
                            },
                            span,
                        )]
                    }
                }
            }
        }
        "ul" | "ol" => {
            let ordered = tag == "ol";
            // Ordered items must carry their number: the markdown writer
            // refuses lists whose items lack one. `<ol start="n">` shifts the
            // sequence; unordered items stay unnumbered.
            let start =
                attribute(element, "start").and_then(|value| value.parse::<u32>().ok()).filter(|value| ordered && *value > 0).unwrap_or(1);
            let mut number = start;
            let items = element
                .children
                .iter()
                .filter_map(as_element)
                .filter(|child| child.tag_name.eq_ignore_ascii_case("li"))
                .map(|item| {
                    let item_number = ordered.then_some(number);
                    number += 1;
                    lower_list_item(item, graph, item_number, source)
                })
                .collect();
            vec![(Block::List { ordered, items }, span)]
        }
        "table" => {
            let caption = element
                .children
                .iter()
                .filter_map(as_element)
                .find(|child| child.tag_name.eq_ignore_ascii_case("caption"))
                .map(|cap| {
                    lower_inlines(cap, graph, source)
                        .into_iter()
                        .filter_map(|inline| match inline {
                            notedown_ir::Inline::Text { text } => Some(text),
                            _ => None,
                        })
                        .collect::<String>()
                })
                .and_then(|cap| if cap.is_empty() { None } else { Some(cap) });
            // A cell holds a block layout in HTML (`<td><p>one</p><p>two</p></td>`),
            // but the IR table cell only carries inline content. The authored text
            // survives as a flattened inline run, so this is a recorded partial
            // loss rather than a silent downgrade.
            let mut flattened_cell_blocks = false;
            let rows = descendants(element, "tr")
                .into_iter()
                .map(|row| {
                    let mut is_header = false;
                    let cells: Vec<notedown_ir::TableCell> = row
                        .children
                        .iter()
                        .filter_map(as_element)
                        .filter(|cell| matches!(cell.tag_name.to_ascii_lowercase().as_str(), "td" | "th"))
                        .map(|cell| {
                            if cell.tag_name.eq_ignore_ascii_case("th") {
                                is_header = true;
                            }
                            if cell.children.iter().filter_map(as_element).any(is_block_element) {
                                flattened_cell_blocks = true;
                            }
                            notedown_ir::TableCell {
                                content: lower_inlines(cell, graph, source),
                                colspan: cell_span("colspan", cell, graph),
                                rowspan: cell_span("rowspan", cell, graph),
                            }
                        })
                        .collect();
                    notedown_ir::TableRow { is_header, cells }
                })
                .collect();
            if flattened_cell_blocks {
                graph.push_loss(LossMarker {
                    code: "import.html.table_cell_blocks".into(),
                    message: "table cells authored as block layout are flattened into one inline run".into(),
                    status: SemanticStatus::Partial,
                });
            }
            vec![(Block::Table { caption, rows }, span)]
        }
        "hr" => vec![(Block::Opaque { kind: "thematic_break".into(), payload_hint: "---".into(), status: SemanticStatus::Partial }, span)],
        "dl" => {
            // The IR has no definition list node. Terms and definitions become
            // paragraphs of their own and the downgrade is recorded.
            graph.push_loss(LossMarker {
                code: "import.html.definition_list".into(),
                message: "definition list is lowered to term and definition paragraphs because the IR has no definition list node"
                    .into(),
                status: SemanticStatus::Partial,
            });
            element
                .children
                .iter()
                .filter_map(as_element)
                .filter(|child| matches!(child.tag_name.to_ascii_lowercase().as_str(), "dt" | "dd"))
                .map(|child| (Block::Paragraph { content: lower_inlines(child, graph, source) }, child.span.clone().into()))
                .collect()
        }
        // Inline SVG is captured verbatim as raw markup: the importer never
        // re-parses it into a shape model and never leaks its label text into
        // prose.
        "svg" => vec![(Block::RawHtml { html: raw_element_markup(element, source) }, span)],
        "section" | "article" | "main" | "div" | "figure" | "nav" => {
            if let Some(definitions) = lower_footnote_section(element) {
                definitions
            }
            else if let Some((language, chart)) = explicit_chart_marker(element, source) {
                vec![(Block::Chart { language, source: chart }, span)]
            }
            else {
                match explicit_diagram_marker(element, source) {
                    Some((language, diagram)) => vec![(Block::Diagram { language, source: diagram }, span)],
                    None => match hosted_diagram_source(element, graph, source) {
                        HostedDiagram::Source(language, diagram) => vec![(Block::Diagram { language, source: diagram }, span)],
                        HostedDiagram::Empty | HostedDiagram::Absent => lower_children(&element.children, graph, source),
                    },
                }
            }
        }
        _ => Vec::new(),
    }
}

/// Explicit diagram round-trip marker written by the HTML exporter. Both the
/// source-only `<pre>` fallback and the rendered `<figure>` carry it, so the
/// authored source survives regardless of which shape was emitted. The hex
/// payload keeps the exact authored bytes; element text is only a fallback.
fn explicit_diagram_marker(element: &Element, source: &str) -> Option<(String, String)> {
    let language = attribute(element, "data-diagram-language").filter(|language| !language.trim().is_empty())?;
    let diagram =
        attribute(element, "data-diagram-source-hex").and_then(|hex| decode_hex(&hex)).unwrap_or_else(|| preformatted_source(element, source));
    Some((language, diagram))
}

/// Explicit chart round-trip marker written by the HTML exporter.
fn explicit_chart_marker(element: &Element, source: &str) -> Option<(String, String)> {
    let language = attribute(element, "data-chart-language").filter(|language| !language.trim().is_empty())?;
    let chart =
        attribute(element, "data-chart-source-hex").and_then(|hex| decode_hex(&hex)).unwrap_or_else(|| preformatted_source(element, source));
    Some((language, chart))
}

/// Lower an endnotes container into opaque `footnote_definition` blocks.
///
/// The IR models footnote bodies as opaque authored payloads, so a container
/// must not leak its list structure into the document.
/// `data-footnote-source-hex` restores the exact GFM definition, while the
/// visible `<li>` text is only a fallback for HTML produced elsewhere.
fn lower_footnote_section(element: &Element) -> Option<Vec<(Block, std::ops::Range<usize>)>> {
    let class = attribute(element, "class").unwrap_or_default();
    let has_class = class.split_whitespace().any(|token| token.eq_ignore_ascii_case("footnotes"));
    let has_role = attribute(element, "role").is_some_and(|role| role.eq_ignore_ascii_case("doc-endnotes"));
    if !has_class && !has_role {
        return None;
    }
    let mut definitions = Vec::new();
    for item in descendants(element, "li") {
        let label = attribute(item, "data-footnote-label")
            .or_else(|| attribute(item, "id").and_then(|id| id.strip_prefix("fn").map(|rest| rest.trim_start_matches('-').to_string())));
        let Some(label) = label.map(|label| label.trim().to_string()).filter(|label| !label.is_empty())
        else {
            continue;
        };
        let payload = attribute(item, "data-footnote-source-hex").and_then(|hex| decode_hex(&hex)).unwrap_or_else(|| {
            let body = text_content(item).trim().to_string();
            format!("[^{label}]: {body}")
        });
        definitions.push((
            Block::Opaque { kind: "footnote_definition".into(), payload_hint: payload, status: SemanticStatus::Resolved },
            item.span.clone().into(),
        ));
    }
    if definitions.is_empty() {
        return None;
    }
    Some(definitions)
}

/// Recognize a footnote reference element and collapse it to `Inline::Note`.
///
/// Only explicit markers qualify: the exporter's `data-footnote-label`, an
/// anchor class of `footnote-ref`, or `role="doc-noteref"`. A plain intra-page
/// anchor stays a link, and Pandoc-style `a.footnote-ref > sup` is accepted.
fn footnote_reference_inline(element: &Element) -> Option<Inline> {
    let link = if element.tag_name.eq_ignore_ascii_case("a") {
        element
    }
    else {
        element.children.iter().filter_map(as_element).find(|child| child.tag_name.eq_ignore_ascii_case("a"))?
    };
    if !has_footnote_marker(element) && !has_footnote_marker(link) {
        return None;
    }
    let href = attribute(link, "href").unwrap_or_default();
    let from_href = href.trim().strip_prefix("#fn").map(|rest| rest.trim_start_matches('-').to_string()).filter(|label| !label.is_empty());
    let label = attribute(element, "data-footnote-label").or_else(|| attribute(link, "data-footnote-label")).or(from_href)?.trim().to_string();
    if label.is_empty() {
        return None;
    }
    let text = text_content(link).trim().to_string();
    // GFM footnote references have no custom display: the marker text is the
    // label. Keep `display` only when the authored text differs, so the
    // Markdown writer can tell an authored display apart from the default.
    let display = (!text.is_empty() && text != label).then(|| vec![Inline::Text { text }]);
    Some(Inline::Note { label, display })
}

fn has_footnote_marker(element: &Element) -> bool {
    if attribute(element, "data-footnote-label").is_some() {
        return true;
    }
    if attribute(element, "role").is_some_and(|role| role.eq_ignore_ascii_case("doc-noteref")) {
        return true;
    }
    attribute(element, "class").is_some_and(|class| {
        class.split_whitespace().any(|token| token.eq_ignore_ascii_case("footnote-ref") || token.eq_ignore_ascii_case("footnote-reference"))
    })
}

/// Outcome of probing a hosted diagram container (`.mermaid` and friends).
enum HostedDiagram {
    /// No hosted diagram marker on the container.
    Absent,
    /// The marker is present but the container carries no text source: a
    /// runtime-rendered fallback such as `<div class="mermaid"><img/></div>`.
    /// The bitmap must stay reachable on the default path, so the caller
    /// lowers the children instead of promoting an empty diagram.
    Empty,
    /// The marker is present and the container carries authored text source.
    Source(String, String),
}

/// Resolve the hosted diagram marker of a container. An image-only or
/// otherwise textless container records an explicit loss instead of being
/// swallowed as an empty `Block::Diagram`.
fn hosted_diagram_source(element: &Element, graph: &mut DocumentGraph, source: &str) -> HostedDiagram {
    let Some(language) = hosted_diagram_language(element)
    else {
        return HostedDiagram::Absent;
    };
    let diagram = diagram_source(element, source);
    if diagram.trim().is_empty() {
        graph.push_loss(LossMarker {
            code: "import.html.empty_diagram_container".into(),
            message: format!("a `{language}` diagram container carried no text source, so its contents were kept on the default path"),
            status: SemanticStatus::Partial,
        });
        return HostedDiagram::Empty;
    }
    HostedDiagram::Source(language, diagram)
}

/// Diagram containers used by real pages: the Mermaid runtime selects
/// `.mermaid` blocks (`<div class="mermaid">` or `<pre class="mermaid">`), and
/// static site generators emit fenced diagrams as
/// `<pre><code class="language-mermaid">`. Only an explicit mermaid marker
/// becomes a diagram; every other class list keeps the default lowering.
fn hosted_diagram_language(element: &Element) -> Option<String> {
    if let Some(language) = attribute(element, "class").as_deref().and_then(diagram_language_in_class_list) {
        return Some(language);
    }
    if element.tag_name.eq_ignore_ascii_case("pre") {
        return element
            .children
            .iter()
            .filter_map(as_element)
            .find(|child| child.tag_name.eq_ignore_ascii_case("code"))
            .and_then(|code| attribute(code, "class"))
            .and_then(|class| diagram_language_in_class_list(&class));
    }
    None
}

fn diagram_language_in_class_list(class: &str) -> Option<String> {
    class.split_whitespace().find_map(|token| {
        let bare = match token.get(..9).filter(|prefix| prefix.eq_ignore_ascii_case("language-")) {
            Some(_) => &token[9..],
            None => token,
        };
        if is_diagram_language(bare) { Some(bare.to_ascii_lowercase()) } else { None }
    })
}

/// Keep the diagram languages the Markdown importer promotes to
/// `Block::Diagram`; unknown or absent markers stay on the default path.
fn is_diagram_language(language: &str) -> bool {
    language.eq_ignore_ascii_case("mermaid") || language.eq_ignore_ascii_case("dot") || language.eq_ignore_ascii_case("plantuml")
}

/// HTML formatting commonly pads the authored diagram source with a leading
/// and trailing newline; strip those without touching the authored lines.
fn diagram_source(element: &Element, source: &str) -> String {
    preformatted_source(element, source).trim_matches(|character| character == '\r' || character == '\n').to_string()
}

/// `<pre>` content is preformatted, but the HTML front end collapses
/// whitespace in text nodes. Recover the authored bytes by slicing the raw
/// source at each child span — mirroring what a browser's `textContent`
/// reader (the Mermaid runtime included) would see — and decode character
/// references afterwards.
fn preformatted_source(element: &Element, source: &str) -> String {
    decode_character_references(&raw_subtree_text(element, source))
}

fn raw_subtree_text(element: &Element, source: &str) -> String {
    let mut raw = String::new();
    for child in &element.children {
        match child {
            HtmlNode::Text(text) => raw.push_str(source.get(text.span.clone()).unwrap_or(&text.content)),
            HtmlNode::Element(child) => raw.push_str(&raw_subtree_text(child, source)),
            HtmlNode::Comment(_) => {}
        }
    }
    raw
}

/// The element's exact authored markup, recovered from its source span.
fn raw_element_markup(element: &Element, source: &str) -> String {
    source.get(element.span.clone()).map(str::to_string).unwrap_or_else(|| text_content(element))
}

/// Decode the character references real diagram and code sources carry.
/// Unknown or malformed references stay verbatim instead of being dropped.
fn decode_character_references(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let candidate = &rest[start..];
        let Some(end) = candidate.find(';')
        else {
            out.push_str(candidate);
            return out;
        };
        let reference = &candidate[..end];
        let decoded = match reference.strip_prefix("&#") {
            Some(digits) => digits
                .strip_prefix(['x', 'X'])
                .map_or_else(|| digits.parse::<u32>().ok(), |hex| u32::from_str_radix(hex, 16).ok())
                .and_then(char::from_u32),
            None => match reference {
                "&amp" => Some('&'),
                "&lt" => Some('<'),
                "&gt" => Some('>'),
                "&quot" => Some('"'),
                "&apos" => Some('\''),
                "&nbsp" => Some('\u{00A0}'),
                _ => None,
            },
        };
        match decoded {
            Some(character) => {
                out.push(character);
                rest = &candidate[end + 1..];
            }
            None => {
                out.push('&');
                rest = &candidate[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn lower_list_item(item: &Element, graph: &mut DocumentGraph, number: Option<u32>, source: &str) -> notedown_ir::ListItem {
    let mut children = Vec::new();
    for child in item.children.iter().filter_map(as_element) {
        if is_block_element(child) {
            for (block, span) in lower_blocks(child, graph, source) {
                children.push(push_block_with_source(graph, block, span));
            }
        }
    }
    notedown_ir::ListItem { content: lower_list_item_inlines(item, graph, source), children, number, is_task: false, is_checked: None }
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
            | "dl"
            | "svg"
            | "section"
            | "article"
            | "main"
            | "div"
            | "figure"
            | "nav"
    )
}

fn lower_list_item_inlines(item: &Element, graph: &mut DocumentGraph, source: &str) -> Vec<Inline> {
    let mut result = Vec::new();
    for child in &item.children {
        match child {
            HtmlNode::Text(text) => push_text(&mut result, text.content.clone()),
            HtmlNode::Element(element) if !is_block_element(element) && !is_unsupported_element(element) => {
                result.extend(lower_inline_element(element, graph, source));
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

fn lower_inlines(element: &Element, graph: &mut DocumentGraph, source: &str) -> Vec<Inline> {
    let mut result = Vec::new();
    for child in &element.children {
        match child {
            HtmlNode::Text(text) => push_text(&mut result, text.content.clone()),
            HtmlNode::Element(child) => {
                if !is_unsupported_element(child) {
                    result.extend(lower_inline_element(child, graph, source));
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

fn lower_inline_element(element: &Element, graph: &mut DocumentGraph, source: &str) -> Vec<Inline> {
    let tag = element.tag_name.to_ascii_lowercase();
    // Inline-position SVG keeps its exact markup instead of leaking label text.
    if tag == "svg" {
        return vec![Inline::RawHtml { html: raw_element_markup(element, source) }];
    }
    let nested = lower_inlines(element, graph, source);
    // Footnote references are semantic notes, not superscript text. Both the
    // exporter marker (`sup.footnote-reference[data-footnote-label]`) and the
    // common Markdown HTML shape (`a.footnote-ref[role=doc-noteref]` wrapping a
    // `<sup>`) collapse back to `Inline::Note`.
    if matches!(tag.as_str(), "sup" | "a") {
        if let Some(note) = footnote_reference_inline(element) {
            return vec![note];
        }
    }
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
/// anchors and style hooks survive into the semantic layer. `<img>` elements
/// additionally keep their authored `width`/`height` in the ordered pairs so
/// bitmap consumers can recover the display size.
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
        else if element.tag_name.eq_ignore_ascii_case("img")
            && (attribute.name.eq_ignore_ascii_case("width") || attribute.name.eq_ignore_ascii_case("height"))
        {
            if let Some(value) = attribute.value.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
                attributes.pairs.push(AttributePair { key: attribute.name.to_ascii_lowercase(), value: value.to_string() });
            }
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
