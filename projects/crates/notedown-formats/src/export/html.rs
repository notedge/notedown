use std::{collections::HashMap, fmt::Write as _};

use notedown_ir::{Attributes, Block, BreakKind, DocumentGraph, Inline, ListItem, NodeId, RelationEndpoint, TableRow};

use crate::FormatError;

/// A diagram scene rendered by the host, embedded into the HTML output in
/// place of the source-only fallback.
///
/// The host owns scene lowering (for example the DaVinci diagram chain); this
/// exporter only owns the markup shape. `svg` must be a serialized, valid SVG
/// fragment and is emitted verbatim inside the figure.
pub struct RenderedDiagram {
    /// Inline SVG markup serialized from the resolved display scene.
    pub svg: String,
}

/// Export a semantic document graph as a standalone HTML fragment.
///
/// Diagram blocks without a host-rendered scene keep the source-only `<pre>`
/// fallback. Hosts that render diagrams use [`export_html_with_diagrams`].
pub fn export_html(graph: &DocumentGraph) -> Result<String, FormatError> {
    export_html_with_diagrams(graph, &HashMap::new())
}

/// Export a semantic document graph as a standalone HTML fragment, embedding
/// host-rendered diagram scenes keyed by the block that authored them.
///
/// A rendered diagram becomes a `<figure>` carrying the inline SVG plus the
/// `data-diagram-language` / `data-diagram-source-hex` round-trip markers, so
/// the HTML importer still recovers the authored source. A diagram block whose
/// scene failed to render keeps the visible source `<pre>` fallback.
pub fn export_html_with_diagrams(graph: &DocumentGraph, diagrams: &HashMap<NodeId, RenderedDiagram>) -> Result<String, FormatError> {
    let validation = graph.validate();
    if !validation.is_valid() {
        return Err(FormatError::invalid_input(format!("invalid document graph: {:?}", validation.issues)));
    }

    let mut out = String::new();
    out.push_str(r#"<article class="notedown-document">"#);
    if let Some(title) = &graph.metadata.title {
        out.push_str("<header><h1>");
        escape_text(&mut out, title);
        out.push_str("</h1></header>");
    }
    let nested = nested_block_ids(graph);
    let mut footnotes: Vec<&str> = Vec::new();
    for node in &graph.blocks {
        if nested.contains(&node.id) {
            continue;
        }
        if let Block::Opaque { kind, payload_hint, .. } = &node.block {
            if kind == "footnote_definition" {
                footnotes.push(payload_hint.as_str());
                continue;
            }
        }
        write_node(&mut out, graph, node.id, &node.block, diagrams)?;
    }
    if !footnotes.is_empty() {
        write_footnote_section(&mut out, &footnotes);
    }
    out.push_str("</article>");
    Ok(out)
}

/// Footnote definitions have no block form in HTML: they are collected into
/// one endnotes section so the reference links resolve. Every entry keeps the
/// authored GFM payload in `data-footnote-source-hex` so the HTML importer can
/// restore the exact definition instead of re-serializing it from display text.
fn write_footnote_section(out: &mut String, definitions: &[&str]) {
    out.push_str(r#"<section class="footnotes" role="doc-endnotes"><ol>"#);
    for payload in definitions {
        let Some((label, body)) = split_footnote_definition(payload)
        else {
            continue;
        };
        out.push_str(r#"<li id="fn-"#);
        escape_attribute(out, &label);
        out.push_str(r#"" data-footnote-source-hex=""#);
        escape_attribute(out, &encode_hex(payload));
        out.push_str(r#""><p>"#);
        escape_text(out, &body);
        out.push_str(r##"</p><a href="#fnref-"##);
        escape_attribute(out, &label);
        out.push_str(r#"">↩</a>"#);
        // The backlink lives outside the paragraph so the imported body stays
        // byte-identical to the authored definition.
        out.push_str(r#"</li>"#);
    }
    out.push_str("</ol></section>");
}

/// Split a GFM footnote definition payload (`[^label]: body`) into its label
/// and body. Continuation lines stay in the body verbatim.
fn split_footnote_definition(payload: &str) -> Option<(String, String)> {
    let rest = payload.strip_prefix("[^")?;
    let (label, body) = rest.split_once("]:")?;
    let label = label.trim();
    if label.is_empty() {
        return None;
    }
    Some((label.to_string(), body.trim_start_matches([' ', '\t', '\n', '\r']).trim_end().to_string()))
}

fn write_node(
    out: &mut String,
    graph: &DocumentGraph,
    id: NodeId,
    block: &Block,
    diagrams: &HashMap<NodeId, RenderedDiagram>,
) -> Result<(), FormatError> {
    write!(out, r#"<div id="node-{}" data-node-id="{}">"#, id.0, id.0).map_err(|error| FormatError::unsupported("html", error.to_string()))?;
    write_block(out, graph, id, block, diagrams)?;
    out.push_str("</div>");
    Ok(())
}

fn nested_block_ids(graph: &DocumentGraph) -> std::collections::HashSet<notedown_ir::NodeId> {
    let mut ids = std::collections::HashSet::new();
    for node in &graph.blocks {
        match &node.block {
            Block::Section { children, .. } => ids.extend(children.iter().copied()),
            Block::Quote { children, .. } => ids.extend(children.iter().copied()),
            Block::List { items, .. } => {
                for item in items {
                    ids.extend(item.children.iter().copied());
                }
            }
            _ => {}
        }
    }
    ids
}

fn write_block(
    out: &mut String,
    graph: &DocumentGraph,
    id: NodeId,
    block: &Block,
    diagrams: &HashMap<NodeId, RenderedDiagram>,
) -> Result<(), FormatError> {
    match block {
        Block::Section { level, title, children } => {
            let level = (*level).clamp(1, 6);
            write!(out, "<h{}>", level).map_err(format_error)?;
            write_inlines(out, title)?;
            write!(out, "</h{}>", level).map_err(format_error)?;
            for child_id in children {
                let child =
                    graph.block(*child_id).ok_or_else(|| FormatError::invalid_input(format!("section child `{child_id:?}` is missing")))?;
                write_node(out, graph, child.id, &child.block, diagrams)?;
            }
        }
        Block::Paragraph { content } => {
            out.push_str("<p>");
            write_inlines(out, content)?;
            out.push_str("</p>");
        }
        Block::Quote { content, children } => {
            out.push_str("<blockquote>");
            write_inlines(out, content)?;
            for child_id in children {
                let child =
                    graph.block(*child_id).ok_or_else(|| FormatError::invalid_input(format!("quote child `{child_id:?}` is missing")))?;
                write_node(out, graph, child.id, &child.block, diagrams)?;
            }
            out.push_str("</blockquote>");
        }
        Block::Code { language, content } => {
            out.push_str("<pre><code");
            if let Some(language) = language {
                out.push_str(r#" class="language-"#);
                escape_attribute(out, language);
                out.push('"');
            }
            out.push('>');
            escape_text(out, content);
            out.push_str("</code></pre>");
        }
        Block::Diagram { language, source } => match diagrams.get(&id) {
            Some(rendered) => {
                out.push_str(r#"<figure class="diagram" data-diagram-language=""#);
                escape_attribute(out, language);
                out.push_str(r#"" data-diagram-source-hex=""#);
                out.push_str(&encode_hex(source));
                out.push_str(r#"">"#);
                out.push_str(&rendered.svg);
                out.push_str("</figure>");
            }
            None => {
                out.push_str(r#"<pre class="diagram" data-diagram-language=""#);
                escape_attribute(out, language);
                out.push_str(r#"" data-diagram-source-hex=""#);
                out.push_str(&encode_hex(source));
                out.push_str(r#""><code>"#);
                escape_text(out, source);
                out.push_str("</code></pre>");
            }
        },
        Block::Chart { language, source } => match diagrams.get(&id) {
            Some(rendered) => {
                out.push_str(r#"<figure class="chart" data-chart-language=""#);
                escape_attribute(out, language);
                out.push_str(r#"" data-chart-source-hex=""#);
                out.push_str(&encode_hex(source));
                out.push_str(r#"">"#);
                out.push_str(&rendered.svg);
                out.push_str("</figure>");
            }
            None => {
                out.push_str(r#"<pre class="chart" data-chart-language=""#);
                escape_attribute(out, language);
                out.push_str(r#"" data-chart-source-hex=""#);
                out.push_str(&encode_hex(source));
                out.push_str(r#""><code>"#);
                escape_text(out, source);
                out.push_str("</code></pre>");
            }
        },
        Block::Math { formula } => {
            out.push_str(r#"<div class="math""#);
            if let Some(language) = &formula.language {
                out.push_str(r#" data-language=""#);
                escape_attribute(out, language);
                out.push('"');
            }
            out.push('>');
            escape_text(out, &formula.source);
            out.push_str("</div>");
        }
        Block::List { ordered, items } => write_list(out, graph, *ordered, items, diagrams)?,
        Block::Table { rows, .. } => write_table(out, rows)?,
        Block::Opaque { kind, payload_hint, .. } if kind == "thematic_break" => {
            out.push_str("<hr>");
            if !payload_hint.is_empty() {
                out.push_str(r#"<span class="opaque-hint">"#);
                escape_text(out, payload_hint);
                out.push_str("</span>");
            }
        }
        Block::Opaque { kind, .. } => {
            return Err(FormatError::unsupported("html", format!("opaque block kind {kind} is not supported by the HTML exporter")));
        }
        Block::RawHtml { html } => {
            out.push_str(html);
        }
    }
    Ok(())
}

fn write_list(
    out: &mut String,
    graph: &DocumentGraph,
    ordered: bool,
    items: &[ListItem],
    diagrams: &HashMap<NodeId, RenderedDiagram>,
) -> Result<(), FormatError> {
    out.push_str(if ordered { "<ol>" } else { "<ul>" });
    for item in items {
        if item.is_task {
            out.push_str(r#"<li class="task-list-item"><input type="checkbox" disabled"#);
            if item.is_checked.unwrap_or(false) {
                out.push_str(" checked");
            }
            out.push_str(">");
        }
        else {
            out.push_str("<li>");
        }
        write_inlines(out, &item.content)?;
        for child_id in &item.children {
            let child = graph.block(*child_id).ok_or_else(|| FormatError::invalid_input(format!("list child `{child_id:?}` is missing")))?;
            write_node(out, graph, child.id, &child.block, diagrams)?;
        }
        out.push_str("</li>");
    }
    out.push_str(if ordered { "</ol>" } else { "</ul>" });
    Ok(())
}

fn write_table(out: &mut String, rows: &[TableRow]) -> Result<(), FormatError> {
    out.push_str("<table>");
    for row in rows {
        out.push_str("<tr>");
        let tag = if row.is_header { "th" } else { "td" };
        for cell in &row.cells {
            out.push('<');
            out.push_str(tag);
            if cell.colspan > 1 {
                out.push_str(" colspan=\"");
                out.push_str(&cell.colspan.to_string());
                out.push('"');
            }
            if cell.rowspan > 1 {
                out.push_str(" rowspan=\"");
                out.push_str(&cell.rowspan.to_string());
                out.push('"');
            }
            out.push('>');
            write_inlines(out, &cell.content)?;
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }
        out.push_str("</tr>");
    }
    out.push_str("</table>");
    Ok(())
}

fn write_inlines(out: &mut String, inlines: &[Inline]) -> Result<(), FormatError> {
    for inline in inlines {
        match inline {
            Inline::Text { text } => escape_text(out, text),
            Inline::InlineCode { text } => {
                out.push_str("<code>");
                escape_text(out, text);
                out.push_str("</code>");
            }
            Inline::InlineMath { formula } => {
                out.push_str(r#"<span class="math""#);
                if let Some(language) = &formula.language {
                    out.push_str(r#" data-language=""#);
                    escape_attribute(out, language);
                    out.push('"');
                }
                out.push('>');
                escape_text(out, &formula.source);
                out.push_str("</span>");
            }
            Inline::Reference { display, target } => {
                write!(out, r##"<a href="#node-{}">"##, target.0).map_err(format_error)?;
                escape_text(out, display);
                out.push_str("</a>");
            }
            Inline::Styled { style, children } => {
                if let Some(color) = style.strip_prefix("color:") {
                    if color.len() != 6 || !color.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                        return Err(FormatError::unsupported("html", format!("invalid color style `{style}`")));
                    }
                    write!(out, "<span style=\"color:#{color}\">").map_err(format_error)?;
                    write_inlines(out, children)?;
                    out.push_str("</span>");
                    continue;
                }
                let tag = match style.as_str() {
                    "bold" | "strong" => "strong",
                    "italic" | "emphasis" => "em",
                    "underline" => "u",
                    "strike" | "strikethrough" => "s",
                    "superscript" => "sup",
                    "subscript" => "sub",
                    _ => {
                        return Err(FormatError::unsupported("html", format!("inline style `{style}` is not representable")));
                    }
                };
                write!(out, "<{}>", tag).map_err(format_error)?;
                write_inlines(out, children)?;
                write!(out, "</{}>", tag).map_err(format_error)?;
            }
            Inline::Link { target, display, title } => {
                let RelationEndpoint::ExternalUri(url) = target
                else {
                    return Err(FormatError::unsupported("html", format!("link target `{target:?}` has no HTML destination mapping")));
                };
                out.push_str(r#"<a href=""#);
                escape_attribute(out, url);
                out.push('"');
                write_optional_title(out, title.as_deref());
                out.push('>');
                write_inlines(out, display)?;
                out.push_str("</a>");
            }
            Inline::Image { source, alt, title, asset: _ } => {
                out.push_str(r#"<img alt=""#);
                escape_attribute(out, &alt.iter().map(inline_plain_text).collect::<String>());
                out.push_str(r#"" src=""#);
                escape_attribute(out, source);
                out.push('"');
                write_optional_title(out, title.as_deref());
                out.push('>');
            }
            Inline::Break { kind } => match kind {
                BreakKind::Hard => out.push_str("<br>"),
                BreakKind::Soft => out.push('\n'),
            },
            Inline::Attributed { attributes, children } => {
                out.push_str("<span");
                write_attributes(out, attributes)?;
                out.push('>');
                write_inlines(out, children)?;
                out.push_str("</span>");
            }
            Inline::Note { label, display } => {
                out.push_str(r#"<sup class="footnote-reference" data-footnote-label=""#);
                escape_attribute(out, label);
                out.push_str(r##""><a href="#fn-"##);
                escape_attribute(out, label);
                out.push_str("\">");
                match display {
                    Some(display) => write_inlines(out, display)?,
                    None => escape_text(out, label),
                }
                out.push_str("</a></sup>");
            }
            Inline::Citation { .. } => {
                return Err(FormatError::unsupported(
                    "html",
                    "inline semantic node is not representable by the conservative IR HTML exporter yet",
                ));
            }
            Inline::RawHtml { html } => out.push_str(html),
        }
    }
    Ok(())
}

fn write_attributes(out: &mut String, attributes: &Attributes) -> Result<(), FormatError> {
    if let Some(id) = &attributes.id {
        out.push_str(r#" id=""#);
        escape_attribute(out, id);
        out.push('"');
    }
    if !attributes.classes.is_empty() {
        out.push_str(r#" class=""#);
        escape_attribute(out, &attributes.classes.join(" "));
        out.push('"');
    }
    for pair in &attributes.pairs {
        if !is_valid_attribute_name(&pair.key) || pair.key.eq_ignore_ascii_case("id") || pair.key.eq_ignore_ascii_case("class") {
            return Err(FormatError::unsupported("html", format!("invalid attribute name `{}`", pair.key)));
        }
        out.push(' ');
        out.push_str(&pair.key);
        out.push_str(r#"=""#);
        escape_attribute(out, &pair.value);
        out.push('"');
    }
    Ok(())
}

fn is_valid_attribute_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn inline_plain_text(inline: &Inline) -> String {
    inline.plain_text()
}

/// Diagram source keeps its exact bytes through HTML, where attribute decoding
/// and whitespace normalization would otherwise rewrite the authored lines.
fn encode_hex(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for byte in text.as_bytes() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn escape_text(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

fn write_optional_title(out: &mut String, title: Option<&str>) {
    if let Some(title) = title {
        out.push_str(r#" title=""#);
        escape_attribute(out, title);
        out.push('"');
    }
}

fn escape_attribute(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
}

fn format_error(error: std::fmt::Error) -> FormatError {
    FormatError::unsupported("html", error.to_string())
}
