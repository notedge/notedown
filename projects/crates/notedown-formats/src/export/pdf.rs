use std::collections::{HashMap, HashSet};
use std::io::{Read as _, Write as _};

use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};

use notedown_ir::{AssetKind, Block, DocumentGraph, Inline, NodeId};

use crate::FormatError;

#[derive(Clone)]
enum RenderLine { Text(String), Image { source: String, payload: PdfImagePayload } }

#[derive(Clone, Copy)]
struct JpegFrame { width: u32, height: u32, components: u8 }

#[derive(Clone)]
struct PdfImagePayload { stream: Vec<u8>, frame: JpegFrame, filter: ImageFilter }

#[derive(Clone, Copy)]
enum ImageFilter { Dct, Flate { colors: u8 } }

/// Export a document as a conservative PDF 1.4 file.
pub fn export_pdf_bytes(graph: &DocumentGraph) -> Result<Vec<u8>, FormatError> {
    let validation = graph.validate();
    if !validation.is_valid() {
        return Err(FormatError::invalid_input(format!("invalid PDF document graph: {:?}", validation.issues)));
    }
    let lines = render_lines(graph)?;
    let mut pages = paginate_lines(lines);
    if pages.is_empty() { pages.push(Vec::new()); }
    let page_count = pages.len();
    let pages_object = 2;
    let first_page = 3;
    let font_object = first_page + page_count * 2;
    let info_object = font_object + 1;
    let mut image_objects = Vec::new();
    let mut image_ids = HashMap::new();
    for page in &pages {
        for line in page {
            let RenderLine::Image { source, payload } = line else { continue; };
            if image_ids.contains_key(source) { continue; }
            let object_id = info_object + image_objects.len() + 2;
            image_ids.insert(source.clone(), object_id);
            image_objects.push((object_id, payload.clone()));
        }
    }
    let metadata_object = format!(
        "<<{}{}{} >>",
        pdf_metadata_entry("Title", graph.metadata.title.as_deref()),
        pdf_metadata_entry("Author", graph.metadata.authors.first().map(String::as_str)),
        pdf_metadata_entry("Keywords", (!graph.metadata.tags.is_empty()).then(|| graph.metadata.tags.join(", ")).as_deref()),
    );
    let mut objects = vec![b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()];
    let kids = (0..page_count).map(|index| format!("{} 0 R", first_page + index * 2)).collect::<Vec<_>>().join(" ");
    objects.push(format!("<< /Type /Pages /Kids [{kids}] /Count {page_count} >>").into_bytes());
    for (index, page) in pages.iter().enumerate() {
        let page_object = first_page + index * 2;
        let content_object = page_object + 1;
        let content = page_content(page, &image_ids)?;
        let image_resources = page.iter().filter_map(|line| match line {
            RenderLine::Image { source, .. } => image_ids.get(source).map(|id| format!("/Im{id} {id} 0 R")),
            RenderLine::Text(_) => None,
        }).collect::<Vec<_>>().join(" ");
        let xobjects = if image_resources.is_empty() { String::new() } else { format!(" /XObject << {image_resources} >>") };
        objects.push(format!("<< /Type /Page /Parent {pages_object} 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 {font_object} 0 R >>{xobjects} >> /Contents {content_object} 0 R >>").into_bytes());
        let mut content_object = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        content_object.extend_from_slice(&content);
        content_object.extend_from_slice(b"endstream");
        objects.push(content_object);
    }
    objects.push(format!("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding /ToUnicode {} 0 R >>", info_object + 1).into_bytes());
    objects.push(metadata_object.into_bytes());
    let cmap = win_ansi_cmap();
    objects.push(format!("<< /Length {} >>\nstream\n{cmap}endstream", cmap.len()).into_bytes());
    for (object_id, payload) in image_objects {
        let JpegFrame { width, height, components } = payload.frame;
        let color_space = if components == 1 { "DeviceGray" } else { "DeviceRGB" };
        let (filter, decode_parms) = match payload.filter {
            ImageFilter::Dct => ("/DCTDecode".to_owned(), String::new()),
            ImageFilter::Flate { colors } => ("/FlateDecode".to_owned(), format!(" /DecodeParms << /Predictor 15 /Colors {colors} /BitsPerComponent 8 /Columns {width} >>")),
        };
        objects.push(format!("<< /Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace /{color_space} /BitsPerComponent 8 /Filter {filter}{decode_parms} /Length {} >>\nstream\n", payload.stream.len()).into_bytes());
        let object = objects.last_mut().expect("image object header");
        object.extend_from_slice(&payload.stream);
        object.extend_from_slice(b"\nendstream");
        debug_assert_eq!(object_id as usize, objects.len());
    }
    let mut pdf = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        write!(&mut pdf, "{} 0 obj\n", index + 1).unwrap();
        pdf.extend_from_slice(object);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref = pdf.len();
    write!(&mut pdf, "xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).unwrap();
    for offset in offsets { writeln!(&mut pdf, "{offset:010} 00000 n ").unwrap(); }
    write!(&mut pdf, "trailer\n<< /Size {} /Root 1 0 R /Info {info_object} 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).unwrap();
    Ok(pdf)
}

/// Export a document as PDF on disk.
pub fn export_pdf(graph: &DocumentGraph, path: impl AsRef<std::path::Path>) -> Result<(), FormatError> {
    let bytes = export_pdf_bytes(graph)?;
    std::fs::write(path.as_ref(), bytes).map_err(|error| FormatError::invalid_input(format!("failed to write {}: {error}", path.as_ref().display())))
}

fn render_lines(graph: &DocumentGraph) -> Result<Vec<RenderLine>, FormatError> {
    let mut lines = Vec::new();
    let nested = nested_block_ids(graph);
    for node in &graph.blocks {
        if !nested.contains(&node.id) {
            render_block(&mut lines, graph, &node.block)?;
        }
    }
    if lines.iter().any(|line| matches!(line, RenderLine::Text(value) if pdf_literal(value).is_none())) {
        return Err(FormatError::unsupported("pdf", "text contains characters outside the WinAnsiEncoding subset and requires an embedded Unicode font"));
    }
    Ok(lines)
}

fn render_block(lines: &mut Vec<RenderLine>, graph: &DocumentGraph, block: &Block) -> Result<(), FormatError> {
    match block {
        Block::Section { title, children, .. } => {
            lines.push(RenderLine::Text(inline_text(graph, title)?));
            for child_id in children {
                let child = graph.block(*child_id).ok_or_else(|| FormatError::invalid_input(format!("missing PDF section child {child_id:?}")))?;
                render_block(lines, graph, &child.block)?;
            }
        }
        Block::Paragraph { content } | Block::Quote { content } => push_inline_lines(lines, graph, content)?,
        Block::Code { content, .. } | Block::Math { content, .. } => lines.extend(content.lines().map(|line| RenderLine::Text(line.to_owned()))),
        Block::List { items, .. } => {
            for item in items {
                lines.push(RenderLine::Text(format!("- {}", inline_text(graph, &item.content)?)));
                for child_id in &item.children {
                    let child = graph.block(*child_id).ok_or_else(|| FormatError::invalid_input(format!("missing PDF list child {child_id:?}")))?;
                    render_block(lines, graph, &child.block)?;
                }
            }
        }
        Block::Table { rows } => for row in rows { lines.push(RenderLine::Text(row.cells.iter().map(|cell| inline_text(graph, cell)).collect::<Result<Vec<_>, _>>()?.join(" | "))); },
        Block::Opaque { kind, .. } => return Err(FormatError::unsupported("pdf", format!("opaque block `{kind}`"))),
    }
    Ok(())
}

fn nested_block_ids(graph: &DocumentGraph) -> HashSet<NodeId> {
    let mut ids = HashSet::new();
    for node in &graph.blocks {
        collect_nested_ids(&node.block, graph, &mut ids);
    }
    ids
}

fn collect_nested_ids(block: &Block, graph: &DocumentGraph, ids: &mut HashSet<NodeId>) {
    let children = match block {
        Block::Section { children, .. } => children.iter(),
        Block::List { items, .. } => return items.iter().flat_map(|item| item.children.iter()).for_each(|child| {
            if ids.insert(*child) {
                if let Some(node) = graph.block(*child) { collect_nested_ids(&node.block, graph, ids); }
            }
        }),
        _ => return,
    };
    for child in children {
        if ids.insert(*child) {
            if let Some(node) = graph.block(*child) { collect_nested_ids(&node.block, graph, ids); }
        }
    }
}

fn push_inline_lines(lines: &mut Vec<RenderLine>, graph: &DocumentGraph, content: &[Inline]) -> Result<(), FormatError> {
    if let [Inline::Styled { style, children }] = content {
        if style == "image" {
            let target = image_target(children).ok_or_else(|| FormatError::unsupported("pdf", "image inline is missing an asset source"))?;
            let asset = graph.assets.iter().find(|asset| asset.kind == AssetKind::Image && asset.source.as_deref() == Some(target.as_str())).ok_or_else(|| FormatError::unsupported("pdf", format!("image asset `{target}` is not registered")))?;
            let bytes = asset.bytes.clone().ok_or_else(|| FormatError::unsupported("pdf", format!("image asset `{target}` has no materialized bytes")))?;
            let payload = if asset.media_type.as_deref() == Some("image/jpeg") {
                if !bytes.starts_with(&[0xFF, 0xD8]) { return Err(FormatError::unsupported("pdf", format!("image asset `{target}` is not a valid JPEG payload"))); }
                let frame = jpeg_frame(&bytes).ok_or_else(|| FormatError::unsupported("pdf", format!("image asset `{target}` needs a complete 8-bit grayscale or RGB JPEG frame")))?;
                PdfImagePayload { stream: bytes, frame, filter: ImageFilter::Dct }
            } else if asset.media_type.as_deref() == Some("image/png") {
                png_payload(&bytes).ok_or_else(|| FormatError::unsupported("pdf", format!("image asset `{target}` needs an 8-bit grayscale or RGB PNG")))?
            } else {
                return Err(FormatError::unsupported("pdf", "only image/jpeg and 8-bit image/png assets can be embedded"));
            };
            lines.push(RenderLine::Image { source: target, payload });
            return Ok(());
        }
    }
    lines.extend(wrap_line(&inline_text(graph, content)?, 88).into_iter().map(RenderLine::Text));
    Ok(())
}

fn inline_text(graph: &DocumentGraph, inlines: &[Inline]) -> Result<String, FormatError> {
    inlines.iter().try_fold(String::new(), |mut output, inline| {
        match inline {
            Inline::Text { text } | Inline::InlineCode { text } => output.push_str(text),
            Inline::Styled { style, .. } if style == "image" => return Err(FormatError::unsupported("pdf", "image inline must occupy its own paragraph")),
            Inline::Styled { children, .. } => output.push_str(&inline_text(graph, children)?),
            Inline::InlineMath { content, .. } => output.push_str(content),
            Inline::Reference { display, .. } => output.push_str(display),
        }
        Ok(output)
    })
}

fn image_target(children: &[Inline]) -> Option<String> { children.iter().rev().find_map(|inline| match inline { Inline::Text { text } if !text.is_empty() => Some(text.clone()), _ => None }) }

fn page_content(page: &[RenderLine], image_ids: &HashMap<String, usize>) -> Result<Vec<u8>, FormatError> {
    let mut content = Vec::new();
    let mut cursor_y = 770u32;
    for line in page {
        match line {
            RenderLine::Text(value) => {
                content.extend_from_slice(format!("BT\n/F1 12 Tf\n72 {cursor_y} Td\n").as_bytes());
                content.extend_from_slice(&pdf_literal(value).ok_or_else(|| FormatError::unsupported("pdf", "text contains characters outside the WinAnsiEncoding subset"))?);
                content.extend_from_slice(b" Tj\nET\n");
            }
            RenderLine::Image { source, payload } => {
                let name = image_ids.get(source).ok_or_else(|| FormatError::unsupported("pdf", format!("image asset `{source}` is not registered")))?;
                let (draw_width, draw_height) = fit_image(payload.frame.width, payload.frame.height, 468, 300);
                let draw_x = 72 + (468 - draw_width) / 2;
                let draw_y = cursor_y - draw_height;
                content.extend_from_slice(format!("q {draw_width} 0 0 {draw_height} {draw_x} {draw_y} cm /Im{name} Do Q\n").as_bytes());
            }
        }
        cursor_y -= line_height(line);
    }
    Ok(content)
}

fn line_height(line: &RenderLine) -> u32 {
    match line {
        RenderLine::Text(_) => 16,
        RenderLine::Image { payload, .. } => fit_image(payload.frame.width, payload.frame.height, 468, 300).1 + 16,
    }
}

fn paginate_lines(lines: Vec<RenderLine>) -> Vec<Vec<RenderLine>> {
    let mut pages = Vec::new();
    let mut page = Vec::new();
    let mut used_height = 0;
    for line in lines {
        let height = line_height(&line);
        if used_height + height > 720 && !page.is_empty() {
            pages.push(std::mem::take(&mut page));
            used_height = 0;
        }
        used_height += height;
        page.push(line);
    }
    if !page.is_empty() { pages.push(page); }
    pages
}

fn wrap_line(value: &str, width: usize) -> Vec<String> {
    let mut result = Vec::new();
    for logical_line in value.split('\n') {
        let mut remainder = logical_line;
        while let Some((boundary, _)) = remainder.char_indices().nth(width) {
            let prefix = &remainder[..boundary];
            let split = prefix.char_indices().rev()
                .find(|(_, character)| character.is_whitespace())
                .map_or(boundary, |(offset, character)| offset + character.len_utf8());
            result.push(remainder[..split].to_owned());
            remainder = &remainder[split..];
        }
        result.push(remainder.to_owned());
    }
    result
}

fn fit_image(width: u32, height: u32, max_width: u32, max_height: u32) -> (u32, u32) {
    if width == 0 || height == 0 { return (max_width, max_height); }
    let width_ratio = max_width as f64 / width as f64;
    let height_ratio = max_height as f64 / height as f64;
    let scale = width_ratio.min(height_ratio).min(1.0);
    ((width as f64 * scale).round().max(1.0) as u32, (height as f64 * scale).round().max(1.0) as u32)
}

fn jpeg_frame(bytes: &[u8]) -> Option<JpegFrame> {
    if !bytes.starts_with(&[0xff, 0xd8]) { return None; }
    let mut cursor = 2;
    while cursor + 3 < bytes.len() {
        if bytes.get(cursor) != Some(&0xff) { return None; }
        while cursor < bytes.len() && bytes[cursor] == 0xff { cursor += 1; }
        let marker = *bytes.get(cursor)?;
        cursor += 1;
        if marker == 0x01 { continue; }
        if matches!(marker, 0xd0..=0xda | 0x00) { return None; }
        let length = u16::from_be_bytes(bytes.get(cursor..cursor + 2)?.try_into().ok()?) as usize;
        if length < 2 || cursor.checked_add(length)? > bytes.len() { return None; }
        if matches!(marker, 0xc0 | 0xc1 | 0xc2) {
            if length < 8 || bytes[cursor + 2] != 8 { return None; }
            let height = u16::from_be_bytes(bytes.get(cursor + 3..cursor + 5)?.try_into().ok()?) as u32;
            let width = u16::from_be_bytes(bytes.get(cursor + 5..cursor + 7)?.try_into().ok()?) as u32;
            let components = bytes[cursor + 7];
            return (width > 0 && height > 0 && matches!(components, 1 | 3) && length == 8 + 3 * usize::from(components)).then_some(JpegFrame { width, height, components });
        }
        cursor += length;
    }
    None
}

fn png_payload(bytes: &[u8]) -> Option<PdfImagePayload> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") { return None; }
    let mut cursor = 8;
    let mut width = None;
    let mut height = None;
    let mut colors = None;
    let mut idat = Vec::new();
    while cursor + 12 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[cursor..cursor + 4].try_into().ok()?) as usize;
        let chunk_end = cursor.checked_add(12)?.checked_add(length)?;
        if chunk_end > bytes.len() { return None; }
        let kind = &bytes[cursor + 4..cursor + 8];
        let data = &bytes[cursor + 8..cursor + 8 + length];
        match kind {
            b"IHDR" if length == 13 => {
                width = Some(u32::from_be_bytes(data[0..4].try_into().ok()?));
                height = Some(u32::from_be_bytes(data[4..8].try_into().ok()?));
                if data[8] != 8 || !matches!(data[9], 0 | 2) || data[10] != 0 || data[11] != 0 || data[12] != 0 { return None; }
                colors = Some(if data[9] == 0 { 1 } else { 3 });
            }
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        cursor = chunk_end;
    }
    let width = width?;
    let height = height?;
    if width == 0 || height == 0 { return None; }
    let colors = colors?;
    let expected = usize::try_from(height).ok()?.checked_mul(usize::try_from(width).ok()?.checked_mul(usize::from(colors))?.checked_add(1)?)?;
    let mut decoder = ZlibDecoder::new(idat.as_slice());
    if expected > 32 * 1024 * 1024 { return None; }
    let mut scanlines = Vec::new();
    decoder.by_ref().take(expected as u64 + 1).read_to_end(&mut scanlines).ok()?;
    if scanlines.len() != expected { return None; }
    let row_bytes = usize::try_from(width).ok()?.checked_mul(usize::from(colors))?;
    if scanlines.chunks_exact(row_bytes + 1).any(|row| row[0] > 4) { return None; }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&scanlines).ok()?;
    let stream = encoder.finish().ok()?;
    Some(PdfImagePayload { stream, frame: JpegFrame { width, height, components: colors }, filter: ImageFilter::Flate { colors } })
}

fn pdf_literal(value: &str) -> Option<Vec<u8>> {
    let mut output = vec![b'('];
    for character in value.chars() {
        let byte = win_ansi_encode(character)?;
        match byte {
            b'\\' => output.extend_from_slice(b"\\\\"),
            b'(' => output.extend_from_slice(b"\\("),
            b')' => output.extend_from_slice(b"\\)"),
            b'\n' => output.extend_from_slice(b"\\n"),
            byte => output.push(byte),
        }
    }
    output.push(b')');
    Some(output)
}

fn win_ansi_encode(character: char) -> Option<u8> {
    let special = [
        ('€', 0x80), ('‚', 0x82), ('ƒ', 0x83), ('„', 0x84), ('…', 0x85), ('†', 0x86), ('‡', 0x87),
        ('ˆ', 0x88), ('‰', 0x89), ('Š', 0x8a), ('‹', 0x8b), ('Œ', 0x8c), ('Ž', 0x8e),
        ('‘', 0x91), ('’', 0x92), ('“', 0x93), ('”', 0x94), ('•', 0x95), ('–', 0x96), ('—', 0x97),
        ('˜', 0x98), ('™', 0x99), ('š', 0x9a), ('›', 0x9b), ('œ', 0x9c), ('ž', 0x9e), ('Ÿ', 0x9f),
    ];
    if let Some((_, byte)) = special.iter().find(|(mapped, _)| *mapped == character) { return Some(*byte); }
    let value = u32::from(character);
    if value <= 0xff && !(0x18..=0x1f).contains(&(value as u8)) && !(0x80..=0x9f).contains(&(value as u8)) {
        return Some(value as u8);
    }
    None
}

fn win_ansi_cmap() -> String {
    let mut entries = Vec::new();
    for value in 0..=0xffff {
        if let Some(character) = char::from_u32(value) {
            if let Some(byte) = win_ansi_encode(character) {
                entries.push(format!("<{byte:02X}> <{value:04X}>\n"));
            }
        }
    }
    let mut cmap = String::from("/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /NotedownWinAnsi def\n/CMapType 2 def\n1 begincodespacerange\n<00> <FF>\nendcodespacerange\n");
    for chunk in entries.chunks(100) {
        cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for entry in chunk { cmap.push_str(entry); }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    cmap
}

fn pdf_metadata_entry(key: &str, value: Option<&str>) -> String { let Some(value) = value else { return String::new(); }; if value.is_ascii() { let literal = String::from_utf8(pdf_literal(value).expect("ASCII is PDFDocEncoding")).expect("PDF literal is ASCII"); return format!(" /{key} {literal}"); } let mut encoded = String::from("<FEFF"); for unit in value.encode_utf16() { encoded.push_str(&format!("{unit:04X}")); } encoded.push('>'); format!(" /{key} {encoded}") }
