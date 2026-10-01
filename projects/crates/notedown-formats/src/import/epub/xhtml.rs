use notedown_ir::{Block, Inline};

use crate::FormatError;

/// Best-effort XHTML block extraction until `oak-html` lowering lands.
pub fn blocks_from_xhtml(xml: &[u8]) -> Result<Vec<Block>, FormatError> {
    let text = String::from_utf8_lossy(xml);
    let mut blocks = Vec::new();
    let mut cursor = 0usize;

    while let Some(start) = text[cursor..].find('<') {
        let absolute = cursor + start;
        let rest = &text[absolute + 1..];
        let (tag, is_close) = parse_tag_name(rest)?;
        let close_index = rest.find('>').ok_or_else(|| {
            FormatError::parse("epub", "malformed xhtml tag")
        })?;
        let inner_start = absolute + 1 + close_index + 1;

        if is_close {
            cursor = inner_start;
            continue;
        }

        if matches!(tag.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
            let level = tag[1..].parse::<u8>().unwrap_or(1);
            let (content, next) = slice_until_close(&text, inner_start, &tag)?;
            blocks.push(Block::Section {
                level,
                title: vec![Inline::Text { text: strip_tags(content) }],
                children: Vec::new(),
            });
            cursor = next;
            continue;
        }

        if tag == "p" {
            let (content, next) = slice_until_close(&text, inner_start, "p")?;
            let plain = strip_tags(content);
            if !plain.is_empty() {
                blocks.push(Block::Paragraph {
                    content: vec![Inline::Text { text: plain }],
                });
            }
            cursor = next;
            continue;
        }

        cursor = inner_start;
    }

    if blocks.is_empty() {
        let plain = strip_tags(&text);
        if !plain.is_empty() {
            blocks.push(Block::Paragraph {
                content: vec![Inline::Text { text: plain }],
            });
        }
    }

    Ok(blocks)
}

fn parse_tag_name(rest: &str) -> Result<(String, bool), FormatError> {
    let trimmed = rest.trim_start();
    let close = trimmed.starts_with('/');
    let name = trimmed.trim_start_matches('/');
    let end = name
        .find(|ch: char| ch == '>' || ch.is_whitespace() || ch == '/')
        .unwrap_or(name.len());
    let tag = name[..end].to_string();
    if tag.is_empty() {
        return Err(FormatError::parse("epub", "empty xhtml tag"));
    }
    Ok((tag, close))
}

fn slice_until_close<'a>(
    text: &'a str,
    start: usize,
    tag: &str,
) -> Result<(&'a str, usize), FormatError> {
    let close = format!("</{tag}>");
    let slice = &text[start..];
    let end = slice
        .find(&close)
        .ok_or_else(|| FormatError::parse("epub", format!("unclosed <{tag}>")))?;
    Ok((&slice[..end], start + end + close.len()))
}

fn strip_tags(text: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}
