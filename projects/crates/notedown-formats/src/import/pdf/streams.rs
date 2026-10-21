use std::io::Read;

use crate::FormatError;

use super::{
    objects::object_id,
    syntax::{dictionary_value, skip_space},
    types::PdfStream,
};

pub(crate) fn stream_contents(bytes: &[u8]) -> Vec<PdfStream<'_>> {
    let mut streams = Vec::new();
    let mut offset = 0;
    while let Some(relative) = bytes[offset..].windows(6).position(|window| window == b"stream") {
        let marker = offset + relative;
        let dictionary_start = bytes[..marker].windows(3).rposition(|window| window == b"obj").map_or(0, |position| position + 3);
        let object_id = object_id(bytes, dictionary_start);
        let dictionary = &bytes[dictionary_start..marker];
        let mut start = marker + 6;
        start = if bytes.get(start..start + 2) == Some(b"\r\n") {
            start + 2
        }
        else if bytes.get(start..start + 1) == Some(b"\n") {
            start + 1
        }
        else {
            offset = start;
            continue;
        };
        let end = dictionary_value(dictionary, b"/Length")
            .and_then(|length| start.checked_add(length).filter(|end| *end <= bytes.len()))
            .filter(|end| bytes[skip_space(bytes, *end)..].starts_with(b"endstream"))
            .unwrap_or_else(|| {
                bytes[start..].windows(9).position(|window| window == b"endstream").map_or(bytes.len(), |position| start + position)
            });
        if end > start {
            streams.push(PdfStream { object_id, dictionary, data: &bytes[start..end] });
        }
        offset = skip_space(bytes, end).saturating_add(9);
        if offset >= bytes.len() {
            break;
        }
    }
    streams
}
pub(crate) fn decode_stream(dictionary: &[u8], stream: &[u8]) -> Result<Vec<u8>, FormatError> {
    if dictionary.windows(12).any(|window| window == b"/FlateDecode") {
        let mut decoder = flate2::read::ZlibDecoder::new(stream);
        let mut decoded = Vec::new();
        decoder
            .by_ref()
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut decoded)
            .map_err(|error| FormatError::invalid_input(format!("invalid FlateDecode stream: {error}")))?;
        if decoded.len() > 32 * 1024 * 1024 {
            return Err(FormatError::unsupported("pdf", "decoded stream exceeds the 32 MiB limit"));
        }
        return Ok(decoded);
    }
    Ok(stream.to_vec())
}
