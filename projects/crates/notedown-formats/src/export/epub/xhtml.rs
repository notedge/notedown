//! XHTML chapter rendering for EPUB export.
//!
//! Own block/inline → XHTML emitter so the EPUB exporter stays self-contained
//! from `export::html`. Rendered shape matches the conservative HTML exporter
//! but wraps every chapter in a full XHTML document with the EPUB required
//! namespaces and `xml:lang`/`lang` pair.

use std::fmt::Write as _;

use notedown_ir::{
    Attributes, Block, BreakKind, DocumentGraph, Inline, ListItem, NodeId, RelationEndpoint, TableRow,
};

use super::chapter::{ChapterPlan, asset_source_map, plan_assets};
use crate::FormatError;

/// Render a single chapter as a full XHTML document.
pub fn chapter_xhtml(graph: &DocumentGraph, plan: &ChapterPlan) -> Result<String, FormatError> {
    let mut out = String::new();
    out.push_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml""#,
    );
    if let Some(lang) = graph.metadata.language.as_deref() {
        out.push_str(" xml:lang=\"");
        escape_attribute(&mut out, lang);
        out.push_str("\" lang=\"");
        escape_attribute(&mut out, lang);
        out.push('"');
    } else {
        out.push_str(" xml:lang=\"en\" lang=\"en\"");
    }
    out.push_str(
        r#">
  <head>
    <meta charset="UTF-8"/>
    <title>"#,
    );
    escape_text(&mut out, &plan.label);
    out.push_str(
        r#"</title>
  </head>
  <body>
"#,
    );

    // Emit each block owned by this chapter (only top-level ones in the plan).
    let assets = plan_assets(graph);
    let source_map = asset_source_map(&assets);
    let mut emitted = std::collections::HashSet::new();
    for node_id in &plan.blocks {
        if emitted.contains(node_id) {
            continue;
        }
        emitted.extend(nested_block_ids(graph, *node_id));
        let Some(node) = graph.block(*node_id) else {
            return Err(FormatError::invalid_input(format!(
                "chapter block `{node_id:?}` is missing from the graph"
            )));
        };
        write_node(&mut out, graph, &node.block, Some(&source_map))?;
    }

    out.push_str(
        r#"  </body>
</html>
"#,
    );
    Ok(out)
}

fn nested_block_ids(graph: &DocumentGraph, node_id: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let Some(node) = graph.block(node_id) else {
        return out;
    };
    match &node.block {
        Block::Section { children, .. } => {
            for child in children {
                out.push(*child);
                out.extend(nested_block_ids(graph, *child));
            }
        }
        Block::List { items, .. } => {
            for item in items {
                for child in &item.children {
                    out.push(*child);
                    out.extend(nested_block_ids(graph, *child));
                }
            }
        }
        _ => {}
    }
    out
}

fn write_node(
    out: &mut String,
    graph: &DocumentGraph,
    block: &Block,
    source_map: Option<&std::collections::HashMap<String, String>>,
) -> Result<(), FormatError> {
    write_block(out, graph, block, source_map)
}

fn write_block(
    out: &mut String,
    graph: &DocumentGraph,
    block: &Block,
    source_map: Option<&std::collections::HashMap<String, String>>,
) -> Result<(), FormatError> {
    match block {
        Block::Section { level, title, children } => {
            let level = (*level).clamp(1, 6);
            write!(out, "<h{}>", level).map_err(format_error)?;
            write_inlines(out, title, source_map)?;
            write!(out, "</h{}>\n", level).map_err(format_error)?;
            for child_id in children {
                let Some(child) = graph.block(*child_id) else {
                    return Err(FormatError::invalid_input(format!(
                        "section child `{child_id:?}` is missing"
                    )));
                };
                write_node(out, graph, &child.block, source_map)?;
            }
        }
        Block::Paragraph { content } => {
            out.push_str("<p>");
            write_inlines(out, content, source_map)?;
            out.push_str("</p>\n");
        }
        Block::Quote { content } => {
            out.push_str("<blockquote>");
            write_inlines(out, content, source_map)?;
            out.push_str("</blockquote>\n");
        }
        Block::Code { language, content } => {
            out.push_str("<pre><code");
            if let Some(language) = language {
                out.push_str(" class=\"language-");
                escape_attribute(out, language);
                out.push('"');
            }
            out.push('>');
            escape_text(out, content);
            out.push_str("</code></pre>\n");
        }
        Block::Diagram { language, source } => {
            // Partial: no raster conversion — emit as a marked-up pre.
            out.push_str("<pre data-diagram-language=\"");
            escape_attribute(out, language);
            out.push_str("\" data-diagram-source=\"");
            escape_attribute(out, source);
            out.push_str("\"><code>");
            escape_text(out, source);
            out.push_str("</code></pre>\n");
        }
        Block::Math { content, language } => {
            out.push_str("<div class=\"math\"");
            if let Some(language) = language {
                out.push_str(" data-language=\"");
                escape_attribute(out, language);
                out.push('"');
            }
            out.push('>');
            escape_text(out, content);
            out.push_str("</div>\n");
        }
        Block::List { ordered, items } => write_list(out, graph, *ordered, items, source_map)?,
        Block::Table { rows, .. } => write_table(out, rows, source_map)?,
        Block::Opaque { kind, payload_hint, .. } if kind == "thematic_break" => {
            out.push_str("<hr/>");
            if !payload_hint.is_empty() {
                out.push_str("<span class=\"opaque-hint\">");
                escape_text(out, payload_hint);
                out.push_str("</span>");
            }
            out.push('\n');
        }
        Block::Opaque { kind, .. } => {
            return Err(FormatError::unsupported(
                "epub",
                format!("opaque block kind `{kind}` is not representable"),
            ));
        }
    }
    Ok(())
}

fn write_list(
    out: &mut String,
    graph: &DocumentGraph,
    ordered: bool,
    items: &[ListItem],
    source_map: Option<&std::collections::HashMap<String, String>>,
) -> Result<(), FormatError> {
    out.push_str(if ordered { "<ol>\n" } else { "<ul>\n" });
    for item in items {
        out.push_str("<li>");
        write_inlines(out, &item.content, source_map)?;
        out.push('\n');
        for child_id in &item.children {
            let Some(child) = graph.block(*child_id) else {
                return Err(FormatError::invalid_input(format!(
                    "list item child `{child_id:?}` is missing"
                )));
            };
            write_node(out, graph, &child.block, source_map)?;
        }
        out.push_str("</li>\n");
    }
    out.push_str(if ordered { "</ol>\n" } else { "</ul>\n" });
    Ok(())
}

fn write_table(
    out: &mut String,
    rows: &[TableRow],
    source_map: Option<&std::collections::HashMap<String, String>>,
) -> Result<(), FormatError> {
    out.push_str("<table>\n");
    for row in rows {
        out.push_str("  <tr>\n");
        let tag = if row.is_header { "th" } else { "td" };
        for cell in &row.cells {
            out.push_str("    <");
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
            write_inlines(out, &cell.content, source_map)?;
            out.push_str("</");
            out.push_str(tag);
            out.push_str(">\n");
        }
        out.push_str("  </tr>\n");
    }
    out.push_str("</table>\n");
    Ok(())
}

fn write_inlines(
    out: &mut String,
    inlines: &[Inline],
    source_map: Option<&std::collections::HashMap<String, String>>,
) -> Result<(), FormatError> {
    for inline in inlines {
        match inline {
            Inline::Text { text } => escape_text(out, text),
            Inline::InlineCode { text } => {
                out.push_str("<code>");
                escape_text(out, text);
                out.push_str("</code>");
            }
            Inline::InlineMath { content, language } => {
                out.push_str("<span class=\"math\"");
                if let Some(language) = language {
                    out.push_str(" data-language=\"");
                    escape_attribute(out, language);
                    out.push('"');
                }
                out.push('>');
                escape_text(out, content);
                out.push_str("</span>");
            }
            Inline::Reference { display, target } => {
                write!(out, "<a href=\"#node-{}\">", target.0).map_err(format_error)?;
                escape_text(out, display);
                out.push_str("</a>");
            }
            Inline::Styled { style, children } => {
                if let Some(color) = style.strip_prefix("color:") {
                    if color.len() != 6 || !color.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                        return Err(FormatError::unsupported(
                            "epub",
                            format!("invalid color style `{style}`"),
                        ));
                    }
                    write!(out, "<span style=\"color:#{color}\">").map_err(format_error)?;
                    write_inlines(out, children, source_map)?;
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
                        return Err(FormatError::unsupported(
                            "epub",
                            format!("inline style `{style}` is not representable"),
                        ));
                    }
                };
                write!(out, "<{}>", tag).map_err(format_error)?;
                write_inlines(out, children, source_map)?;
                write!(out, "</{}>", tag).map_err(format_error)?;
            }
            Inline::Link { target, display, title } => {
                let RelationEndpoint::ExternalUri(url) = target
                else {
                    return Err(FormatError::unsupported(
                        "epub",
                        format!("link target `{target:?}` has no EPUB destination mapping"),
                    ));
                };
                out.push_str("<a href=\"");
                escape_attribute(out, url);
                out.push('"');
                write_optional_title(out, title.as_deref());
                out.push('>');
                write_inlines(out, display, source_map)?;
                out.push_str("</a>");
            }
            Inline::Image { source, alt, title, asset: _ } => {
                let href = rewrite_image_href(source, source_map);
                out.push_str("<img alt=\"");
                escape_attribute(out, &alt.iter().map(|i| i.plain_text()).collect::<String>());
                out.push_str("\" src=\"");
                escape_attribute(out, &href);
                out.push('"');
                write_optional_title(out, title.as_deref());
                out.push_str("/>");
            }
            Inline::Break { kind } => match kind {
                BreakKind::Hard => out.push_str("<br/>"),
                BreakKind::Soft => out.push('\n'),
            },
            Inline::Attributed { attributes, children } => {
                out.push_str("<span");
                write_attributes(out, attributes)?;
                out.push('>');
                write_inlines(out, children, source_map)?;
                out.push_str("</span>");
            }
            Inline::Note { label, display } => {
                out.push_str("<sup class=\"footnote-reference\"><a href=\"#fn-");
                escape_attribute(out, label);
                out.push_str("\">");
                match display {
                    Some(display) => write_inlines(out, display, source_map)?,
                    None => escape_text(out, label),
                }
                out.push_str("</a></sup>");
            }
            Inline::Citation { .. } => {
                return Err(FormatError::unsupported(
                    "epub",
                    "inline Citation is not representable by the conservative EPUB exporter",
                ));
            }
        }
    }
    Ok(())
}

fn rewrite_image_href(
    source: &str,
    source_map: Option<&std::collections::HashMap<String, String>>,
) -> String {
    let Some(map) = source_map else {
        return source.to_string();
    };
    // Try exact match, then basename match.
    if let Some(mapped) = map.get(source) {
        return mapped.clone();
    }
    let basename = source.rsplit('/').next().unwrap_or(source);
    if let Some(mapped) = map.get(basename) {
        return mapped.clone();
    }
    source.to_string()
}

fn write_attributes(out: &mut String, attributes: &Attributes) -> Result<(), FormatError> {
    if let Some(id) = &attributes.id {
        out.push_str(" id=\"");
        escape_attribute(out, id);
        out.push('"');
    }
    if !attributes.classes.is_empty() {
        out.push_str(" class=\"");
        escape_attribute(out, &attributes.classes.join(" "));
        out.push('"');
    }
    for pair in &attributes.pairs {
        if !is_valid_attribute_name(&pair.key)
            || pair.key.eq_ignore_ascii_case("id")
            || pair.key.eq_ignore_ascii_case("class")
        {
            return Err(FormatError::unsupported(
                "epub",
                format!("invalid attribute name `{}`", pair.key),
            ));
        }
        out.push(' ');
        out.push_str(&pair.key);
        out.push_str("=\"");
        escape_attribute(out, &pair.value);
        out.push('"');
    }
    Ok(())
}

fn is_valid_attribute_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

fn write_optional_title(out: &mut String, title: Option<&str>) {
    if let Some(title) = title {
        out.push_str(" title=\"");
        escape_attribute(out, title);
        out.push('"');
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
    FormatError::unsupported("epub", error.to_string())
}
