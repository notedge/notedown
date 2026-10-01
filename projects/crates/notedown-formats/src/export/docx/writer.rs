use std::fmt::Write as _;

use notedown_ir::{AssetKind, Block, DocumentGraph, Inline, ListItem, TableRow};

use crate::FormatError;

use super::footnotes::{collect_footnote_catalog, parse_footnote_reference_token, render_footnotes_xml};
use super::numbering::{ORDERED_NUM_ID, UNORDERED_NUM_ID};
use super::rels::DocumentRelsRegistry;

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const WP_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";

/// Rendered WordprocessingML body and collected OPC relationships.
pub struct DocxRenderOutput {
    pub document_xml: String,
    pub include_numbering: bool,
    pub document_rels_xml: String,
    pub media_parts: Vec<(String, Vec<u8>)>,
    pub image_extensions: Vec<String>,
    pub footnotes_xml: Option<String>,
}

pub fn render_document_xml(graph: &DocumentGraph) -> Result<DocxRenderOutput, FormatError> {
    let footnotes = collect_footnote_catalog(graph);
    let mut body = String::new();
    let mut uses_numbering = false;
    let mut rels = DocumentRelsRegistry::default();
    for node in &graph.blocks {
        if let Block::Opaque {
            kind,
            ..
        } = &node.block
        {
            if kind == "footnote_definition" {
                continue;
            }
        }
        if matches!(node.block, Block::List { .. }) {
            uses_numbering = true;
        }
        write_block(&mut body, &node.block, graph, &mut rels)?;
    }
    body.push_str("<w:sectPr/>");

    let footnotes_xml = if footnotes.is_empty() {
        None
    } else {
        rels.ensure_footnotes_rel();
        Some(render_footnotes_xml(&footnotes)?)
    };

    Ok(DocxRenderOutput {
        document_xml: format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{W_NS}" xmlns:r="{R_NS}">
  <w:body>
    {body}
  </w:body>
</w:document>"#
        ),
        include_numbering: uses_numbering,
        document_rels_xml: rels.render_document_rels_xml(),
        media_parts: rels.media_parts(),
        image_extensions: rels.image_extensions(),
        footnotes_xml,
    })
}

fn write_block(
    out: &mut String,
    block: &Block,
    graph: &DocumentGraph,
    rels: &mut DocumentRelsRegistry,
) -> Result<(), FormatError> {
    match block {
        Block::Paragraph { content } => {
            out.push_str("<w:p>");
            write_inlines(out, content, graph, rels)?;
            out.push_str("</w:p>");
        }
        Block::Section { level, title, children: _ } => {
            let level = (*level).clamp(1, 6);
            write!(out, "<w:p><w:pPr><w:pStyle w:val=\"Heading{level}\"/></w:pPr>")
                .map_err(map_fmt_error)?;
            write_inlines(out, title, graph, rels)?;
            out.push_str("</w:p>");
        }
        Block::Code { content, language: _ } => {
            out.push_str("<w:p><w:r><w:t xml:space=\"preserve\">");
            write_xml_text(out, content)?;
            out.push_str("</w:t></w:r></w:p>");
        }
        Block::Quote { content } => {
            out.push_str("<w:p><w:pPr><w:pStyle w:val=\"Quote\"/></w:pPr>");
            write_inlines(out, content, graph, rels)?;
            out.push_str("</w:p>");
        }
        Block::List { ordered, items } => {
            write_list(out, *ordered, items, graph, rels)?;
        }
        Block::Table { rows } => {
            write_table(out, rows, graph, rels)?;
        }
        Block::Math { .. } => {
            return Err(FormatError::unsupported(
                "docx",
                "block type is not supported by the conservative DOCX exporter yet",
            ));
        }
        Block::Opaque { kind, .. } => {
            return Err(FormatError::unsupported(
                "docx",
                format!("opaque block kind `{kind}` is not supported by the conservative DOCX exporter yet"),
            ));
        }
    }
    Ok(())
}

fn write_list(
    out: &mut String,
    ordered: bool,
    items: &[ListItem],
    graph: &DocumentGraph,
    rels: &mut DocumentRelsRegistry,
) -> Result<(), FormatError> {
    let num_id = if ordered { ORDERED_NUM_ID } else { UNORDERED_NUM_ID };
    for item in items {
        write!(out, "<w:p><w:pPr><w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"{num_id}\"/></w:numPr></w:pPr>")
            .map_err(map_fmt_error)?;
        write_inlines(out, &item.content, graph, rels)?;
        out.push_str("</w:p>");
    }
    Ok(())
}

fn write_table(
    out: &mut String,
    rows: &[TableRow],
    graph: &DocumentGraph,
    rels: &mut DocumentRelsRegistry,
) -> Result<(), FormatError> {
    if rows.is_empty() {
        return Ok(());
    }
    out.push_str("<w:tbl>");
    for row in rows {
        out.push_str("<w:tr>");
        for cell in &row.cells {
            out.push_str("<w:tc><w:p>");
            write_inlines(out, cell, graph, rels)?;
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
    graph: &DocumentGraph,
    rels: &mut DocumentRelsRegistry,
) -> Result<(), FormatError> {
    for inline in inlines {
        write_inline(out, inline, graph, rels)?;
    }
    Ok(())
}

fn write_inline(
    out: &mut String,
    inline: &Inline,
    graph: &DocumentGraph,
    rels: &mut DocumentRelsRegistry,
) -> Result<(), FormatError> {
    match inline {
        Inline::Text { text } => {
            write_text_with_footnotes(out, text)?;
        }
        Inline::Styled { style, children } => match style.as_str() {
            "bold" => {
                out.push_str("<w:r><w:rPr><w:b/></w:rPr>");
                write_styled_children(out, children, graph, rels)?;
                out.push_str("</w:r>");
            }
            "italic" => {
                out.push_str("<w:r><w:rPr><w:i/></w:rPr>");
                write_styled_children(out, children, graph, rels)?;
                out.push_str("</w:r>");
            }
            "link" => {
                if let Some((display, url)) = split_link_children(children) {
                    let rel_id = rels.id_for_hyperlink(&url);
                    write!(out, "<w:hyperlink r:id=\"{rel_id}\">").map_err(map_fmt_error)?;
                    write_inlines(
                        out,
                        &[Inline::Text { text: display }],
                        graph,
                        rels,
                    )?;
                    out.push_str("</w:hyperlink>");
                } else {
                    write_inlines(out, children, graph, rels)?;
                }
            }
            "image" => {
                if let Some((alt, target)) = split_image_children(children) {
                    let bytes = image_bytes_for_target(graph, &target)?;
                    let rel_id = rels.id_for_image(&target, bytes);
                    write_image_drawing(out, &rel_id, &alt)?;
                } else {
                    return Err(FormatError::unsupported(
                        "docx",
                        "image inline is missing alt text or target path",
                    ));
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

fn write_image_drawing(out: &mut String, rel_id: &str, alt: &str) -> Result<(), FormatError> {
    write!(
        out,
        r#"<w:r><w:drawing><wp:inline xmlns:wp="{WP_NS}" xmlns:a="{A_NS}"><wp:docPr descr="{descr}"/><a:graphic><a:graphicData><a:blip r:embed="{rel_id}"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#,
        descr = escape_xml_attr(alt),
        rel_id = rel_id,
    )
    .map_err(map_fmt_error)?;
    Ok(())
}

fn write_styled_children(
    out: &mut String,
    children: &[Inline],
    graph: &DocumentGraph,
    rels: &mut DocumentRelsRegistry,
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
            other => write_inline(out, other, graph, rels)?,
        }
    }
    Ok(())
}

fn image_bytes_for_target(graph: &DocumentGraph, target: &str) -> Result<Vec<u8>, FormatError> {
    graph
        .assets
        .iter()
        .find(|asset| asset.kind == AssetKind::Image && asset.source.as_deref() == Some(target))
        .and_then(|asset| asset.bytes.clone())
        .ok_or_else(|| {
            FormatError::unsupported(
                "docx",
                format!("embedded image `{target}` has no materialized bytes for export"),
            )
        })
}

fn write_text_with_footnotes(out: &mut String, text: &str) -> Result<(), FormatError> {
    if let Some(id) = parse_footnote_reference_token(text) {
        return write_footnote_reference(out, id);
    }

    let mut rest = text;
    while !rest.is_empty() {
        if let Some(start) = rest.find("[^") {
            if start > 0 {
                write_run_text(out, &rest[..start])?;
            }
            let token_start = &rest[start..];
            if let Some(end) = token_start.find(']') {
                let token = &token_start[..=end];
                if let Some(id) = parse_footnote_reference_token(token) {
                    write_footnote_reference(out, id)?;
                    rest = &token_start[end + 1..];
                    continue;
                }
            }
            write_run_text(out, rest)?;
            break;
        } else {
            write_run_text(out, rest)?;
            break;
        }
    }
    Ok(())
}

fn write_run_text(out: &mut String, text: &str) -> Result<(), FormatError> {
    if text.is_empty() {
        return Ok(());
    }
    out.push_str("<w:r><w:t>");
    write_xml_text(out, text)?;
    out.push_str("</w:t></w:r>");
    Ok(())
}

fn write_footnote_reference(out: &mut String, id: u32) -> Result<(), FormatError> {
    write!(out, "<w:r><w:footnoteReference w:id=\"{id}\"/></w:r>").map_err(map_fmt_error)?;
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

fn split_image_children(children: &[Inline]) -> Option<(String, String)> {
    if children.len() < 2 {
        return None;
    }
    let alt = inline_plain_text(&children[0]);
    let target = inline_plain_text(&children[1]);
    if target.is_empty() {
        return None;
    }
    Some((alt, target))
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

fn escape_xml_attr(value: &str) -> String {
    let mut output = String::new();
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '"' => output.push_str("&quot;"),
            _ => output.push(ch),
        }
    }
    output
}
