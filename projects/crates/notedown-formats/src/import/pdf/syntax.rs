pub(crate) fn split_pdf_whitespace(bytes: &[u8]) -> impl DoubleEndedIterator<Item = &[u8]> {
    bytes.split(|byte| byte.is_ascii_whitespace()).filter(|token| !token.is_empty())
}

pub(crate) fn trim_pdf_brackets(mut token: &[u8]) -> &[u8] {
    while token.first() == Some(&b'[') || token.first() == Some(&b']') {
        token = &token[1..];
    }
    while token.last() == Some(&b'[') || token.last() == Some(&b']') {
        token = &token[..token.len() - 1];
    }
    token
}

pub(crate) fn dictionary_value(dictionary: &[u8], key: &[u8]) -> Option<usize> {
    let start = dictionary.windows(key.len()).rposition(|window| window == key)? + key.len();
    let value = dictionary[start..].iter().position(|byte| !byte.is_ascii_whitespace()).map(|position| start + position)?;
    let end = dictionary[value..]
        .iter()
        .position(|byte| byte.is_ascii_whitespace() || *byte == b'>')
        .map_or(dictionary.len(), |position| value + position);
    std::str::from_utf8(&dictionary[value..end]).ok()?.parse().ok()
}
pub(crate) fn skip_space(bytes: &[u8], mut cursor: usize) -> usize {
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    cursor
}

pub(crate) fn pdf_delimiter(byte: u8) -> bool {
    byte.is_ascii_whitespace() || b"()<>[]{}/%".contains(&byte) || byte == 0
}

pub(crate) fn pdf_operator_at(bytes: &[u8], cursor: usize, operator: &[u8]) -> bool {
    let Some(end) = cursor.checked_add(operator.len())
    else {
        return false;
    };
    bytes.get(cursor..end) == Some(operator) && bytes.get(end).is_none_or(|byte| pdf_delimiter(*byte))
}

pub(crate) fn skip_pdf_comment(bytes: &[u8], cursor: usize) -> usize {
    bytes[cursor..].iter().position(|byte| matches!(byte, b'\r' | b'\n')).map_or(bytes.len(), |offset| cursor + offset)
}

pub(crate) fn skip_content_space(bytes: &[u8], mut cursor: usize) -> usize {
    loop {
        while bytes.get(cursor).is_some_and(|byte| byte.is_ascii_whitespace() || *byte == 0) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'%') {
            return cursor;
        }
        cursor = skip_pdf_comment(bytes, cursor);
    }
}

pub(crate) fn literal_close(bytes: &[u8], mut cursor: usize) -> Option<usize> {
    let mut depth = 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor += 2,
            b'(' => {
                depth += 1;
                cursor += 1;
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(cursor);
                }
                cursor += 1;
            }
            _ => cursor += 1,
        }
    }
    None
}

pub(crate) fn is_pdf_number_start(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.')
}

pub(crate) fn skip_pdf_number(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes.get(cursor).is_some_and(|byte| byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.')) {
        cursor += 1;
    }
    cursor
}

pub(crate) fn hex_bytes(value: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut high = None;
    for byte in value.iter().copied().filter(|byte| !byte.is_ascii_whitespace()) {
        let Some(nibble) = (byte as char).to_digit(16).map(|number| number as u8)
        else {
            continue;
        };
        if let Some(previous) = high.take() {
            result.push((previous << 4) | nibble);
        }
        else {
            high = Some(nibble);
        }
    }
    if let Some(previous) = high {
        result.push(previous << 4);
    }
    result
}
