use std::collections::HashSet;
use std::fmt::Write as _;

use notedown_ir::{Block, DocumentGraph, Inline, ListItem, NodeId, TableRow};

use crate::FormatError;

/// Export `notedown-ir` to Markdown text.
pub fn export_markdown(graph: &DocumentGraph) -> Result<String, FormatError> {
    let mut output = String::new();
    let mut footnotes = Vec::new();
    let nested = nested_block_ids(graph);
    for node in &graph.blocks {
        if nested.contains(&node.id) {
            continue;
        }
        if let Block::Opaque {
            kind,
            payload_hint,
            ..
        } = &node.block
        {
            if kind == "footnote_definition" {
                footnotes.push(payload_hint.as_str());
                continue;
            }
        }
        write_block(&mut output, graph, &node.block)?;
    }
    for footnote in footnotes {
        output.push_str(footnote);
        output.push_str("\n\n");
    }
    Ok(output)
}

fn nested_block_ids(graph: &DocumentGraph) -> HashSet<NodeId> {
    let mut ids = HashSet::new();
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
        Block::Section { level, title, children: _ } => {
            let level = (*level).clamp(1, 6);
            for _ in 0..level {
                out.push('#');
            }
            out.push(' ');
            write_inlines(out, title)?;
            out.push_str("\n\n");
        }
        Block::Paragraph { content } => {
            write_inlines(out, content)?;
            out.push_str("\n\n");
        }
        Block::Code { language, content } => {
            out.push_str("```");
            if let Some(language) = language {
                out.push_str(language);
            }
            out.push('\n');
            out.push_str(content);
            out.push_str("\n```\n\n");
        }
        Block::Quote { content } => {
            let mut line = String::new();
            write_inlines(&mut line, content)?;
            for part in line.lines() {
                out.push_str("> ");
                out.push_str(part);
                out.push('\n');
            }
            out.push('\n');
        }
        Block::List { ordered, items } => {
            write_list(out, graph, *ordered, items, 0)?;
            out.push('\n');
        }
        Block::Table { rows } => {
            write_table(out, rows)?;
            out.push('\n');
        }
        Block::Math { .. } => {
            return Err(FormatError::unsupported(
                "markdown",
                "block type is not supported by the IR markdown exporter yet",
            ));
        }
        Block::Opaque { kind, payload_hint, .. } => {
            if kind == "thematic_break" {
                out.push_str(payload_hint);
                out.push_str("\n\n");
            } else {
                return Err(FormatError::unsupported(
                    "markdown",
                    format!(
                        "opaque block kind `{kind}` is not supported by the IR markdown exporter yet"
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn write_list(
    out: &mut String,
    graph: &DocumentGraph,
    ordered: bool,
    items: &[ListItem],
    indent: usize,
) -> Result<(), FormatError> {
    let prefix = " ".repeat(indent);
    for (index, item) in items.iter().enumerate() {
        out.push_str(&prefix);
        if ordered {
            write!(out, "{}. ", index + 1).map_err(|error| {
                FormatError::unsupported("markdown", error.to_string())
            })?;
        } else {
            out.push_str("- ");
        }
        write_inlines(out, &item.content)?;
        out.push('\n');
        for child_id in &item.children {
            if let Some(child) = graph.block(*child_id) {
                if let Block::List {
                    ordered: nested_ordered,
                    items: nested_items,
                } = &child.block
                {
                    write_list(out, graph, *nested_ordered, nested_items, indent + 2)?;
                }
            }
        }
    }
    Ok(())
}

fn write_table(out: &mut String, rows: &[TableRow]) -> Result<(), FormatError> {
    if rows.is_empty() {
        return Ok(());
    }
    for (index, row) in rows.iter().enumerate() {
        out.push('|');
        for cell in &row.cells {
            out.push(' ');
            write_table_cell(out, cell)?;
            out.push_str(" |");
        }
        out.push('\n');
        if index == 0 {
            out.push('|');
            for _ in &row.cells {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    Ok(())
}

fn write_table_cell(out: &mut String, inlines: &[Inline]) -> Result<(), FormatError> {
    let text = inlines
        .iter()
        .map(inline_plain_text)
        .collect::<String>()
        .replace('|', "\\|")
        .replace('\n', " ");
    out.push_str(&text);
    Ok(())
}

fn write_markdown_image(out: &mut String, children: &[Inline]) -> Result<(), FormatError> {
    if children.len() >= 2 {
        let alt = inline_plain_text(&children[0]);
        let url = inline_plain_text(&children[1]);
        out.push_str("![");
        out.push_str(&alt);
        out.push_str("](");
        out.push_str(&url);
        out.push(')');
        return Ok(());
    }
    write_inlines(out, children)
}

fn write_markdown_link(out: &mut String, children: &[Inline]) -> Result<(), FormatError> {
    if children.len() >= 2 {
        let display = inline_plain_text(&children[0]);
        let url = inline_plain_text(&children[1]);
        out.push('[');
        out.push_str(&display);
        out.push_str("](");
        out.push_str(&url);
        out.push(')');
        return Ok(());
    }
    write_inlines(out, children)
}

fn inline_plain_text(inline: &Inline) -> String {
    match inline {
        Inline::Text { text } => text.clone(),
        Inline::InlineCode { text } => text.clone(),
        Inline::Styled { children, .. } => children.iter().map(inline_plain_text).collect(),
        Inline::InlineMath { content, .. } => content.clone(),
        Inline::Reference { display, .. } => display.clone(),
    }
}

fn write_inlines(out: &mut String, inlines: &[Inline]) -> Result<(), FormatError> {
    for inline in inlines {
        write_inline(out, inline)?;
    }
    Ok(())
}

fn write_inline(out: &mut String, inline: &Inline) -> Result<(), FormatError> {
    match inline {
        Inline::Text { text } => {
            out.push_str(text);
        }
        Inline::InlineCode { text } => {
            out.push('`');
            out.push_str(text);
            out.push('`');
        }
        Inline::Styled { style, children } => {
            if style == "link" {
                return write_markdown_link(out, children);
            }
            if style == "image" {
                return write_markdown_image(out, children);
            }
            let wrapper = match style.as_str() {
                "bold" | "strong" => ("**", "**"),
                "italic" | "emphasis" | "figcaption" => ("*", "*"),
                _ => ("", ""),
            };
            out.push_str(wrapper.0);
            write_inlines(out, children)?;
            out.push_str(wrapper.1);
        }
        Inline::InlineMath { content, .. } => {
            out.push('$');
            out.push_str(content);
            out.push('$');
        }
        Inline::Reference { display, .. } => {
            out.push_str(display);
        }
    }
    Ok(())
}
