use std::{
    collections::HashMap,
    io::{Read, Write},
};

use crate::FormatError;

use super::{
    objects::{dictionary_subtype_is, dictionary_type_is, object_bodies, page_resource_sections, resource_font_references},
    syntax::{dictionary_value, skip_space},
    types::{PdfImage, PdfStream},
};

pub(crate) fn page_images(bytes: &[u8], streams: &[PdfStream<'_>]) -> Result<HashMap<String, PdfImage>, FormatError> {
    let bodies = object_bodies(bytes);
    let mut result = HashMap::new();
    for resources in page_resource_sections(bytes) {
        let Some(xobject_start) = resources.windows(8).position(|window| window == b"/XObject")
        else {
            continue;
        };
        for (name, object_id) in resource_font_references(&resources[xobject_start + 8..]) {
            let Some((_, image_body)) = bodies.iter().find(|(id, _)| *id == object_id)
            else {
                continue;
            };
            if !dictionary_type_is(image_body, b"/Image") && !dictionary_subtype_is(image_body, b"/Image") {
                continue;
            }
            let Some(stream) = streams.iter().find(|stream| stream.object_id == Some(object_id))
            else {
                continue;
            };
            let source = format!("pdf-object-{object_id}");
            let image = if image_body.windows(10).any(|window| window == b"/DCTDecode") {
                PdfImage { bytes: stream.data.to_vec(), media_type: "image/jpeg", source }
            }
            else if image_body.windows(12).any(|window| window == b"/FlateDecode") {
                let bytes = decode_pdf_image_png(image_body, stream.data)?;
                PdfImage { bytes, media_type: "image/png", source }
            }
            else {
                continue;
            };
            result.insert(name, image);
        }
    }
    Ok(result)
}

pub(crate) fn decode_pdf_image_png(dictionary: &[u8], stream: &[u8]) -> Result<Vec<u8>, FormatError> {
    let width = dictionary_value(dictionary, b"/Width")
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| FormatError::parse("pdf", "Flate image has invalid /Width"))?;
    let height = dictionary_value(dictionary, b"/Height")
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| FormatError::parse("pdf", "Flate image has invalid /Height"))?;
    let bits = dictionary_value(dictionary, b"/BitsPerComponent").unwrap_or(8);
    let color_position = dictionary
        .windows(11)
        .position(|window| window == b"/ColorSpace")
        .ok_or_else(|| FormatError::unsupported("pdf", "Flate image requires a direct grayscale or RGB colorspace"))?;
    let color_start = skip_space(dictionary, color_position + 11);
    let color_end = dictionary[color_start..]
        .iter()
        .position(|byte| byte.is_ascii_whitespace() || matches!(byte, b'/' | b'>' | b'['))
        .map_or(dictionary.len(), |position| color_start + position);
    let color_end = if dictionary.get(color_start) == Some(&b'/') {
        dictionary[color_start + 1..]
            .iter()
            .position(|byte| byte.is_ascii_whitespace() || matches!(byte, b'/' | b'>' | b'['))
            .map_or(dictionary.len(), |position| color_start + 1 + position)
    }
    else {
        color_end
    };
    let colors = match &dictionary[color_start..color_end] {
        b"/DeviceGray" => 1,
        b"/DeviceRGB" => 3,
        _ => return Err(FormatError::unsupported("pdf", "Flate image requires a direct grayscale or RGB colorspace")),
    };
    if dictionary_value(dictionary, b"/Colors").is_some_and(|value| value != colors) {
        return Err(FormatError::unsupported("pdf", "Flate image predictor colors disagree with colorspace"));
    }
    if width == 0 || height == 0 || bits != 8 || !matches!(colors, 1 | 3) {
        return Err(FormatError::unsupported("pdf", "Flate image requires non-zero 8-bit grayscale or RGB dimensions"));
    }
    let mut decoder = flate2::read::ZlibDecoder::new(stream);
    let mut filtered = Vec::new();
    decoder
        .by_ref()
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut filtered)
        .map_err(|error| FormatError::parse("pdf", format!("invalid Flate image: {error}")))?;
    if filtered.len() > 32 * 1024 * 1024 {
        return Err(FormatError::unsupported("pdf", "decoded image exceeds the 32 MiB limit"));
    }
    let row_bytes = usize::try_from(width)
        .ok()
        .and_then(|value| value.checked_mul(colors))
        .ok_or_else(|| FormatError::unsupported("pdf", "Flate image row is too large"))?;
    let predictor = dictionary_value(dictionary, b"/Predictor").unwrap_or(1);
    let rows = if predictor == 15 {
        let expected = usize::try_from(height)
            .ok()
            .and_then(|value| value.checked_mul(row_bytes + 1))
            .ok_or_else(|| FormatError::unsupported("pdf", "Flate image is too large"))?;
        if filtered.len() != expected {
            return Err(FormatError::parse("pdf", "Flate image data length does not match dimensions"));
        }
        unfilter_png_rows(&filtered, row_bytes, colors)?
    }
    else if predictor == 1 {
        let expected = usize::try_from(height)
            .ok()
            .and_then(|value| value.checked_mul(row_bytes))
            .ok_or_else(|| FormatError::unsupported("pdf", "Flate image is too large"))?;
        if filtered.len() != expected {
            return Err(FormatError::parse("pdf", "Flate image data length does not match dimensions"));
        }
        filtered.chunks_exact(row_bytes).map(|row| row.to_vec()).collect()
    }
    else {
        return Err(FormatError::unsupported("pdf", "unsupported Flate image predictor"));
    };
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
            current[offset] = match filter {
                0 => current[offset],
                1 => current[offset].wrapping_add(left),
                2 => current[offset].wrapping_add(up),
                3 => current[offset].wrapping_add(((u16::from(left) + u16::from(up)) / 2) as u8),
                4 => current[offset].wrapping_add(paeth(left, up, upper_left)),
                _ => return Err(FormatError::unsupported("pdf", "unsupported PNG predictor row filter")),
            };
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
    if pa <= pb && pa <= pc {
        left
    }
    else if pb <= pc {
        up
    }
    else {
        upper_left
    }
}

fn encode_png(width: u32, height: u32, colors: u8, raw: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    for row in raw.chunks_exact(width as usize * colors as usize) {
        let _ = encoder.write_all(&[0]);
        let _ = encoder.write_all(row);
    }
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
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    output.extend_from_slice(&(crc ^ 0xffff_ffff).to_be_bytes());
}
pub(crate) fn parse_inline_jpeg(stream: &[u8], cursor: usize) -> Option<(PdfImage, usize)> {
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
    if !dictionary.windows(9).any(|window| window == b"DCTDecode") {
        return None;
    }
    let data_start = skip_space(stream, id + 2);
    if !stream.get(data_start..)?.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut search = data_start + 2;
    while let Some(relative) = stream[search..].windows(2).position(|window| window == [0xFF, 0xD9]) {
        let data_end = search + relative + 2;
        let end = skip_space(stream, data_end);
        let after = end + 2;
        if end > data_end && stream.get(end..after) == Some(b"EI") && stream.get(after).is_none_or(|byte| byte.is_ascii_whitespace()) {
            return Some((
                PdfImage {
                    bytes: stream[data_start..data_end].to_vec(),
                    media_type: "image/jpeg",
                    source: format!("pdf-inline-image-{data_start}"),
                },
                after,
            ));
        }
        search = data_end;
    }
    None
}
