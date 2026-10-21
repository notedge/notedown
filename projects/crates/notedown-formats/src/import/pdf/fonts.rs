use std::collections::HashMap;

use crate::FormatError;

use super::{
    encoding::win_ansi_encoding,
    objects::{body_contains_name, body_reference, object_bodies, page_resource_sections, resource_font_references},
    streams::decode_stream,
    syntax::hex_bytes,
    types::PdfStream,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct ToUnicodeMap {
    entries: HashMap<Vec<u8>, String>,
    max_code_len: usize,
    code_lengths: Vec<usize>,
    winansi_fallback: bool,
}

impl ToUnicodeMap {
    pub(crate) fn decode(&self, bytes: &[u8]) -> String {
        if self.entries.is_empty() && self.winansi_fallback {
            return bytes.iter().map(|byte| win_ansi_encoding(*byte)).collect();
        }
        let mut output = String::new();
        let mut cursor = 0;
        while cursor < bytes.len() {
            let mut found = None;
            for length in (1..=self.max_code_len.min(bytes.len() - cursor)).rev() {
                if !self.code_lengths.is_empty() && !self.code_lengths.contains(&length) {
                    continue;
                }
                if let Some(value) = self.entries.get(&bytes[cursor..cursor + length]) {
                    found = Some((length, value));
                    break;
                }
            }
            if let Some((length, value)) = found {
                output.push_str(value);
                cursor += length;
            }
            else {
                output.push('\u{FFFD}');
                cursor += 1;
            }
        }
        output
    }
}
pub(crate) fn page_font_maps(bytes: &[u8], streams: &[PdfStream<'_>]) -> Result<HashMap<String, ToUnicodeMap>, FormatError> {
    let bodies = object_bodies(bytes);
    let mut result = HashMap::new();
    for resources in page_resource_sections(bytes) {
        let Some(font_start) = resources.windows(5).position(|window| window == b"/Font")
        else {
            continue;
        };
        let section = &resources[font_start + 5..];
        for (name, object_id) in resource_font_references(section) {
            let Some((_, font_body)) = bodies.iter().find(|(id, _)| *id == object_id)
            else {
                continue;
            };
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
pub(crate) fn parse_to_unicode_cmap(bytes: &[u8]) -> Option<ToUnicodeMap> {
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
                    if index + 1 >= tokens.len() || tokens[index] == "endcodespacerange" {
                        break;
                    }
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
                    if index + 1 >= tokens.len() || tokens[index] == "endbfchar" {
                        break;
                    }
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
                    if index + 2 >= tokens.len() || tokens[index] == "endbfrange" {
                        break;
                    }
                    let source_start = parse_hex_token(tokens[index]);
                    let source_end = parse_hex_token(tokens[index + 1]);
                    if let (Some(start), Some(end)) = (source_start, source_end) {
                        let destination = tokens[index + 2];
                        if destination.starts_with('<') {
                            if let Some(base) = parse_hex_token(destination) {
                                add_cmap_range(&mut map, start, end, base);
                            }
                        }
                        else if destination == "[" {
                            let mut source = start.clone();
                            let mut destination_index = index + 3;
                            while source <= end && destination_index < tokens.len() && tokens[destination_index] != "]" {
                                if let Some(value) = parse_hex_token(tokens[destination_index]) {
                                    add_cmap_entry(&mut map, source.clone(), value);
                                }
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

pub(crate) fn pdf_tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split_ascii_whitespace().filter(|token| !token.is_empty())
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
    if start.is_empty() || start.len() != end.len() || destination.is_empty() || destination.len() % 2 != 0 || start.len() > 4 {
        return;
    }
    let source_start = start.iter().fold(0u32, |value, byte| (value << 8) | u32::from(*byte));
    let source_end = end.iter().fold(0u32, |value, byte| (value << 8) | u32::from(*byte));
    if source_end < source_start || source_end - source_start >= 65_536 {
        return;
    }
    for offset in 0..=source_end.saturating_sub(source_start) {
        let source_value = source_start + offset;
        let source_len = start.len();
        let mut source = vec![0u8; source_len];
        for (index, byte) in source.iter_mut().enumerate() {
            *byte = (source_value >> (8 * (source_len - index - 1))) as u8;
        }
        add_cmap_entry(map, source, destination.clone());
        if offset < source_end.saturating_sub(source_start) && !increment_unicode(&mut destination) {
            break;
        }
    }
}

fn increment_code(value: &mut Vec<u8>) {
    for byte in value.iter_mut().rev() {
        if *byte < 0xff {
            *byte += 1;
            return;
        }
        *byte = 0;
    }
}

pub(crate) fn increment_unicode(value: &mut Vec<u8>) -> bool {
    if value.len() < 2 || value.len() % 2 != 0 {
        return false;
    }
    for index in (0..value.len()).rev().step_by(2) {
        if index == 0 {
            break;
        }
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
        if let Ok(text) = String::from_utf16(&units) {
            return text;
        }
    }
    String::from_utf8_lossy(value).into_owned()
}
