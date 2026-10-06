use std::fmt::Write as _;

use notedown_ir::{Block, DocumentGraph, Inline, ListItem, TableRow};

use crate::FormatError;

/// Export a semantic document graph as a standalone HTML fragment.
pub fn export_html(graph: &DocumentGraph) -> Result<String, FormatError> {
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
    for node in &graph.blocks {
        if nested.contains(&node.id) {
            continue;
        }
        write_node(&mut out, graph, node.id, &node.block)?;
    }
    out.push_str("</article>");
    Ok(out)
}

fn write_node(out: &mut String, graph: &DocumentGraph, id: notedown_ir::NodeId, block: &Block) -> Result<(), FormatError> {
    write!(out, r#"<div id="node-{}" data-node-id="{}">"#, id.0, id.0).map_err(|error| FormatError::unsupported("html", error.to_string()))?;
    write_block(out, graph, block)?;
    out.push_str("</div>");
    Ok(())
}

fn nested_block_ids(graph: &DocumentGraph) -> std::collections::HashSet<notedown_ir::NodeId> {
    let mut ids = std::collections::HashSet::new();
    for node in &graph.blocks {
        match &node.block {
            Block::Section { children, .. } => ids.extend(children.iter().copied()),
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

fn write_block(out: &mut String, graph: &DocumentGraph, block: &Block) -> Result<(), FormatError> {
    match block {
        Block::Section { level, title, children } => {
            let level = (*level).clamp(1, 6);
            write!(out, "<h{}>", level).map_err(format_error)?;
            write_inlines(out, title)?;
            write!(out, "</h{}>", level).map_err(format_error)?;
            for child_id in children {
                let child = graph.block(*child_id).ok_or_else(|| {
                    FormatError::invalid_input(format!("section child `{child_id:?}` is missing"))
                })?;
                write_node(out, graph, child.id, &child.block)?;
            }
        }
        Block::Paragraph { content } => {
            out.push_str("<p>");
            write_inlines(out, content)?;
            out.push_str("</p>");
        }
        Block::Quote { content } => {
            out.push_str("<blockquote>");
            write_inlines(out, content)?;
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
        Block::Math { content, language } => {
            out.push_str(r#"<div class="math""#);
            if let Some(language) = language {
                out.push_str(r#" data-language=""#);
                escape_attribute(out, language);
                out.push('"');
            }
            out.push('>');
            escape_text(out, content);
            out.push_str("</div>");
        }
        Block::List { ordered, items } => write_list(out, graph, *ordered, items)?,
        Block::Table { rows } => write_table(out, rows)?,
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
    }
    Ok(())
}

fn write_list(out: &mut String, graph: &DocumentGraph, ordered: bool, items: &[ListItem]) -> Result<(), FormatError> {
    out.push_str(if ordered { "<ol>" } else { "<ul>" });
    for item in items {
        out.push_str("<li>");
        write_inlines(out, &item.content)?;
        for child_id in &item.children {
            let child = graph.block(*child_id).ok_or_else(|| {
                FormatError::invalid_input(format!("list child `{child_id:?}` is missing"))
            })?;
            write_node(out, graph, child.id, &child.block)?;
        }
        out.push_str("</li>");
    }
    out.push_str(if ordered { "</ol>" } else { "</ul>" });
    Ok(())
}

fn write_table(out: &mut String, rows: &[TableRow]) -> Result<(), FormatError> {
    out.push_str("<table><tbody>");
    for row in rows {
        out.push_str("<tr>");
        for cell in &row.cells {
            out.push_str("<td>");
            write_inlines(out, cell)?;
            out.push_str("</td>");
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table>");
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
            Inline::InlineMath { content, language } => {
                out.push_str(r#"<span class="math""#);
                if let Some(language) = language {
                    out.push_str(r#" data-language=""#);
                    escape_attribute(out, language);
                    out.push('"');
                }
                out.push('>');
                escape_text(out, content);
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
                    "link" if children.len() >= 2 => {
                        let display = inline_plain_text(&children[0]);
                        let url = inline_plain_text(&children[1]);
                        out.push_str(r#"<a href=""#);
                        escape_attribute(out, &url);
                        out.push_str(r#"">"#);
                        escape_text(out, &display);
                        out.push_str("</a>");
                        continue;
                    }
                    "image" if children.len() >= 2 => {
                        let alt = inline_plain_text(&children[0]);
                        let src = inline_plain_text(&children[1]);
                        out.push_str(r#"<img alt=""#);
                        escape_attribute(out, &alt);
                        out.push_str(r#"" src=""#);
                        escape_attribute(out, &src);
                        out.push_str(r#"">"#);
                        continue;
                    }
                    _ => {
                        return Err(FormatError::unsupported("html", format!("inline style `{style}` is not representable")));
                    }
                };
                write!(out, "<{}>", tag).map_err(format_error)?;
                write_inlines(out, children)?;
                write!(out, "</{}>", tag).map_err(format_error)?;
            }
        }
    }
    Ok(())
}

fn inline_plain_text(inline: &Inline) -> String {
    match inline {
        Inline::Text { text } | Inline::InlineCode { text } => text.clone(),
        Inline::Styled { children, .. } => children.iter().map(inline_plain_text).collect(),
        Inline::InlineMath { content, .. } => content.clone(),
        Inline::Reference { display, .. } => display.clone(),
    }
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
