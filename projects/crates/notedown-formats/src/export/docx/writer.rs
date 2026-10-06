use std::fmt::Write as _;

use std::collections::HashSet;

use notedown_ir::{AssetKind, Block, DocumentGraph, Inline, ListItem, NodeId, TableRow};

use crate::FormatError;

use super::{
    footnotes::{collect_footnote_catalog, parse_footnote_reference_token, render_footnotes_xml},
    numbering::{ORDERED_NUM_ID, UNORDERED_NUM_ID},
    rels::DocumentRelsRegistry,
};

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
    let validation = graph.validate();
    if !validation.is_valid() {
        return Err(FormatError::invalid_input(format!("invalid DOCX document graph: {:?}", validation.issues)));
    }
    let footnotes = collect_footnote_catalog(graph);
    let mut body = String::new();
    let uses_numbering = graph.blocks.iter().any(|node| matches!(node.block, Block::List { .. }));
    let mut rels = DocumentRelsRegistry::default();
    let nested_blocks = nested_block_ids(graph);
    for node in &graph.blocks {
        if nested_blocks.contains(&node.id) {
            continue;
        }
        if let Block::Opaque { kind, .. } = &node.block {
            if kind == "footnote_definition" {
                continue;
            }
        }
        write_block(&mut body, &node.block, graph, &mut rels)?;
    }
    body.push_str("<w:sectPr/>");

    let footnotes_xml = if footnotes.is_empty() {
        None
    }
    else {
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

fn write_block(out: &mut String, block: &Block, graph: &DocumentGraph, rels: &mut DocumentRelsRegistry) -> Result<(), FormatError> {
    match block {
        Block::Paragraph { content } => {
            out.push_str("<w:p>");
            write_inlines(out, content, graph, rels)?;
            out.push_str("</w:p>");
        }
        Block::Section { level, title, children } => {
            let level = (*level).clamp(1, 6);
            write!(out, "<w:p><w:pPr><w:pStyle w:val=\"Heading{level}\"/></w:pPr>").map_err(map_fmt_error)?;
            write_inlines(out, title, graph, rels)?;
            out.push_str("</w:p>");
            for child_id in children {
                let child = graph.block(*child_id).ok_or_else(|| FormatError::invalid_input(format!("missing section child {child_id:?}")))?;
                write_block(out, &child.block, graph, rels)?;
            }
        }
        Block::Code { content, language: _ } => {
            out.push_str("<w:p>");
            write_run_text(out, content)?;
            out.push_str("</w:p>");
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
            return Err(FormatError::unsupported("docx", "block type is not supported by the conservative DOCX exporter yet"));
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
    write_list_at_level(out, ordered, items, graph, rels, 0)
}

fn write_list_at_level(
    out: &mut String,
    ordered: bool,
    items: &[ListItem],
    graph: &DocumentGraph,
    rels: &mut DocumentRelsRegistry,
    level: u32,
) -> Result<(), FormatError> {
    if level > 8 {
        return Err(FormatError::unsupported("docx", "list nesting exceeds the nine numbering levels"));
    }
    let num_id = if ordered { ORDERED_NUM_ID } else { UNORDERED_NUM_ID };
    for item in items {
        write!(out, "<w:p><w:pPr><w:numPr><w:ilvl w:val=\"{level}\"/><w:numId w:val=\"{num_id}\"/></w:numPr></w:pPr>").map_err(map_fmt_error)?;
        write_inlines(out, &item.content, graph, rels)?;
        out.push_str("</w:p>");
        for child_id in &item.children {
            let Some(child) = graph.block(*child_id) else { continue };
            if let Block::List { ordered, items } = &child.block {
                write_list_at_level(out, *ordered, items, graph, rels, level + 1)?;
            } else {
                write_block(out, &child.block, graph, rels)?;
            }
        }
    }
    Ok(())
}

fn nested_block_ids(graph: &DocumentGraph) -> HashSet<NodeId> {
    let mut ids = HashSet::new();
    for node in &graph.blocks {
        collect_nested_ids(&node.block, &mut ids, graph);
    }
    ids
}

fn collect_nested_ids(block: &Block, ids: &mut HashSet<NodeId>, graph: &DocumentGraph) {
    match block {
        Block::Section { children, .. } => {
            for child in children {
                if ids.insert(*child) {
                    if let Some(node) = graph.block(*child) { collect_nested_ids(&node.block, ids, graph); }
                }
            }
        }
        Block::List { items, .. } => {
            for item in items {
                for child in &item.children {
                    if ids.insert(*child) {
                        if let Some(node) = graph.block(*child) { collect_nested_ids(&node.block, ids, graph); }
                    }
                }
            }
        }
        _ => {}
    }
}

fn write_table(out: &mut String, rows: &[TableRow], graph: &DocumentGraph, rels: &mut DocumentRelsRegistry) -> Result<(), FormatError> {
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

fn write_inlines(out: &mut String, inlines: &[Inline], graph: &DocumentGraph, rels: &mut DocumentRelsRegistry) -> Result<(), FormatError> {
    for inline in inlines {
        write_inline(out, inline, graph, rels)?;
    }
    Ok(())
}

fn write_inline(out: &mut String, inline: &Inline, graph: &DocumentGraph, rels: &mut DocumentRelsRegistry) -> Result<(), FormatError> {
    write_inline_with_properties(out, inline, graph, rels, "")
}

fn write_inline_with_properties(out: &mut String, inline: &Inline, graph: &DocumentGraph, rels: &mut DocumentRelsRegistry, properties: &str) -> Result<(), FormatError> {
    match inline {
        Inline::Text { text } => {
            if properties.is_empty() { write_text_with_footnotes(out, text)?; }
            else { write_run_with_properties(out, text, properties)?; }
        }
        Inline::Styled { style, children } => match style.as_str() {
            style if style.strip_prefix("color:").is_some() => {
                let value = style.strip_prefix("color:").unwrap_or_default();
                if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err(FormatError::unsupported("docx", format!("invalid color style `{style}`")));
                }
                let property = format!("<w:color w:val=\"{value}\"/>");
                let combined = if properties.contains(&property) { properties.to_owned() } else { format!("{properties}{property}") };
                for child in children { write_inline_with_properties(out, child, graph, rels, &combined)?; }
            }
            style if style.strip_prefix("font-size:").is_some() => {
                let value = style.strip_prefix("font-size:").unwrap_or_default();
                if value.is_empty() || !value.parse::<u32>().map(|value| value > 0).unwrap_or(false) {
                    return Err(FormatError::unsupported("docx", format!("invalid font size style `{style}`")));
                }
                let property = format!("<w:sz w:val=\"{value}\"/><w:szCs w:val=\"{value}\"/>");
                let combined = format!("{properties}{property}");
                for child in children { write_inline_with_properties(out, child, graph, rels, &combined)?; }
            }
            style if style.strip_prefix("font-family:").is_some() => {
                let value = style.strip_prefix("font-family:").unwrap_or_default();
                if value.is_empty() || value.chars().any(|character| character.is_control()) {
                    return Err(FormatError::unsupported("docx", format!("invalid font family style `{style}`")));
                }
                let escaped = escape_xml_attr(value);
                let property = format!("<w:rFonts w:ascii=\"{escaped}\" w:hAnsi=\"{escaped}\"/>");
                let combined = format!("{properties}{property}");
                for child in children { write_inline_with_properties(out, child, graph, rels, &combined)?; }
            }
            style if style.strip_prefix("highlight:").is_some() => {
                let value = style.strip_prefix("highlight:").unwrap_or_default();
                if value.is_empty() || value.chars().any(|character| character.is_control() || matches!(character, '<' | '>' | '"' | '\'')) {
                    return Err(FormatError::unsupported("docx", format!("invalid highlight style `{style}`")));
                }
                let property = format!("<w:highlight w:val=\"{}\"/>", escape_xml_attr(value));
                let combined = format!("{properties}{property}");
                for child in children { write_inline_with_properties(out, child, graph, rels, &combined)?; }
            }
            "bold" | "italic" | "underline" | "strike" | "superscript" | "subscript" => {
                let property = match style.as_str() {
                    "bold" => "<w:b/>",
                    "italic" => "<w:i/>",
                    "underline" => "<w:u w:val=\"single\"/>",
                    "strike" => "<w:strike/>",
                    "superscript" => "<w:vertAlign w:val=\"superscript\"/>",
                    "subscript" => "<w:vertAlign w:val=\"subscript\"/>",
                    _ => unreachable!(),
                };
                let combined = if properties.contains(property) { properties.to_owned() } else { format!("{properties}{property}") };
                for child in children { write_inline_with_properties(out, child, graph, rels, &combined)?; }
            }
            "link" => {
                if let Some((display, url)) = split_link_children(children) {
                    let rel_id = rels.id_for_hyperlink(&url);
                    write!(out, "<w:hyperlink r:id=\"{rel_id}\">").map_err(map_fmt_error)?;
                    write_inline_with_properties(out, &Inline::Text { text: display }, graph, rels, properties)?;
                    out.push_str("</w:hyperlink>");
                }
                else {
                    for child in children { write_inline_with_properties(out, child, graph, rels, properties)?; }
                }
            }
            "image" => {
                if let Some((alt, target)) = split_image_children(children) {
                    let bytes = image_bytes_for_target(graph, &target)?;
                    let extent = image_extent(&bytes)?;
                    let rel_id = rels.id_for_image(&target, bytes);
                    write_image_drawing(out, &rel_id, &alt, extent)?;
                }
                else {
                    return Err(FormatError::unsupported("docx", "image inline is missing alt text or target path"));
                }
            }
            "footnote_reference" => {
                let label = children.iter().map(inline_plain_text).collect::<String>();
                let token = format!("[^{label}]");
                let id = parse_footnote_reference_token(&token)
                    .ok_or_else(|| FormatError::unsupported("docx", format!("footnote reference label `{label}` is not numeric")))?;
                write_footnote_reference(out, id)?;
            }
            other => {
                return Err(FormatError::unsupported(
                    "docx",
                    format!("inline style `{other}` is not supported by the conservative DOCX exporter yet"),
                ));
            }
        },
        Inline::InlineCode { text } => {
            write_run_with_properties(out, text, &format!("{properties}<w:rStyle w:val=\"VerbatimChar\"/>"))?;
        }
        Inline::InlineMath { .. } | Inline::Reference { .. } => {
            return Err(FormatError::unsupported("docx", "inline type is not supported by the conservative DOCX exporter yet"));
        }
    }
    Ok(())
}

fn write_image_drawing(out: &mut String, rel_id: &str, alt: &str, extent: (u64, u64)) -> Result<(), FormatError> {
    let (width, height) = extent;
    write!(
        out,
        r#"<w:r><w:drawing><wp:inline xmlns:wp="{WP_NS}" xmlns:a="{A_NS}" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><wp:extent cx="{width}" cy="{height}"/><wp:docPr id="1" name="Picture" descr="{descr}"/><wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="1"/></wp:cNvGraphicFramePr><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="0" name="{descr}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{rel_id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{width}" cy="{height}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#,
        descr = escape_xml_attr(alt),
        rel_id = rel_id,
    )
    .map_err(map_fmt_error)?;
    Ok(())
}

fn image_extent(bytes: &[u8]) -> Result<(u64, u64), FormatError> {
    let dimensions = if bytes.starts_with(b"\x89PNG") {
        png_dimensions(bytes)
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        jpeg_dimensions(bytes)
    } else {
        return Ok((4_680_000, 3_000_000));
    };
    let (pixel_width, pixel_height) = dimensions.filter(|(width, height)| *width > 0 && *height > 0)
        .ok_or_else(|| FormatError::unsupported("docx", "image has invalid or unsupported PNG/JPEG dimensions"))?;
    let natural_width = u64::from(pixel_width) * 9_525;
    let natural_height = u64::from(pixel_height) * 9_525;
    let scale = (4_680_000.0 / natural_width as f64).min(3_000_000.0 / natural_height as f64).min(1.0);
    Ok(((natural_width as f64 * scale).round().max(1.0) as u64, (natural_height as f64 * scale).round().max(1.0) as u64))
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.get(8..12)? != 13u32.to_be_bytes() || bytes.get(12..16)? != b"IHDR" || bytes.len() < 33 { return None; }
    Some((u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?), u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?)))
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut cursor = 2;
    while cursor + 3 < bytes.len() {
        if bytes[cursor] != 0xff { return None; }
        while bytes.get(cursor) == Some(&0xff) { cursor += 1; }
        let marker = *bytes.get(cursor)?;
        cursor += 1;
        if marker == 0x01 { continue; }
        if matches!(marker, 0xd0..=0xda | 0x00) { return None; }
        let length = u16::from_be_bytes(bytes.get(cursor..cursor + 2)?.try_into().ok()?) as usize;
        if length < 2 || cursor.checked_add(length)? > bytes.len() { return None; }
        if matches!(marker, 0xc0 | 0xc1 | 0xc2) {
            if length < 8 || bytes[cursor + 2] != 8 { return None; }
            let components = usize::from(bytes[cursor + 7]);
            if components == 0 || length != 8 + components * 3 { return None; }
            let height = u16::from_be_bytes(bytes.get(cursor + 3..cursor + 5)?.try_into().ok()?) as u32;
            let width = u16::from_be_bytes(bytes.get(cursor + 5..cursor + 7)?.try_into().ok()?) as u32;
            return Some((width, height));
        }
        cursor += length;
    }
    None
}

fn image_bytes_for_target(graph: &DocumentGraph, target: &str) -> Result<Vec<u8>, FormatError> {
    graph
        .assets
        .iter()
        .find(|asset| asset.kind == AssetKind::Image && asset.source.as_deref() == Some(target))
        .and_then(|asset| asset.bytes.clone())
        .ok_or_else(|| FormatError::unsupported("docx", format!("embedded image `{target}` has no materialized bytes for export")))
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
        }
        else {
            write_run_text(out, rest)?;
            break;
        }
    }
    Ok(())
}

fn write_run_text(out: &mut String, text: &str) -> Result<(), FormatError> {
    write_run_with_properties(out, text, "")
}

fn write_run_with_properties(out: &mut String, text: &str, properties: &str) -> Result<(), FormatError> {
    if text.is_empty() {
        return Ok(());
    }
    out.push_str("<w:r>");
    if !properties.is_empty() { write!(out, "<w:rPr>{properties}</w:rPr>").map_err(map_fmt_error)?; }
    let mut segment_start = 0;
    let mut characters = text.char_indices().peekable();
    while let Some((position, character)) = characters.next() {
        if !matches!(character, '\n' | '\r' | '\t') { continue; }
        if segment_start < position {
            out.push_str("<w:t xml:space=\"preserve\">");
            write_xml_text(out, &text[segment_start..position])?;
            out.push_str("</w:t>");
        }
        out.push_str(if character == '\t' { "<w:tab/>" } else { "<w:br/>" });
        segment_start = position + character.len_utf8();
        if character == '\r' && characters.peek().is_some_and(|(_, next)| *next == '\n') {
            let (next_position, _) = characters.next().expect("peeked line feed");
            segment_start = next_position + 1;
        }
    }
    if segment_start < text.len() {
        out.push_str("<w:t xml:space=\"preserve\">");
        write_xml_text(out, &text[segment_start..])?;
        out.push_str("</w:t>");
    }
    out.push_str("</w:r>");
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
