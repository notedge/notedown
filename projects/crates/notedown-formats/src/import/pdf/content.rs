use std::collections::HashMap;

use super::{
    encoding::decode_with_font,
    fonts::ToUnicodeMap,
    images::parse_inline_jpeg,
    syntax::{
        hex_bytes, is_pdf_number_start, literal_close, pdf_delimiter, pdf_operator_at, skip_content_space, skip_pdf_comment, skip_pdf_number,
        skip_space,
    },
    types::{ContentEvent, ExtractedText},
};

pub(crate) fn extract_text_operators(stream: &[u8], font_maps: &HashMap<String, ToUnicodeMap>) -> ExtractedText {
    let mut events = Vec::new();
    let mut spacing_adjustments = false;
    let mut cursor = 0;
    let mut current_font = None;
    while cursor < stream.len() {
        if stream[cursor..].starts_with(b"BI")
            && (cursor == 0 || stream[cursor - 1].is_ascii_whitespace())
            && stream.get(cursor + 2).is_some_and(|byte| byte.is_ascii_whitespace())
        {
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
                }
                else if let Some((name, end)) = resource_operator(stream, cursor, "Do") {
                    events.push(ContentEvent::NamedImage(name));
                    cursor = end;
                }
                else {
                    cursor += 1;
                }
            }
            b'(' => {
                let Some(close) = literal_close(stream, cursor + 1)
                else {
                    break;
                };
                let after = skip_content_space(stream, close + 1);
                if pdf_operator_at(stream, after, b"Tj") {
                    push_text_event(&mut events, decode_with_font(&stream[cursor + 1..close], current_font.as_deref(), font_maps, true));
                }
                else if pdf_operator_at(stream, after, b"'") || pdf_operator_at(stream, after, b"\"") {
                    push_text_event(
                        &mut events,
                        format!("\n{}", decode_with_font(&stream[cursor + 1..close], current_font.as_deref(), font_maps, true)),
                    );
                }
                cursor = close + 1;
            }
            b'<' if stream.get(cursor + 1) != Some(&b'<') => {
                let Some(close) = stream[cursor + 1..].iter().position(|byte| *byte == b'>').map(|position| cursor + 1 + position)
                else {
                    break;
                };
                let after = skip_content_space(stream, close + 1);
                let text = decode_with_font(&hex_bytes(&stream[cursor + 1..close]), current_font.as_deref(), font_maps, false);
                if pdf_operator_at(stream, after, b"Tj") {
                    push_text_event(&mut events, text);
                }
                else if pdf_operator_at(stream, after, b"'") || pdf_operator_at(stream, after, b"\"") {
                    push_text_event(&mut events, format!("\n{text}"));
                }
                cursor = close + 1;
            }
            b'[' => {
                let mut position = cursor + 1;
                let mut run = String::new();
                while position < stream.len() && stream[position] != b']' {
                    if stream[position] == b'(' {
                        let Some(end) = literal_close(stream, position + 1)
                        else {
                            break;
                        };
                        run.push_str(&decode_with_font(&stream[position + 1..end], current_font.as_deref(), font_maps, true));
                        position = end + 1;
                    }
                    else if stream[position] == b'<' && stream.get(position + 1) != Some(&b'<') {
                        let Some(relative) = stream[position + 1..].iter().position(|byte| *byte == b'>')
                        else {
                            break;
                        };
                        let end = position + 1 + relative;
                        run.push_str(&decode_with_font(&hex_bytes(&stream[position + 1..end]), current_font.as_deref(), font_maps, false));
                        position = end + 1;
                    }
                    else if stream[position] == b'%' {
                        position = skip_pdf_comment(stream, position);
                    }
                    else if is_pdf_number_start(stream[position]) {
                        spacing_adjustments = true;
                        position = skip_pdf_number(stream, position);
                    }
                    else {
                        position += 1;
                    }
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
fn push_text_event(events: &mut Vec<ContentEvent>, text: String) {
    if text.is_empty() {
        return;
    }
    events.push(ContentEvent::Text(text));
}
