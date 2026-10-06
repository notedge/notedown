use std::collections::HashMap;
use std::io::{Read, Write};

use notedown_ir::{Asset, AssetId, AssetKind, Block, DocumentGraph, DocumentId, DocumentMetadata, Inline, LossMarker, SemanticStatus};

use crate::FormatError;

/// Import basic page text from a PDF into semantic paragraphs.
pub fn import_pdf_bytes(label: &str, bytes: &[u8]) -> Result<DocumentGraph, FormatError> {
    if !bytes.starts_with(b"%PDF-") { return Err(FormatError::invalid_input("input is not a PDF file")); }
    let mut graph = DocumentGraph::new(DocumentId(1));
    if let Some(metadata) = pdf_metadata(bytes) { graph.metadata = metadata; }
    report_unsupported_pdf_structures(bytes, &mut graph);
    let page_content_ids = page_content_ids(bytes);
    let streams = stream_contents(bytes);
    let font_maps = page_font_maps(bytes, &streams)?;
    let images = page_images(bytes, &streams)?;
    let use_page_filter = !page_content_ids.is_empty();
    let selected = if use_page_filter {
        page_content_ids
            .iter()
            .filter_map(|id| streams.iter().find(|stream| stream.object_id == Some(*id)))
            .collect::<Vec<_>>()
    } else {
        streams.iter().collect::<Vec<_>>()
    };
    for stream in selected {
        let decoded = decode_stream(stream.dictionary, stream.data)?;
        let extracted = extract_text_operators(&decoded, &font_maps);
        if extracted.spacing_adjustments {
            graph.push_loss(LossMarker {
                code: "import.pdf.text_spacing".into(),
                message: "TJ numeric spacing adjustments are ignored while preserving the adjacent text strings".into(),
                status: SemanticStatus::Partial,
            });
        }
        for event in extracted.events {
            match event {
                ContentEvent::Text(text) => {
                    for paragraph in split_pdf_paragraphs(&text) {
                        graph.push_block(Block::Paragraph { content: vec![Inline::Text { text: paragraph.to_string() }] });
                    }
                }
                ContentEvent::NamedImage(image_name) => {
                    if let Some(image) = images.get(&image_name) {
                        append_image_asset(&mut graph, image);
                    } else {
                        graph.push_loss(LossMarker {
                            code: "import.pdf.unresolved_xobject".into(),
                            message: format!("content stream references named XObject `/{image_name}` that is not a supported image"),
                            status: SemanticStatus::Unresolved,
                        });
                    }
                }
                ContentEvent::InlineImage(image) => append_image_asset(&mut graph, &image),
            }
        }
    }
    if graph.blocks.is_empty() {
        graph.push_loss(LossMarker { code: "import.pdf.no_extractable_text".into(), message: format!("{label}: no supported text operators were found"), status: SemanticStatus::Partial });
    }
    graph.push_loss(LossMarker { code: "import.pdf.partial_coverage".into(), message: "PDF import preserves basic page text and JPEG or 8-bit grayscale/RGB Flate image assets in content-stream order, but not general layout, fonts, indexed or alpha images, annotations, forms, or encryption".into(), status: SemanticStatus::Partial });
    Ok(graph)
}

fn split_pdf_paragraphs(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n').filter(|paragraph| !paragraph.is_empty())
}

fn report_unsupported_pdf_structures(bytes: &[u8], graph: &mut DocumentGraph) {
    if bytes.windows(7).any(|window| window == b"/Annots") {
        graph.push_loss(LossMarker {
            code: "import.pdf.annotations".into(),
            message: "PDF annotations are detected but annotation ranges, URI actions, and widget content are not represented in notedown-ir".into(),
            status: SemanticStatus::Partial,
        });
    }
    if bytes.windows(9).any(|window| window == b"/AcroForm") {
        graph.push_loss(LossMarker {
            code: "import.pdf.forms".into(),
            message: "PDF AcroForm fields are detected but form names, values, appearances, and widget geometry are not represented in notedown-ir".into(),
            status: SemanticStatus::Partial,
        });
    }
    if bytes.windows(8).any(|window| window == b"/Encrypt") {
        graph.push_loss(LossMarker {
            code: "import.pdf.encryption".into(),
            message: "PDF encryption is detected and encrypted object semantics are not imported".into(),
            status: SemanticStatus::Unsupported,
        });
    }
}

fn pdf_metadata(bytes: &[u8]) -> Option<DocumentMetadata> {
    let info_position = bytes.windows(5).position(|window| window == b"/Info")?;
    let info_ref = reference_ids(bytes.get(info_position + 5..)?).first().copied()?;
    let marker = format!("{info_ref} 0 obj");
    let start = bytes.windows(marker.len()).position(|window| window == marker.as_bytes())? + marker.len();
    let end = bytes[start..].windows(6).position(|window| window == b"endobj")? + start;
    let object = &bytes[start..end];
    let title = pdf_dictionary_string(object, b"/Title");
    let author = pdf_dictionary_string(object, b"/Author");
    let keywords = pdf_dictionary_string(object, b"/Keywords");
    if title.is_none() && author.is_none() && keywords.is_none() { return None; }
    Some(DocumentMetadata {
        title,
        language: None,
        authors: author.into_iter().collect(),
        tags: keywords.map(|value| value.split(',').map(str::trim).filter(|item| !item.is_empty()).map(str::to_owned).collect()).unwrap_or_default(),
    })
}

fn pdf_dictionary_string(object: &[u8], key: &[u8]) -> Option<String> {
    let start = object.windows(key.len()).position(|window| window == key)? + key.len();
    let open = skip_space(object, start);
    match object.get(open)? {
        b'(' => {
            let close = literal_close(object, open + 1)?;
            Some(decode_pdf_string(&object[open + 1..close]))
        }
        b'<' if object.get(open + 1) != Some(&b'<') => {
            let close = object[open + 1..].iter().position(|byte| *byte == b'>')? + open + 1;
            let value = &object[open + 1..close];
            if value.iter().any(|byte| !byte.is_ascii_whitespace() && !byte.is_ascii_hexdigit()) { return None; }
            Some(decode_text_bytes(&hex_bytes(value)))
        }
        _ => None,
    }
}

/// Import a PDF file from disk.
pub fn import_pdf(path: impl AsRef<std::path::Path>) -> Result<DocumentGraph, FormatError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|error| FormatError::invalid_input(format!("failed to read {}: {error}", path.display())))?;
    import_pdf_bytes(&path.display().to_string(), &bytes)
}

struct PdfStream<'a> { object_id: Option<u32>, dictionary: &'a [u8], data: &'a [u8] }

#[derive(Clone, Debug)]
struct PdfImage { bytes: Vec<u8>, media_type: &'static str, source: String }

#[derive(Debug)]
enum ContentEvent { Text(String), NamedImage(String), InlineImage(PdfImage) }

#[derive(Default)]
struct ExtractedText { events: Vec<ContentEvent>, spacing_adjustments: bool }

fn append_image_asset(graph: &mut DocumentGraph, image: &PdfImage) {
    if !graph.assets.iter().any(|asset| asset.kind == AssetKind::Image && asset.source.as_deref() == Some(image.source.as_str())) {
        let asset_id = AssetId(graph.assets.len() as u64 + 1);
        graph.push_asset(Asset {
            id: asset_id,
            kind: AssetKind::Image,
            content_identity: None,
            source: Some(image.source.clone()),
            media_type: Some(image.media_type.to_owned()),
            status: SemanticStatus::Resolved,
            bytes: Some(image.bytes.clone()),
        });
    }
    graph.push_block(Block::Paragraph { content: vec![Inline::Styled {
        style: "image".into(),
        children: vec![Inline::Text { text: String::new() }, Inline::Text { text: image.source.clone() }],
    }] });
}

#[derive(Clone, Debug, Default)]
struct ToUnicodeMap {
    entries: HashMap<Vec<u8>, String>,
    max_code_len: usize,
    code_lengths: Vec<usize>,
    winansi_fallback: bool,
}

impl ToUnicodeMap {
    fn decode(&self, bytes: &[u8]) -> String {
        if self.entries.is_empty() && self.winansi_fallback {
            return bytes.iter().map(|byte| win_ansi_encoding(*byte)).collect();
        }
        let mut output = String::new();
        let mut cursor = 0;
        while cursor < bytes.len() {
            let mut found = None;
            for length in (1..=self.max_code_len.min(bytes.len() - cursor)).rev() {
                if !self.code_lengths.is_empty() && !self.code_lengths.contains(&length) { continue; }
                if let Some(value) = self.entries.get(&bytes[cursor..cursor + length]) {
                    found = Some((length, value));
                    break;
                }
            }
            if let Some((length, value)) = found {
                output.push_str(value);
                cursor += length;
            } else {
                output.push('\u{FFFD}');
                cursor += 1;
            }
        }
        output
    }
}

fn stream_contents(bytes: &[u8]) -> Vec<PdfStream<'_>> {
    let mut streams = Vec::new();
    let mut offset = 0;
    while let Some(relative) = bytes[offset..].windows(6).position(|window| window == b"stream") {
        let marker = offset + relative;
        let dictionary_start = bytes[..marker].windows(3).rposition(|window| window == b"obj").map_or(0, |position| position + 3);
        let object_id = object_id(bytes, dictionary_start);
        let dictionary = &bytes[dictionary_start..marker];
        let mut start = marker + 6;
        start = if bytes.get(start..start + 2) == Some(b"\r\n") { start + 2 } else if bytes.get(start..start + 1) == Some(b"\n") { start + 1 } else { offset = start; continue; };
        let end = dictionary_value(dictionary, b"/Length")
            .and_then(|length| start.checked_add(length).filter(|end| *end <= bytes.len()))
            .filter(|end| bytes[skip_space(bytes, *end)..].starts_with(b"endstream"))
            .unwrap_or_else(|| bytes[start..].windows(9).position(|window| window == b"endstream").map_or(bytes.len(), |position| start + position));
        if end > start { streams.push(PdfStream { object_id, dictionary, data: &bytes[start..end] }); }
        offset = skip_space(bytes, end).saturating_add(9);
        if offset >= bytes.len() { break; }
    }
    streams
}

fn page_font_maps(bytes: &[u8], streams: &[PdfStream<'_>]) -> Result<HashMap<String, ToUnicodeMap>, FormatError> {
    let bodies = object_bodies(bytes);
    let mut result = HashMap::new();
    for resources in page_resource_sections(bytes) {
        let Some(font_start) = resources.windows(5).position(|window| window == b"/Font") else { continue; };
        let section = &resources[font_start + 5..];
        for (name, object_id) in resource_font_references(section) {
            let Some((_, font_body)) = bodies.iter().find(|(id, _)| *id == object_id) else { continue; };
            if let Some(to_unicode) = body_reference(font_body, b"/ToUnicode") {
                if let Some(stream) = streams.iter().find(|stream| stream.object_id == Some(to_unicode)) {
                    let decoded = decode_stream(stream.dictionary, stream.data)?;
                    if let Some(map) = parse_to_unicode_cmap(&decoded) {
                        result.insert(name, map);
                        continue;
                    }
                }
            }
            if body_contains_name(font_body, b"/WinAnsiEncoding") {
                result.insert(name, ToUnicodeMap { winansi_fallback: true, ..ToUnicodeMap::default() });
            }
        }
    }
    Ok(result)
}

fn page_images(bytes: &[u8], streams: &[PdfStream<'_>]) -> Result<HashMap<String, PdfImage>, FormatError> {
    let bodies = object_bodies(bytes);
    let mut result = HashMap::new();
    for resources in page_resource_sections(bytes) {
        let Some(xobject_start) = resources.windows(8).position(|window| window == b"/XObject") else { continue; };
        for (name, object_id) in resource_font_references(&resources[xobject_start + 8..]) {
            let Some((_, image_body)) = bodies.iter().find(|(id, _)| *id == object_id) else { continue; };
            if !dictionary_type_is(image_body, b"/Image") && !dictionary_subtype_is(image_body, b"/Image") { continue; }
            let Some(stream) = streams.iter().find(|stream| stream.object_id == Some(object_id)) else { continue; };
            let source = format!("pdf-object-{object_id}");
            let image = if image_body.windows(10).any(|window| window == b"/DCTDecode") {
                PdfImage { bytes: stream.data.to_vec(), media_type: "image/jpeg", source }
            } else if image_body.windows(12).any(|window| window == b"/FlateDecode") {
                let bytes = decode_pdf_image_png(image_body, stream.data)?;
                PdfImage { bytes, media_type: "image/png", source }
            } else { continue };
            result.insert(name, image);
        }
    }
    Ok(result)
}

fn decode_pdf_image_png(dictionary: &[u8], stream: &[u8]) -> Result<Vec<u8>, FormatError> {
    let width = dictionary_value(dictionary, b"/Width").and_then(|value| u32::try_from(value).ok()).ok_or_else(|| FormatError::parse("pdf", "Flate image has invalid /Width"))?;
    let height = dictionary_value(dictionary, b"/Height").and_then(|value| u32::try_from(value).ok()).ok_or_else(|| FormatError::parse("pdf", "Flate image has invalid /Height"))?;
    let bits = dictionary_value(dictionary, b"/BitsPerComponent").unwrap_or(8);
    let color_position = dictionary.windows(11).position(|window| window == b"/ColorSpace").ok_or_else(|| FormatError::unsupported("pdf", "Flate image requires a direct grayscale or RGB colorspace"))?;
    let color_start = skip_space(dictionary, color_position + 11);
    let color_end = dictionary[color_start..].iter().position(|byte| byte.is_ascii_whitespace() || matches!(byte, b'/' | b'>' | b'[')).map_or(dictionary.len(), |position| color_start + position);
    let color_end = if dictionary.get(color_start) == Some(&b'/') {
        dictionary[color_start + 1..].iter().position(|byte| byte.is_ascii_whitespace() || matches!(byte, b'/' | b'>' | b'[')).map_or(dictionary.len(), |position| color_start + 1 + position)
    } else { color_end };
    let colors = match &dictionary[color_start..color_end] {
        b"/DeviceGray" => 1,
        b"/DeviceRGB" => 3,
        _ => return Err(FormatError::unsupported("pdf", "Flate image requires a direct grayscale or RGB colorspace")),
    };
    if dictionary_value(dictionary, b"/Colors").is_some_and(|value| value != colors) { return Err(FormatError::unsupported("pdf", "Flate image predictor colors disagree with colorspace")); }
    if width == 0 || height == 0 || bits != 8 || !matches!(colors, 1 | 3) { return Err(FormatError::unsupported("pdf", "Flate image requires non-zero 8-bit grayscale or RGB dimensions")); }
    let mut decoder = flate2::read::ZlibDecoder::new(stream);
    let mut filtered = Vec::new();
    decoder.by_ref().take(32 * 1024 * 1024 + 1).read_to_end(&mut filtered).map_err(|error| FormatError::parse("pdf", format!("invalid Flate image: {error}")))?;
    if filtered.len() > 32 * 1024 * 1024 { return Err(FormatError::unsupported("pdf", "decoded image exceeds the 32 MiB limit")); }
    let row_bytes = usize::try_from(width).ok().and_then(|value| value.checked_mul(colors)).ok_or_else(|| FormatError::unsupported("pdf", "Flate image row is too large"))?;
    let predictor = dictionary_value(dictionary, b"/Predictor").unwrap_or(1);
    let rows = if predictor == 15 {
        let expected = usize::try_from(height).ok().and_then(|value| value.checked_mul(row_bytes + 1)).ok_or_else(|| FormatError::unsupported("pdf", "Flate image is too large"))?;
        if filtered.len() != expected { return Err(FormatError::parse("pdf", "Flate image data length does not match dimensions")); }
        unfilter_png_rows(&filtered, row_bytes, colors)?
    } else if predictor == 1 {
        let expected = usize::try_from(height).ok().and_then(|value| value.checked_mul(row_bytes)).ok_or_else(|| FormatError::unsupported("pdf", "Flate image is too large"))?;
        if filtered.len() != expected { return Err(FormatError::parse("pdf", "Flate image data length does not match dimensions")); }
        filtered.chunks_exact(row_bytes).map(|row| row.to_vec()).collect()
    } else { return Err(FormatError::unsupported("pdf", "unsupported Flate image predictor")); };
    let raw = rows.into_iter().flatten().collect::<Vec<_>>();
    Ok(encode_png(width, height, colors as u8, &raw))
}

fn unfilter_png_rows(filtered: &[u8], row_bytes: usize, bytes_per_pixel: usize) -> Result<Vec<Vec<u8>>, FormatError> {
    let mut rows = Vec::new();
    for (index, row) in filtered.chunks_exact(row_bytes + 1).enumerate() {
        let filter = row[0];
        let prior = rows.get(index.wrapping_sub(1)).map(Vec::as_slice).unwrap_or(&[]);
        let mut current = row[1..].to_vec();
        for offset in 0..current.len() {
            let left = if offset >= bytes_per_pixel { current[offset - bytes_per_pixel] } else { 0 };
            let up = prior.get(offset).copied().unwrap_or(0);
            let upper_left = if offset >= bytes_per_pixel { prior.get(offset - bytes_per_pixel).copied().unwrap_or(0) } else { 0 };
            current[offset] = match filter { 0 => current[offset], 1 => current[offset].wrapping_add(left), 2 => current[offset].wrapping_add(up), 3 => current[offset].wrapping_add(((u16::from(left) + u16::from(up)) / 2) as u8), 4 => current[offset].wrapping_add(paeth(left, up, upper_left)), _ => return Err(FormatError::unsupported("pdf", "unsupported PNG predictor row filter")) };
        }
        rows.push(current);
    }
    Ok(rows)
}

fn paeth(left: u8, up: u8, upper_left: u8) -> u8 {
    let p = i32::from(left) + i32::from(up) - i32::from(upper_left);
    let pa = (p - i32::from(left)).abs();
    let pb = (p - i32::from(up)).abs();
    let pc = (p - i32::from(upper_left)).abs();
    if pa <= pb && pa <= pc { left } else if pb <= pc { up } else { upper_left }
}

fn encode_png(width: u32, height: u32, colors: u8, raw: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    for row in raw.chunks_exact(width as usize * colors as usize) { let _ = encoder.write_all(&[0]); let _ = encoder.write_all(row); }
    let compressed = encoder.finish().unwrap_or_default();
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    let color_type = if colors == 1 { 0 } else { 2 };
    header.extend_from_slice(&[8, color_type, 0, 0, 0]);
    append_png_chunk(&mut png, b"IHDR", &header);
    append_png_chunk(&mut png, b"IDAT", &compressed);
    append_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn append_png_chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    output.extend_from_slice(kind);
    output.extend_from_slice(data);
    let mut crc = 0xffff_ffffu32;
    for byte in kind.iter().chain(data) {
        crc ^= u32::from(*byte);
        for _ in 0..8 { crc = if crc & 1 == 1 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 }; }
    }
    output.extend_from_slice(&(crc ^ 0xffff_ffff).to_be_bytes());
}

fn page_resource_sections(bytes: &[u8]) -> Vec<Vec<u8>> {
    let bodies = object_bodies(bytes);
    let mut result = Vec::new();
    if let Some((_, catalog)) = bodies.iter().find(|(_, body)| dictionary_type_is(body, b"/Catalog")) {
        if let Some(pages) = body_reference(catalog, b"/Pages") {
            collect_page_resources(&bodies, pages, None, &mut result, &mut Vec::new());
        }
    }
    if result.is_empty() {
        for (_, body) in bodies.iter().filter(|(_, body)| dictionary_type_is(body, b"/Page")) {
            if let Some(resources) = resource_dictionary(body, &bodies) { result.push(resources); }
        }
    }
    result
}

fn collect_page_resources(
    bodies: &[(u32, Vec<u8>)],
    object: u32,
    inherited: Option<Vec<u8>>,
    result: &mut Vec<Vec<u8>>,
    visited: &mut Vec<u32>,
) {
    if visited.contains(&object) { return; }
    visited.push(object);
    let Some((_, body)) = bodies.iter().find(|(id, _)| *id == object) else { return; };
    let resources = resource_dictionary(body, bodies).or(inherited);
    if dictionary_type_is(body, b"/Page") {
        if let Some(resources) = resources { result.push(resources); }
        return;
    }
    for child in body_reference_values(body, b"/Kids").unwrap_or_default() {
        collect_page_resources(bodies, child, resources.clone(), result, visited);
    }
}

fn resource_dictionary(body: &[u8], bodies: &[(u32, Vec<u8>)]) -> Option<Vec<u8>> {
    let position = body.windows(10).position(|window| window == b"/Resources")?;
    let start = skip_space(body, position + 10);
    if body.get(start..start + 2) == Some(b"<<") {
        let end = dictionary_end(body, start)?;
        return Some(body[start..end].to_vec());
    }
    let id = reference_ids(body.get(start..)?).first().copied()?;
    let resource_body = bodies.iter().find(|(object, _)| *object == id).map(|(_, body)| body)?;
    let start = resource_body.windows(2).position(|window| window == b"<<")?;
    let end = dictionary_end(resource_body, start)?;
    Some(resource_body[start..end].to_vec())
}

fn dictionary_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut cursor = start;
    while cursor + 1 < bytes.len() {
        if bytes[cursor..].starts_with(b"<<") { depth += 1; cursor += 2; continue; }
        if bytes[cursor..].starts_with(b">>") {
            depth = depth.checked_sub(1)?;
            cursor += 2;
            if depth == 0 { return Some(cursor); }
            continue;
        }
        cursor += 1;
    }
    None
}

fn resource_font_references(section: &[u8]) -> Vec<(String, u32)> {
    let mut result = Vec::new();
    let tokens = split_pdf_whitespace(section).map(trim_pdf_brackets).collect::<Vec<_>>();
    for index in 0..tokens.len().saturating_sub(3) {
        let token = tokens[index];
        let Some(name) = token.strip_prefix(b"/") else { continue; };
        if name.is_empty() || tokens[index + 2] != b"0" || tokens[index + 3] != b"R" { continue; }
        let Ok(object_id) = std::str::from_utf8(tokens[index + 1]).unwrap_or("").parse() else { continue; };
        result.push((String::from_utf8_lossy(name).into_owned(), object_id));
    }
    result
}

fn parse_to_unicode_cmap(bytes: &[u8]) -> Option<ToUnicodeMap> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut map = ToUnicodeMap::default();
    let tokens = pdf_tokens(text).collect::<Vec<_>>();
    let mut index = 0;
    while index < tokens.len() {
        match tokens[index] {
            "begincodespacerange" => {
                let count = tokens.get(index.wrapping_sub(1)).and_then(|value| value.parse::<usize>().ok()).unwrap_or(usize::MAX);
                index += 1;
                for _ in 0..count {
                    if index + 1 >= tokens.len() || tokens[index] == "endcodespacerange" { break; }
                    if let (Some(start), Some(end)) = (parse_hex_token(tokens[index]), parse_hex_token(tokens[index + 1])) {
                        if !start.is_empty() && start.len() == end.len() && !map.code_lengths.contains(&start.len()) {
                            map.code_lengths.push(start.len());
                        }
                    }
                    index += 2;
                }
            }
            "beginbfchar" => {
                let count = tokens.get(index.wrapping_sub(1)).and_then(|value| value.parse::<usize>().ok()).unwrap_or(usize::MAX);
                index += 1;
                for _ in 0..count {
                    if index + 1 >= tokens.len() || tokens[index] == "endbfchar" { break; }
                    if let (Some(source), Some(destination)) = (parse_hex_token(tokens[index]), parse_hex_token(tokens[index + 1])) {
                        add_cmap_entry(&mut map, source, destination);
                    }
                    index += 2;
                }
            }
            "beginbfrange" => {
                let count = tokens.get(index.wrapping_sub(1)).and_then(|value| value.parse::<usize>().ok()).unwrap_or(usize::MAX);
                index += 1;
                for _ in 0..count {
                    if index + 2 >= tokens.len() || tokens[index] == "endbfrange" { break; }
                    let source_start = parse_hex_token(tokens[index]);
                    let source_end = parse_hex_token(tokens[index + 1]);
                    if let (Some(start), Some(end)) = (source_start, source_end) {
                        let destination = tokens[index + 2];
                        if destination.starts_with('<') {
                            if let Some(base) = parse_hex_token(destination) {
                                add_cmap_range(&mut map, start, end, base);
                            }
                        } else if destination == "[" {
                            let mut source = start.clone();
                            let mut destination_index = index + 3;
                            while source <= end && destination_index < tokens.len() && tokens[destination_index] != "]" {
                                if let Some(value) = parse_hex_token(tokens[destination_index]) { add_cmap_entry(&mut map, source.clone(), value); }
                                increment_code(&mut source);
                                destination_index += 1;
                            }
                            index = destination_index;
                        }
                    }
                    index += 3;
                }
            }
            _ => {}
        }
        index += 1;
    }
    (!map.entries.is_empty()).then_some(map)
}

fn pdf_tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split_ascii_whitespace()
        .filter(|token| !token.is_empty())
}

fn parse_hex_token(token: &str) -> Option<Vec<u8>> {
    let value = token.strip_prefix('<')?.strip_suffix('>')?;
    Some(hex_bytes(value.as_bytes()))
}

fn add_cmap_entry(map: &mut ToUnicodeMap, source: Vec<u8>, destination: Vec<u8>) {
    map.max_code_len = map.max_code_len.max(source.len());
    map.entries.insert(source, decode_utf16be_or_utf8(&destination));
}

fn add_cmap_range(map: &mut ToUnicodeMap, start: Vec<u8>, end: Vec<u8>, mut destination: Vec<u8>) {
    if start.is_empty() || start.len() != end.len() || destination.is_empty() || destination.len() % 2 != 0 || start.len() > 4 { return; }
    let source_start = start.iter().fold(0u32, |value, byte| (value << 8) | u32::from(*byte));
    let source_end = end.iter().fold(0u32, |value, byte| (value << 8) | u32::from(*byte));
    if source_end < source_start || source_end - source_start >= 65_536 { return; }
    for offset in 0..=source_end.saturating_sub(source_start) {
        let source_value = source_start + offset;
        let source_len = start.len();
        let mut source = vec![0u8; source_len];
        for (index, byte) in source.iter_mut().enumerate() {
            *byte = (source_value >> (8 * (source_len - index - 1))) as u8;
        }
        add_cmap_entry(map, source, destination.clone());
        if offset < source_end.saturating_sub(source_start) && !increment_unicode(&mut destination) { break; }
    }
}

fn increment_code(value: &mut Vec<u8>) {
    for byte in value.iter_mut().rev() {
        if *byte < 0xff { *byte += 1; return; }
        *byte = 0;
    }
}

fn increment_unicode(value: &mut Vec<u8>) -> bool {
    if value.len() < 2 || value.len() % 2 != 0 { return false; }
    for index in (0..value.len()).rev().step_by(2) {
        if index == 0 { break; }
        let current = u16::from_be_bytes([value[index - 1], value[index]]);
        if current < 0xffff {
            let next = current + 1;
            let bytes = next.to_be_bytes();
            value[index - 1] = bytes[0];
            value[index] = bytes[1];
            return true;
        }
        value[index - 1] = 0;
        value[index] = 0;
    }
    false
}

fn decode_utf16be_or_utf8(value: &[u8]) -> String {
    if value.len() % 2 == 0 {
        let units = value.chunks_exact(2).map(|pair| u16::from_be_bytes([pair[0], pair[1]])).collect::<Vec<_>>();
        if let Ok(text) = String::from_utf16(&units) { return text; }
    }
    String::from_utf8_lossy(value).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{decode_pdf_string, decode_text_bytes, increment_unicode, parse_inline_jpeg, parse_to_unicode_cmap, split_pdf_paragraphs};

    #[test]
    fn preserves_leading_and_trailing_spaces_in_pdf_text_paragraphs() {
        let paragraphs = split_pdf_paragraphs("  leading\ttext  \nnext  ").collect::<Vec<_>>();
        assert_eq!(paragraphs, ["  leading\ttext  ", "next  "]);
    }

    #[test]
    fn literal_escapes_are_decoded_before_tounicode_mapping() {
        let map = parse_to_unicode_cmap(b"2 beginbfchar\n<01> <4F60>\n<02> <597D>\nendbfchar").expect("CMap");
        let maps = std::collections::HashMap::from([("F1".into(), map)]);
        assert_eq!(super::decode_with_font(br"\001\002", Some("F1"), &maps, true), "你好");
        assert_eq!(super::decode_with_font(&[1, 2], Some("F1"), &maps, false), "你好");
    }

    #[test]
    fn decodes_pdf_doc_encoding_and_line_continuations() {
        assert_eq!(decode_text_bytes(&[b'A', 0x80, 0x9f]), "A•€");
        assert_eq!(decode_pdf_string(b"line\\\ncontinued"), "linecontinued");
        assert_eq!(decode_pdf_string(b"line\\\r\ncontinued"), "linecontinued");
    }

    #[test]
    fn inline_jpeg_ignores_operator_like_bytes_inside_image() {
        let stream = b"BI /F /DCTDecode ID \xFF\xD8 payload EI (fake) Tj \xFF\xD9\nEI\nBT (real) Tj ET";
        let (image, end) = parse_inline_jpeg(stream, 0).expect("inline JPEG");
        assert_eq!(image.bytes, b"\xFF\xD8 payload EI (fake) Tj \xFF\xD9");
        assert_eq!(&stream[end..], b"\nBT (real) Tj ET");
    }

    #[test]
    fn inline_jpeg_requires_eoi_and_complete_operator_boundaries() {
        for stream in [
            &b"BI /F /DCTDecode ID \xFF\xD8 data EI "[..],
            &b"BI /F /DCTDecode ID \xFF\xD8\xFF\xD9 EIx "[..],
            &b"BI /F /DCTDecode IDx \xFF\xD8\xFF\xD9 EI "[..],
        ] {
            assert!(parse_inline_jpeg(stream, 0).is_none());
        }
    }

    #[test]
    fn decodes_surrogate_pair_bfrange_targets() {
        let map = parse_to_unicode_cmap(b"1 beginbfrange\n<01> <03> <D83DDE00>\nendbfrange").expect("CMap");
        assert_eq!(map.decode(&[1, 2, 3]), "😀😁😂");
    }

    #[test]
    fn decodes_multiple_code_units_in_range_targets() {
        let map = parse_to_unicode_cmap(b"1 beginbfrange\n<01> <02> <00660069>\nendbfrange").expect("CMap");
        assert_eq!(map.decode(&[1, 2]), "fifj");
    }

    #[test]
    fn codespace_ranges_control_multibyte_grouping() {
        let map = parse_to_unicode_cmap(b"1 begincodespacerange\n<0000> <00FF>\nendcodespacerange\n1 beginbfchar\n<0102> <4F60>\nendbfchar").expect("CMap");
        assert_eq!(map.decode(&[1, 2]), "你");
        assert_eq!(map.decode(&[1, 2, 3]), "你\u{FFFD}");
    }

    #[test]
    fn rejects_reversed_and_unbounded_ranges() {
        assert!(parse_to_unicode_cmap(b"1 beginbfrange\n<02> <01> <0041>\nendbfrange").is_none());
        assert!(parse_to_unicode_cmap(b"1 beginbfrange\n<00000000> <FFFFFFFF> <0041>\nendbfrange").is_none());
    }

    #[test]
    fn range_increment_carries_and_reports_overflow() {
        let mut value = vec![0x00, 0x01, 0xFF, 0xFF];
        assert!(increment_unicode(&mut value));
        assert_eq!(value, [0x00, 0x02, 0x00, 0x00]);
        assert!(!increment_unicode(&mut vec![0xFF, 0xFF]));
    }
}

fn object_id(bytes: &[u8], dictionary_start: usize) -> Option<u32> {
    let tokens = split_pdf_whitespace(&bytes[..dictionary_start]).rev();
    let mut tokens = tokens.skip(1);
    let _generation = tokens.next()?;
    std::str::from_utf8(tokens.next()?).ok()?.parse().ok()
}

fn page_content_ids(bytes: &[u8]) -> Vec<u32> {
    if let Some(catalog) = object_bodies(bytes).into_iter().find(|(_, body)| dictionary_type_is(body, b"/Catalog")) {
        if let Some(pages) = body_reference(&catalog.1, b"/Pages") {
            let mut result = Vec::new();
            collect_page_contents(bytes, pages, &mut result, &mut Vec::new());
            if !result.is_empty() { return result; }
        }
    }
    page_content_ids_by_scan(bytes)
}

fn collect_page_contents(bytes: &[u8], object: u32, result: &mut Vec<u32>, visited: &mut Vec<u32>) {
    if visited.contains(&object) { return; }
    visited.push(object);
    let Some(body) = object_bodies(bytes).into_iter().find(|(id, _)| *id == object).map(|(_, body)| body) else { return; };
    if dictionary_type_is(&body, b"/Page") {
        if let Some(contents) = body_reference_values(&body, b"/Contents") {
            for id in contents { if !result.contains(&id) { result.push(id); } }
        }
        return;
    }
    if let Some(kids) = body_reference_values(&body, b"/Kids") {
        for child in kids { collect_page_contents(bytes, child, result, visited); }
    }
}

fn body_reference(body: &[u8], key: &[u8]) -> Option<u32> {
    body_reference_values(body, key)?.into_iter().next()
}

fn dictionary_type_is(body: &[u8], expected: &[u8]) -> bool {
    let Some(position) = body.windows(5).position(|window| window == b"/Type") else { return false; };
    let start = skip_space(body, position + 5);
    let Some(value) = body.get(start..start + expected.len()) else { return false; };
    value == expected && body.get(start + expected.len()).is_none_or(|byte| byte.is_ascii_whitespace() || b"/<>[]()".contains(byte))
}

fn dictionary_subtype_is(body: &[u8], expected: &[u8]) -> bool {
    let Some(position) = body.windows(8).position(|window| window == b"/Subtype") else { return false; };
    let start = skip_space(body, position + 8);
    let Some(value) = body.get(start..start + expected.len()) else { return false; };
    value == expected && body.get(start + expected.len()).is_none_or(|byte| byte.is_ascii_whitespace() || b"/<>[]()".contains(byte))
}

fn body_reference_values(body: &[u8], key: &[u8]) -> Option<Vec<u32>> {
    let position = body.windows(key.len()).position(|window| window == key)?;
    let values = reference_ids(body.get(position + key.len()..)?);
    (!values.is_empty()).then_some(values)
}

fn object_bodies(bytes: &[u8]) -> Vec<(u32, Vec<u8>)> {
    let mut result = Vec::new();
    let mut search = 0;
    while let Some(relative) = bytes[search..].windows(4).position(|window| window == b" obj") {
        let marker = search + relative;
        let Some(id) = object_id_before_obj(bytes, marker) else { search = marker + 4; continue };
        let start = marker + 4;
        let end = bytes[start..].windows(6).position(|window| window == b"endobj").map_or(bytes.len(), |position| start + position);
        result.push((id, bytes[start..end].to_vec()));
        search = end.saturating_add(6);
        if search >= bytes.len() { break; }
    }
    result
}

fn page_content_ids_by_scan(bytes: &[u8]) -> Vec<u32> {
    let mut result = Vec::new();
    let mut search = 0;
    while let Some(relative) = bytes[search..].windows(4).position(|window| window == b" obj") {
        let marker = search + relative;
        let Some(id) = object_id_before_obj(bytes, marker) else { search = marker + 4; continue };
        let object_start = marker + 4;
        let end = bytes[marker + 4..].windows(6).position(|window| window == b"endobj").map_or(bytes.len(), |position| marker + 4 + position);
        let body = &bytes[object_start..end];
        if dictionary_type_is(body, b"/Page") {
            if let Some(contents) = body.windows(9).position(|window| window == b"/Contents") {
                for id in reference_ids(&body[contents + 9..]) {
                    if !result.contains(&id) { result.push(id); }
                }
            }
        }
        let _ = id;
        search = end.saturating_add(6);
        if search >= bytes.len() { break; }
    }
    result
}

fn object_id_before_obj(bytes: &[u8], marker: usize) -> Option<u32> {
    let mut tokens = split_pdf_whitespace(&bytes[..marker]).rev();
    let _generation = tokens.next()?;
    std::str::from_utf8(tokens.next()?).ok()?.parse().ok()
}

fn reference_ids(bytes: &[u8]) -> Vec<u32> {
    let end = bytes.iter().position(|byte| *byte == b'>').unwrap_or(bytes.len());
    let tokens = split_pdf_whitespace(&bytes[..end]).map(trim_pdf_brackets);
    let mut ids = Vec::new();
    let mut tokens = tokens.peekable();
    while let Some(token) = tokens.next() {
        if let Ok(id) = std::str::from_utf8(token).unwrap_or("").parse::<u32>() {
            if tokens.next().is_some_and(|generation| generation == b"0") && tokens.next().is_some_and(|reference| reference == b"R") {
                ids.push(id);
            }
        }
    }
    ids
}

fn split_pdf_whitespace(bytes: &[u8]) -> impl DoubleEndedIterator<Item = &[u8]> {
    bytes.split(|byte| byte.is_ascii_whitespace()).filter(|token| !token.is_empty())
}

fn trim_pdf_brackets(mut token: &[u8]) -> &[u8] {
    while token.first() == Some(&b'[') || token.first() == Some(&b']') { token = &token[1..]; }
    while token.last() == Some(&b'[') || token.last() == Some(&b']') { token = &token[..token.len() - 1]; }
    token
}

fn dictionary_value(dictionary: &[u8], key: &[u8]) -> Option<usize> {
    let start = dictionary.windows(key.len()).rposition(|window| window == key)? + key.len();
    let value = dictionary[start..].iter().position(|byte| !byte.is_ascii_whitespace()).map(|position| start + position)?;
    let end = dictionary[value..].iter().position(|byte| byte.is_ascii_whitespace() || *byte == b'>').map_or(dictionary.len(), |position| value + position);
    std::str::from_utf8(&dictionary[value..end]).ok()?.parse().ok()
}

fn decode_stream(dictionary: &[u8], stream: &[u8]) -> Result<Vec<u8>, FormatError> {
    if dictionary.windows(12).any(|window| window == b"/FlateDecode") {
        let mut decoder = flate2::read::ZlibDecoder::new(stream);
        let mut decoded = Vec::new();
        decoder.by_ref().take(32 * 1024 * 1024 + 1).read_to_end(&mut decoded).map_err(|error| FormatError::invalid_input(format!("invalid FlateDecode stream: {error}")))?;
        if decoded.len() > 32 * 1024 * 1024 {
            return Err(FormatError::unsupported("pdf", "decoded stream exceeds the 32 MiB limit"));
        }
        return Ok(decoded);
    }
    Ok(stream.to_vec())
}

fn extract_text_operators(stream: &[u8], font_maps: &HashMap<String, ToUnicodeMap>) -> ExtractedText {
    let mut events = Vec::new();
    let mut spacing_adjustments = false;
    let mut cursor = 0;
    let mut current_font = None;
    while cursor < stream.len() {
        if stream[cursor..].starts_with(b"BI") && (cursor == 0 || stream[cursor - 1].is_ascii_whitespace()) && stream.get(cursor + 2).is_some_and(|byte| byte.is_ascii_whitespace()) {
            if let Some((image, end)) = parse_inline_jpeg(stream, cursor) {
                events.push(ContentEvent::InlineImage(image));
                cursor = end;
                continue;
            }
        }
        match stream[cursor] {
            b'/' => {
                if let Some((font, end)) = text_font_operator(stream, cursor) {
                    current_font = Some(font);
                    cursor = end;
                } else if let Some((name, end)) = resource_operator(stream, cursor, "Do") {
                    events.push(ContentEvent::NamedImage(name));
                    cursor = end;
                } else { cursor += 1; }
            }
            b'(' => {
                let Some(close) = literal_close(stream, cursor + 1) else { break };
                let after = skip_content_space(stream, close + 1);
                if pdf_operator_at(stream, after, b"Tj") {
                    push_text_event(&mut events, decode_with_font(&stream[cursor + 1..close], current_font.as_deref(), font_maps, true));
                } else if pdf_operator_at(stream, after, b"'") || pdf_operator_at(stream, after, b"\"") {
                    push_text_event(&mut events, format!("\n{}", decode_with_font(&stream[cursor + 1..close], current_font.as_deref(), font_maps, true)));
                }
                cursor = close + 1;
            }
            b'<' if stream.get(cursor + 1) != Some(&b'<') => {
                let Some(close) = stream[cursor + 1..].iter().position(|byte| *byte == b'>').map(|position| cursor + 1 + position) else { break };
                let after = skip_content_space(stream, close + 1);
                let text = decode_with_font(&hex_bytes(&stream[cursor + 1..close]), current_font.as_deref(), font_maps, false);
                if pdf_operator_at(stream, after, b"Tj") { push_text_event(&mut events, text); }
                else if pdf_operator_at(stream, after, b"'") || pdf_operator_at(stream, after, b"\"") { push_text_event(&mut events, format!("\n{text}")); }
                cursor = close + 1;
            }
            b'[' => {
                let mut position = cursor + 1;
                let mut run = String::new();
                while position < stream.len() && stream[position] != b']' {
                    if stream[position] == b'(' {
                        let Some(end) = literal_close(stream, position + 1) else { break };
                        run.push_str(&decode_with_font(&stream[position + 1..end], current_font.as_deref(), font_maps, true));
                        position = end + 1;
                    } else if stream[position] == b'<' && stream.get(position + 1) != Some(&b'<') {
                        let Some(relative) = stream[position + 1..].iter().position(|byte| *byte == b'>') else { break };
                        let end = position + 1 + relative;
                        run.push_str(&decode_with_font(&hex_bytes(&stream[position + 1..end]), current_font.as_deref(), font_maps, false));
                        position = end + 1;
                    } else if stream[position] == b'%' { position = skip_pdf_comment(stream, position); }
                    else if is_pdf_number_start(stream[position]) {
                        spacing_adjustments = true;
                        position = skip_pdf_number(stream, position);
                    } else { position += 1; }
                }
                if position < stream.len() && pdf_operator_at(stream, skip_content_space(stream, position + 1), b"TJ") {
                    push_text_event(&mut events, run);
                }
                cursor = position.saturating_add(1);
            }
            b'T' if (cursor == 0 || pdf_delimiter(stream[cursor - 1])) && pdf_operator_at(stream, cursor, b"T*") => {
                push_text_event(&mut events, "\n".to_owned());
                cursor += 2;
            }
            b'%' => cursor = skip_pdf_comment(stream, cursor),
            _ => cursor += 1,
        }
    }
    ExtractedText { events, spacing_adjustments }
}

fn is_pdf_number_start(byte: u8) -> bool { byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.') }

fn skip_pdf_number(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes.get(cursor).is_some_and(|byte| byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.')) { cursor += 1; }
    cursor
}

fn parse_inline_jpeg(stream: &[u8], cursor: usize) -> Option<(PdfImage, usize)> {
    let dictionary_start = cursor.checked_add(2)?;
    let mut id = None;
    for index in dictionary_start..stream.len().saturating_sub(1) {
        if &stream[index..index + 2] == b"ID"
            && (index == dictionary_start || stream[index - 1].is_ascii_whitespace())
            && stream.get(index + 2).is_some_and(|byte| byte.is_ascii_whitespace())
        {
            id = Some(index);
            break;
        }
    }
    let id = id?;
    let dictionary = &stream[dictionary_start..id];
    if !dictionary.windows(9).any(|window| window == b"DCTDecode") { return None; }
    let data_start = skip_space(stream, id + 2);
    if !stream.get(data_start..)?.starts_with(&[0xFF, 0xD8]) { return None; }
    let mut search = data_start + 2;
    while let Some(relative) = stream[search..].windows(2).position(|window| window == [0xFF, 0xD9]) {
        let data_end = search + relative + 2;
        let end = skip_space(stream, data_end);
        let after = end + 2;
        if end > data_end && stream.get(end..after) == Some(b"EI") && stream.get(after).is_none_or(|byte| byte.is_ascii_whitespace()) {
            return Some((PdfImage { bytes: stream[data_start..data_end].to_vec(), media_type: "image/jpeg", source: format!("pdf-inline-image-{data_start}") }, after));
        }
        search = data_end;
    }
    None
}

fn text_font_operator(stream: &[u8], cursor: usize) -> Option<(String, usize)> {
    let name_end = stream[cursor + 1..].iter().position(|byte| byte.is_ascii_whitespace()).map(|position| cursor + 1 + position)?;
    let name = std::str::from_utf8(&stream[cursor + 1..name_end]).ok()?.to_owned();
    let number_start = skip_space(stream, name_end);
    let number_end = stream[number_start..].iter().position(|byte| byte.is_ascii_whitespace()).map(|position| number_start + position)?;
    let operator = skip_space(stream, number_end);
    pdf_operator_at(stream, operator, b"Tf").then_some((name, operator + 2))
}

fn resource_operator(stream: &[u8], cursor: usize, expected: &str) -> Option<(String, usize)> {
    let name_end = stream[cursor + 1..].iter().position(|byte| byte.is_ascii_whitespace()).map(|position| cursor + 1 + position)?;
    let name = std::str::from_utf8(&stream[cursor + 1..name_end]).ok()?.to_owned();
    let operator_start = skip_space(stream, name_end);
    let operator_end = operator_start.checked_add(expected.len())?;
    pdf_operator_at(stream, operator_start, expected.as_bytes()).then_some((name, operator_end))
}

fn decode_with_font(bytes: &[u8], font: Option<&str>, maps: &HashMap<String, ToUnicodeMap>, literal: bool) -> String {
    if let Some(font) = font.and_then(|name| maps.get(name)) {
        return if literal { font.decode(&unescape_pdf_literal(bytes)) } else { font.decode(bytes) };
    }
    if literal { decode_pdf_string(bytes) } else { decode_text_bytes(bytes) }
}

fn body_contains_name(body: &[u8], name: &[u8]) -> bool {
    body.windows(name.len()).any(|window| window == name)
}

fn win_ansi_encoding(byte: u8) -> char {
    match byte {
        0x80 => '€', 0x82 => '‚', 0x83 => 'ƒ', 0x84 => '„', 0x85 => '…', 0x86 => '†', 0x87 => '‡',
        0x88 => 'ˆ', 0x89 => '‰', 0x8a => 'Š', 0x8b => '‹', 0x8c => 'Œ', 0x8e => 'Ž',
        0x91 => '‘', 0x92 => '’', 0x93 => '“', 0x94 => '”', 0x95 => '•', 0x96 => '–', 0x97 => '—',
        0x98 => '˜', 0x99 => '™', 0x9a => 'š', 0x9b => '›', 0x9c => 'œ', 0x9e => 'ž', 0x9f => 'Ÿ',
        0x81 | 0x8d | 0x8f | 0x90 | 0x9d => '\u{fffd}',
        value => char::from_u32(u32::from(value)).unwrap_or('\u{fffd}'),
    }
}

fn push_text_event(events: &mut Vec<ContentEvent>, text: String) {
    if text.is_empty() { return; }
    events.push(ContentEvent::Text(text));
}
fn skip_space(bytes: &[u8], mut cursor: usize) -> usize { while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() { cursor += 1; } cursor }

fn pdf_delimiter(byte: u8) -> bool { byte.is_ascii_whitespace() || b"()<>[]{}/%".contains(&byte) || byte == 0 }

fn pdf_operator_at(bytes: &[u8], cursor: usize, operator: &[u8]) -> bool {
    let Some(end) = cursor.checked_add(operator.len()) else { return false };
    bytes.get(cursor..end) == Some(operator) && bytes.get(end).is_none_or(|byte| pdf_delimiter(*byte))
}

fn skip_pdf_comment(bytes: &[u8], cursor: usize) -> usize {
    bytes[cursor..].iter().position(|byte| matches!(byte, b'\r' | b'\n')).map_or(bytes.len(), |offset| cursor + offset)
}

fn skip_content_space(bytes: &[u8], mut cursor: usize) -> usize {
    loop {
        while bytes.get(cursor).is_some_and(|byte| byte.is_ascii_whitespace() || *byte == 0) { cursor += 1; }
        if bytes.get(cursor) != Some(&b'%') { return cursor; }
        cursor = skip_pdf_comment(bytes, cursor);
    }
}

fn literal_close(bytes: &[u8], mut cursor: usize) -> Option<usize> {
    let mut depth = 1;
    while cursor < bytes.len() { match bytes[cursor] { b'\\' => cursor += 2, b'(' => { depth += 1; cursor += 1; }, b')' => { depth -= 1; if depth == 0 { return Some(cursor); } cursor += 1; }, _ => cursor += 1 } }
    None
}

fn decode_pdf_string(value: &[u8]) -> String {
    decode_text_bytes(&unescape_pdf_literal(value))
}

fn unescape_pdf_literal(value: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(value.len());
    let mut cursor = 0;
    while cursor < value.len() {
        if value[cursor] != b'\\' { decoded.push(value[cursor]); cursor += 1; continue; }
        cursor += 1; if cursor == value.len() { break; }
        match value[cursor] { b'n' => decoded.push(b'\n'), b'r' => decoded.push(b'\r'), b't' => decoded.push(b'\t'), b'b' => decoded.push(8), b'f' => decoded.push(12), b'\r' => { if value.get(cursor + 1) == Some(&b'\n') { cursor += 1; } }, b'\n' => {}, b'0'..=b'7' => { let start = cursor; cursor += 1; while cursor < value.len() && cursor < start + 3 && (b'0'..=b'7').contains(&value[cursor]) { cursor += 1; } if let Ok(number) = u8::from_str_radix(std::str::from_utf8(&value[start..cursor]).unwrap_or("0"), 8) { decoded.push(number); } continue; }, escaped => decoded.push(escaped) }
        cursor += 1;
    }
    decoded
}

fn decode_text_bytes(decoded: &[u8]) -> String {
    if decoded.starts_with(&[0xfe, 0xff]) { let units = decoded[2..].chunks_exact(2).map(|pair| u16::from_be_bytes([pair[0], pair[1]])).collect::<Vec<_>>(); return String::from_utf16_lossy(&units); }
    decoded.iter().map(|byte| pdf_doc_encoding(*byte)).collect()
}

fn pdf_doc_encoding(byte: u8) -> char {
    match byte {
        0x18 => '˘', 0x19 => 'ˇ', 0x1a => 'ˆ', 0x1b => '˙', 0x1c => '˝', 0x1d => '˛', 0x1e => '˚', 0x1f => '˜',
        0x80 => '•', 0x81 => '†', 0x82 => '‡', 0x83 => '…', 0x84 => '—', 0x85 => '–', 0x86 => 'ƒ', 0x87 => '⁄',
        0x88 => '‹', 0x89 => '›', 0x8a => '−', 0x8b => '‰', 0x8c => '„', 0x8d => '“', 0x8e => '”', 0x8f => '‘',
        0x90 => '’', 0x91 => '‚', 0x92 => '™', 0x93 => 'ﬁ', 0x94 => 'ﬂ', 0x95 => 'Ł', 0x96 => 'Œ', 0x97 => 'Š',
        0x98 => 'Ÿ', 0x99 => 'Ž', 0x9a => 'ı', 0x9b => 'ł', 0x9c => 'œ', 0x9d => 'š', 0x9e => 'ž', 0x9f => '€',
        value => char::from_u32(u32::from(value)).unwrap_or('\u{FFFD}')
    }
}

fn hex_bytes(value: &[u8]) -> Vec<u8> {
    let mut result = Vec::new(); let mut high = None;
    for byte in value.iter().copied().filter(|byte| !byte.is_ascii_whitespace()) { let Some(nibble) = (byte as char).to_digit(16).map(|number| number as u8) else { continue }; if let Some(previous) = high.take() { result.push((previous << 4) | nibble); } else { high = Some(nibble); } }
    if let Some(previous) = high { result.push(previous << 4); } result
}
