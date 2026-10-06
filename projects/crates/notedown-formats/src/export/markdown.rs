use std::{collections::{HashMap, HashSet}, fmt::Write as _};

use notedown_ir::{Block, DocumentGraph, Inline, ListItem, NodeId, TableRow};

use crate::FormatError;

/// Export `notedown-ir` to Markdown text.
pub fn export_markdown(graph: &DocumentGraph) -> Result<String, FormatError> {
    export_markdown_with_image_urls(graph, None)
}

/// Export Markdown while rewriting inline image destinations through `image_urls`.
pub fn export_markdown_with_image_urls(
    graph: &DocumentGraph,
    image_urls: Option<&HashMap<String, String>>,
) -> Result<String, FormatError> {
    export_markdown_with_image_urls_for_roots(graph, None, image_urls)
}

/// Export Markdown for selected top-level block roots.
pub fn export_markdown_with_image_urls_for_roots(
    graph: &DocumentGraph,
    roots: Option<&[NodeId]>,
    image_urls: Option<&HashMap<String, String>>,
) -> Result<String, FormatError> {
    let validation = graph.validate();
    if !validation.is_valid() {
        return Err(FormatError::invalid_input(format!("invalid document graph: {:?}", validation.issues)));
    }

    let root_filter = roots.map(|items| items.iter().copied().collect::<HashSet<_>>());
    let mut output = String::new();
    let mut footnotes = Vec::new();
    let nested = nested_block_ids(graph);
    for node in &graph.blocks {
        if nested.contains(&node.id) {
            continue;
        }
        if let Some(filter) = &root_filter {
            if !filter.contains(&node.id) {
                continue;
            }
        }
        if let Block::Opaque { kind, payload_hint, .. } = &node.block {
            if kind == "footnote_definition" {
                footnotes.push(payload_hint.as_str());
                continue;
            }
        }
        write_block(&mut output, graph, &node.block, image_urls)?;
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

fn write_block(
    out: &mut String,
    graph: &DocumentGraph,
    block: &Block,
    image_urls: Option<&HashMap<String, String>>,
) -> Result<(), FormatError> {
    match block {
        Block::Section { level, title, children } => {
            let level = (*level).clamp(1, 6);
            for _ in 0..level {
                out.push('#');
            }
            out.push(' ');
            write_inlines(out, title, image_urls)?;
            out.push_str("\n\n");
            for child_id in children {
                if let Some(child) = graph.block(*child_id) {
                    write_block(out, graph, &child.block, image_urls)?;
                }
            }
        }
        Block::Paragraph { content } => {
            write_inlines(out, content, image_urls)?;
            out.push_str("\n\n");
        }
        Block::Code { language, content } => {
            let fence = "`".repeat(longest_backtick_run(content).max(2) + 1);
            out.push_str(&fence);
            if let Some(language) = language {
                out.push_str(language);
            }
            out.push('\n');
            out.push_str(content);
            out.push('\n');
            out.push_str(&fence);
            out.push_str("\n\n");
        }
        Block::Quote { content } => {
            let mut line = String::new();
            write_inlines(&mut line, content, image_urls)?;
            for part in line.lines() {
                out.push_str("> ");
                out.push_str(part);
                out.push('\n');
            }
            out.push('\n');
        }
        Block::List { ordered, items } => {
            write_list(out, graph, *ordered, items, 0, image_urls)?;
            out.push('\n');
        }
        Block::Table { rows } => {
            write_table(out, rows)?;
            out.push('\n');
        }
        Block::Math { content, language } => {
            ensure_latex_math(content, language.as_deref())?;
            out.push_str("$$");
            out.push_str(content);
            out.push_str("$$\n\n");
        }
        Block::Opaque { kind, payload_hint, .. } => {
            if kind == "thematic_break" {
                out.push_str(payload_hint);
                out.push_str("\n\n");
            }
            else {
                return Err(FormatError::unsupported(
                    "markdown",
                    format!("opaque block kind `{kind}` is not supported by the IR markdown exporter yet"),
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
    image_urls: Option<&HashMap<String, String>>,
) -> Result<(), FormatError> {
    let prefix = " ".repeat(indent);
    for (index, item) in items.iter().enumerate() {
        out.push_str(&prefix);
        if ordered {
            write!(out, "{}. ", index + 1).map_err(|error| FormatError::unsupported("markdown", error.to_string()))?;
        }
        else {
            out.push_str("- ");
        }
        write_inlines(out, &item.content, image_urls)?;
        out.push('\n');
        for child_id in &item.children {
            if let Some(child) = graph.block(*child_id) {
                if let Block::List { ordered: nested_ordered, items: nested_items } = &child.block {
                    write_list(out, graph, *nested_ordered, nested_items, indent + 2, image_urls)?;
                }
                else {
                    return Err(FormatError::unsupported(
                        "markdown",
                        format!("nested list child `{child_id:?}` is not representable yet"),
                    ));
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
    let text = inlines.iter().map(inline_plain_text).collect::<String>();
    write_markdown_text(out, &text.replace('\n', " "));
    Ok(())
}

fn write_markdown_image(
    out: &mut String,
    children: &[Inline],
    image_urls: Option<&HashMap<String, String>>,
) -> Result<(), FormatError> {
    if children.len() >= 2 {
        let alt = inline_plain_text(&children[0]);
        let source = inline_plain_text(&children[1]);
        let url = image_urls
            .and_then(|map| map.get(&source))
            .map(String::as_str)
            .unwrap_or(source.as_str());
        out.push_str("![");
        write_markdown_text(out, &alt);
        out.push_str("](");
        write_markdown_destination(out, url);
        out.push(')');
        return Ok(());
    }
    write_inlines(out, children, image_urls)
}

fn write_markdown_link(out: &mut String, children: &[Inline], image_urls: Option<&HashMap<String, String>>) -> Result<(), FormatError> {
    if children.len() >= 2 {
        let display = inline_plain_text(&children[0]);
        let url = inline_plain_text(&children[1]);
        out.push('[');
        write_markdown_text(out, &display);
        out.push_str("](");
        write_markdown_destination(out, &url);
        out.push(')');
        return Ok(());
    }
    write_inlines(out, children, image_urls)
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

fn write_inlines(out: &mut String, inlines: &[Inline], image_urls: Option<&HashMap<String, String>>) -> Result<(), FormatError> {
    for inline in inlines {
        write_inline(out, inline, image_urls)?;
    }
    Ok(())
}

fn write_inline(out: &mut String, inline: &Inline, image_urls: Option<&HashMap<String, String>>) -> Result<(), FormatError> {
    match inline {
        Inline::Text { text } => {
            write_markdown_text(out, text);
        }
        Inline::InlineCode { text } => {
            let delimiter = "`".repeat(longest_backtick_run(text) + 1);
            let needs_padding =
                text.starts_with('`') || text.ends_with('`') || (text.starts_with(' ') && text.ends_with(' ') && !text.trim().is_empty());
            out.push_str(&delimiter);
            if needs_padding {
                out.push(' ');
            }
            out.push_str(text);
            if needs_padding {
                out.push(' ');
            }
            out.push_str(&delimiter);
        }
        Inline::Styled { style, children } => {
            if style == "link" {
                return write_markdown_link(out, children, image_urls);
            }
            if style == "image" {
                return write_markdown_image(out, children, image_urls);
            }
            if style == "footnote_reference" {
                out.push_str("[^");
                for child in children {
                    write_inlines(out, std::slice::from_ref(child), image_urls)?;
                }
                out.push(']');
                return Ok(());
            }
            let wrapper = match style.as_str() {
                "bold" | "strong" => ("**", "**"),
                "italic" | "emphasis" => ("*", "*"),
                "strike" | "strikethrough" => ("~~", "~~"),
                _ => {
                    return Err(FormatError::unsupported("markdown", format!("inline style `{style}` is not representable")));
                }
            };
            out.push_str(wrapper.0);
            write_inlines(out, children, image_urls)?;
            out.push_str(wrapper.1);
        }
        Inline::InlineMath { content, language } => {
            ensure_latex_math(content, language.as_deref())?;
            out.push('$');
            out.push_str(content);
            out.push('$');
        }
        Inline::Reference { target, .. } => {
            return Err(FormatError::unsupported("markdown", format!("semantic reference target `{target:?}` has no stable Markdown anchor mapping")));
        }
    }
    Ok(())
}

fn write_markdown_text(out: &mut String, text: &str) {
    for character in text.chars() {
        if matches!(character, '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '<' | '>' | '!' | '|' | '#' | '+' | '-') {
            out.push('\\');
        }
        out.push(character);
    }
}

fn ensure_latex_math(content: &str, language: Option<&str>) -> Result<(), FormatError> {
    if language.is_some_and(|language| language != "latex") || content.contains('$') {
        return Err(FormatError::unsupported("markdown", "math requires LaTeX content without dollar delimiters"));
    }
    Ok(())
}

fn write_markdown_destination(out: &mut String, destination: &str) {
    for character in destination.chars() {
        if matches!(character, '\\' | '(' | ')') {
            out.push('\\');
        }
        out.push(character);
    }
}

fn longest_backtick_run(text: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for character in text.chars() {
        if character == '`' {
            current += 1;
            longest = longest.max(current);
        }
        else {
            current = 0;
        }
    }
    longest
}
