use std::fmt::Write as _;

use notedown_ir::{Block, DocumentGraph, Inline, ListItem};

use crate::FormatError;

use super::numbering::{ORDERED_NUM_ID, UNORDERED_NUM_ID};

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

pub fn render_document_xml(graph: &DocumentGraph) -> Result<(String, bool), FormatError> {
    let mut body = String::new();
    let mut uses_numbering = false;
    for node in &graph.blocks {
        if matches!(node.block, Block::List { .. }) {
            uses_numbering = true;
        }
        write_block(&mut body, &node.block)?;
    }
    body.push_str("<w:sectPr/>");

    Ok((
        format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{W_NS}">
  <w:body>
    {body}
  </w:body>
</w:document>"#
        ),
        uses_numbering,
    ))
}

fn write_block(out: &mut String, block: &Block) -> Result<(), FormatError> {
    match block {
        Block::Paragraph { content } => {
            out.push_str("<w:p>");
            write_inlines(out, content)?;
            out.push_str("</w:p>");
        }
        Block::Section { level, title, children: _ } => {
            let level = (*level).clamp(1, 6);
            write!(out, "<w:p><w:pPr><w:pStyle w:val=\"Heading{level}\"/></w:pPr>")
                .map_err(map_fmt_error)?;
            write_inlines(out, title)?;
            out.push_str("</w:p>");
        }
        Block::Code { content, language: _ } => {
            out.push_str("<w:p><w:r><w:t xml:space=\"preserve\">");
            write_xml_text(out, content)?;
            out.push_str("</w:t></w:r></w:p>");
        }
        Block::Quote { content } => {
            out.push_str("<w:p><w:pPr><w:pStyle w:val=\"Quote\"/></w:pPr>");
            write_inlines(out, content)?;
            out.push_str("</w:p>");
        }
        Block::List { ordered, items } => {
            write_list(out, *ordered, items)?;
        }
        Block::Table { .. }
        | Block::Math { .. }
        | Block::Opaque { .. } => {
            return Err(FormatError::unsupported(
                "docx",
                "block type is not supported by the conservative DOCX exporter yet",
            ));
        }
    }
    Ok(())
}

fn write_list(out: &mut String, ordered: bool, items: &[ListItem]) -> Result<(), FormatError> {
    let num_id = if ordered { ORDERED_NUM_ID } else { UNORDERED_NUM_ID };
    for item in items {
        write!(out, "<w:p><w:pPr><w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"{num_id}\"/></w:numPr></w:pPr>")
            .map_err(map_fmt_error)?;
        write_inlines(out, &item.content)?;
        out.push_str("</w:p>");
    }
    Ok(())
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
            out.push_str("<w:r><w:t>");
            write_xml_text(out, text)?;
            out.push_str("</w:t></w:r>");
        }
        Inline::Styled { style, children } => match style.as_str() {
            "bold" => {
                out.push_str("<w:r><w:rPr><w:b/></w:rPr>");
                write_styled_children(out, children)?;
                out.push_str("</w:r>");
            }
            "italic" => {
                out.push_str("<w:r><w:rPr><w:i/></w:rPr>");
                write_styled_children(out, children)?;
                out.push_str("</w:r>");
            }
            "link" => write_inlines(out, children)?,
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

fn write_styled_children(out: &mut String, children: &[Inline]) -> Result<(), FormatError> {
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
            other => write_inline(out, other)?,
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

fn map_fmt_error(error: std::fmt::Error) -> FormatError {
    FormatError::unsupported("docx", error.to_string())
}
