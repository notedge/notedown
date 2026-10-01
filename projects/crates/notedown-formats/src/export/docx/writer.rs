use std::fmt::Write as _;

use notedown_ir::{Block, DocumentGraph, Inline, ListItem, TableRow};

use crate::FormatError;

use super::numbering::{ORDERED_NUM_ID, UNORDERED_NUM_ID};
use super::rels::HyperlinkRegistry;

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

pub fn render_document_xml(graph: &DocumentGraph) -> Result<(String, bool, String), FormatError> {
    let mut body = String::new();
    let mut uses_numbering = false;
    let mut hyperlinks = HyperlinkRegistry::default();
    for node in &graph.blocks {
        if matches!(node.block, Block::List { .. }) {
            uses_numbering = true;
        }
        write_block(&mut body, &node.block, &mut hyperlinks)?;
    }
    body.push_str("<w:sectPr/>");

    Ok((
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{W_NS}" xmlns:r="{R_NS}">
  <w:body>
    {body}
  </w:body>
</w:document>"#
        ),
        uses_numbering,
        hyperlinks.render_document_rels_xml(),
    ))
}

fn write_block(
    out: &mut String,
    block: &Block,
    hyperlinks: &mut HyperlinkRegistry,
) -> Result<(), FormatError> {
    match block {
        Block::Paragraph { content } => {
            out.push_str("<w:p>");
            write_inlines(out, content, hyperlinks)?;
            out.push_str("</w:p>");
        }
        Block::Section { level, title, children: _ } => {
            let level = (*level).clamp(1, 6);
            write!(out, "<w:p><w:pPr><w:pStyle w:val=\"Heading{level}\"/></w:pPr>")
                .map_err(map_fmt_error)?;
            write_inlines(out, title, hyperlinks)?;
            out.push_str("</w:p>");
        }
        Block::Code { content, language: _ } => {
            out.push_str("<w:p><w:r><w:t xml:space=\"preserve\">");
            write_xml_text(out, content)?;
            out.push_str("</w:t></w:r></w:p>");
        }
        Block::Quote { content } => {
            out.push_str("<w:p><w:pPr><w:pStyle w:val=\"Quote\"/></w:pPr>");
            write_inlines(out, content, hyperlinks)?;
            out.push_str("</w:p>");
        }
        Block::List { ordered, items } => {
            write_list(out, *ordered, items, hyperlinks)?;
        }
        Block::Table { rows } => {
            write_table(out, rows, hyperlinks)?;
        }
        Block::Math { .. } | Block::Opaque { .. } => {
            return Err(FormatError::unsupported(
                "docx",
                "block type is not supported by the conservative DOCX exporter yet",
            ));
        }
    }
    Ok(())
}

fn write_list(
    out: &mut String,
    ordered: bool,
    items: &[ListItem],
    hyperlinks: &mut HyperlinkRegistry,
) -> Result<(), FormatError> {
    let num_id = if ordered { ORDERED_NUM_ID } else { UNORDERED_NUM_ID };
    for item in items {
        write!(out, "<w:p><w:pPr><w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"{num_id}\"/></w:numPr></w:pPr>")
            .map_err(map_fmt_error)?;
        write_inlines(out, &item.content, hyperlinks)?;
        out.push_str("</w:p>");
    }
    Ok(())
}

fn write_table(
    out: &mut String,
    rows: &[TableRow],
    hyperlinks: &mut HyperlinkRegistry,
) -> Result<(), FormatError> {
    if rows.is_empty() {
        return Ok(());
    }
    out.push_str("<w:tbl>");
    for row in rows {
        out.push_str("<w:tr>");
        for cell in &row.cells {
            out.push_str("<w:tc><w:p>");
            write_inlines(out, cell, hyperlinks)?;
            out.push_str("</w:p></w:tc>");
        }
        out.push_str("</w:tr>");
    }
    out.push_str("</w:tbl>");
    Ok(())
}

fn write_inlines(
    out: &mut String,
    inlines: &[Inline],
    hyperlinks: &mut HyperlinkRegistry,
) -> Result<(), FormatError> {
    for inline in inlines {
        write_inline(out, inline, hyperlinks)?;
    }
    Ok(())
}

fn write_inline(
    out: &mut String,
    inline: &Inline,
    hyperlinks: &mut HyperlinkRegistry,
) -> Result<(), FormatError> {
    match inline {
        Inline::Text { text } => {
            out.push_str("<w:r><w:t>");
            write_xml_text(out, text)?;
            out.push_str("</w:t></w:r>");
        }
        Inline::Styled { style, children } => match style.as_str() {
            "bold" => {
                out.push_str("<w:r><w:rPr><w:b/></w:rPr>");
                write_styled_children(out, children, hyperlinks)?;
                out.push_str("</w:r>");
            }
            "italic" => {
                out.push_str("<w:r><w:rPr><w:i/></w:rPr>");
                write_styled_children(out, children, hyperlinks)?;
                out.push_str("</w:r>");
            }
            "link" => {
                if let Some((display, url)) = split_link_children(children) {
                    let rel_id = hyperlinks.id_for(&url);
                    write!(out, "<w:hyperlink r:id=\"{rel_id}\">").map_err(map_fmt_error)?;
                    write_inlines(
                        out,
                        &[Inline::Text { text: display }],
                        hyperlinks,
                    )?;
                    out.push_str("</w:hyperlink>");
                } else {
                    write_inlines(out, children, hyperlinks)?;
                }
            }
            other => {
                return Err(FormatError::unsupported(
                    "docx",
                    format!("inline style `{other}` is not supported by the conservative DOCX exporter yet"),
                ));
            }
        },
        Inline::InlineCode { text } => {
            out.push_str("<w:r><w:rPr><w:rStyle w:val=\"VerbatimChar\"/></w:rPr><w:t>");
            write_xml_text(out, text)?;
            out.push_str("</w:t></w:r>");
        }
        Inline::InlineMath { .. } | Inline::Reference { .. } => {
            return Err(FormatError::unsupported(
                "docx",
                "inline type is not supported by the conservative DOCX exporter yet",
            ));
        }
    }
    Ok(())
}

fn write_styled_children(
    out: &mut String,
    children: &[Inline],
    hyperlinks: &mut HyperlinkRegistry,
) -> Result<(), FormatError> {
    if children.is_empty() {
        out.push_str("<w:t></w:t>");
        return Ok(());
    }
    for child in children {
        match child {
            Inline::Text { text } => {
                out.push_str("<w:t>");
                write_xml_text(out, text)?;
                out.push_str("</w:t>");
            }
            other => write_inline(out, other, hyperlinks)?,
        }
    }
    Ok(())
}

fn write_xml_text(out: &mut String, text: &str) -> Result<(), FormatError> {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    Ok(())
}

fn split_link_children(children: &[Inline]) -> Option<(String, String)> {
    if children.len() < 2 {
        return None;
    }
    let display = inline_plain_text(&children[0]);
    let url = inline_plain_text(&children[1]);
    if display.is_empty() || url.is_empty() {
        return None;
    }
    Some((display, url))
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

fn map_fmt_error(error: std::fmt::Error) -> FormatError {
    FormatError::unsupported("docx", error.to_string())
}
