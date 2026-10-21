use std::collections::HashMap;

use super::fonts::ToUnicodeMap;
pub(crate) fn decode_with_font(bytes: &[u8], font: Option<&str>, maps: &HashMap<String, ToUnicodeMap>, literal: bool) -> String {
    if let Some(font) = font.and_then(|name| maps.get(name)) {
        return if literal { font.decode(&unescape_pdf_literal(bytes)) } else { font.decode(bytes) };
    }
    if literal { decode_pdf_string(bytes) } else { decode_text_bytes(bytes) }
}
pub(crate) fn win_ansi_encoding(byte: u8) -> char {
    match byte {
        0x80 => '€',
        0x82 => '‚',
        0x83 => 'ƒ',
        0x84 => '„',
        0x85 => '…',
        0x86 => '†',
        0x87 => '‡',
        0x88 => 'ˆ',
        0x89 => '‰',
        0x8a => 'Š',
        0x8b => '‹',
        0x8c => 'Œ',
        0x8e => 'Ž',
        0x91 => '‘',
        0x92 => '’',
        0x93 => '“',
        0x94 => '”',
        0x95 => '•',
        0x96 => '–',
        0x97 => '—',
        0x98 => '˜',
        0x99 => '™',
        0x9a => 'š',
        0x9b => '›',
        0x9c => 'œ',
        0x9e => 'ž',
        0x9f => 'Ÿ',
        0x81 | 0x8d | 0x8f | 0x90 | 0x9d => '\u{fffd}',
        value => char::from_u32(u32::from(value)).unwrap_or('\u{fffd}'),
    }
}
pub(crate) fn decode_pdf_string(value: &[u8]) -> String {
    decode_text_bytes(&unescape_pdf_literal(value))
}

fn unescape_pdf_literal(value: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(value.len());
    let mut cursor = 0;
    while cursor < value.len() {
        if value[cursor] != b'\\' {
            decoded.push(value[cursor]);
            cursor += 1;
            continue;
        }
        cursor += 1;
        if cursor == value.len() {
            break;
        }
        match value[cursor] {
            b'n' => decoded.push(b'\n'),
            b'r' => decoded.push(b'\r'),
            b't' => decoded.push(b'\t'),
            b'b' => decoded.push(8),
            b'f' => decoded.push(12),
            b'\r' => {
                if value.get(cursor + 1) == Some(&b'\n') {
                    cursor += 1;
                }
            }
            b'\n' => {}
            b'0'..=b'7' => {
                let start = cursor;
                cursor += 1;
                while cursor < value.len() && cursor < start + 3 && (b'0'..=b'7').contains(&value[cursor]) {
                    cursor += 1;
                }
                if let Ok(number) = u8::from_str_radix(std::str::from_utf8(&value[start..cursor]).unwrap_or("0"), 8) {
                    decoded.push(number);
                }
                continue;
            }
            escaped => decoded.push(escaped),
        }
        cursor += 1;
    }
    decoded
}

pub(crate) fn decode_text_bytes(decoded: &[u8]) -> String {
    if decoded.starts_with(&[0xfe, 0xff]) {
        let units = decoded[2..].chunks_exact(2).map(|pair| u16::from_be_bytes([pair[0], pair[1]])).collect::<Vec<_>>();
        return String::from_utf16_lossy(&units);
    }
    decoded.iter().map(|byte| pdf_doc_encoding(*byte)).collect()
}

fn pdf_doc_encoding(byte: u8) -> char {
    match byte {
        0x18 => '˘',
        0x19 => 'ˇ',
        0x1a => 'ˆ',
        0x1b => '˙',
        0x1c => '˝',
        0x1d => '˛',
        0x1e => '˚',
        0x1f => '˜',
        0x80 => '•',
        0x81 => '†',
        0x82 => '‡',
        0x83 => '…',
        0x84 => '—',
        0x85 => '–',
        0x86 => 'ƒ',
        0x87 => '⁄',
        0x88 => '‹',
        0x89 => '›',
        0x8a => '−',
        0x8b => '‰',
        0x8c => '„',
        0x8d => '“',
        0x8e => '”',
        0x8f => '‘',
        0x90 => '’',
        0x91 => '‚',
        0x92 => '™',
        0x93 => 'ﬁ',
        0x94 => 'ﬂ',
        0x95 => 'Ł',
        0x96 => 'Œ',
        0x97 => 'Š',
        0x98 => 'Ÿ',
        0x99 => 'Ž',
        0x9a => 'ı',
        0x9b => 'ł',
        0x9c => 'œ',
        0x9d => 'š',
        0x9e => 'ž',
        0x9f => '€',
        value => char::from_u32(u32::from(value)).unwrap_or('\u{FFFD}'),
    }
}
