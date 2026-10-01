use std::collections::HashMap;
use std::fmt::Write as _;

use notedown_ir::{Block, DocumentGraph};

use crate::FormatError;

/// Collect footnote bodies from `footnote_definition` opaque blocks.
pub fn collect_footnote_catalog(graph: &DocumentGraph) -> HashMap<u32, String> {
    let mut catalog = HashMap::new();
    for node in &graph.blocks {
        if let Block::Opaque {
            kind,
            payload_hint,
            ..
        } = &node.block
        {
            if kind == "footnote_definition" {
                if let Some((id, body)) = parse_footnote_definition(payload_hint) {
                    catalog.insert(id, body);
                }
            }
        }
    }
    catalog
}

/// Parse GFM footnote definition text such as `[^1]: body`.
pub fn parse_footnote_definition(payload: &str) -> Option<(u32, String)> {
    let trimmed = payload.trim();
    if !trimmed.starts_with("[^") {
        return None;
    }
    let close = trimmed.find(']')?;
    let id_text = trimmed[2..close].trim();
    let id = id_text.parse().ok()?;
    let body = trimmed
        .get(close + 1..)
        .and_then(|rest| rest.strip_prefix(':'))
        .map(str::trim)
        .filter(|body| !body.is_empty())?;
    Some((id, body.to_string()))
}

/// Parse an inline footnote reference token such as `[^1]`.
pub fn parse_footnote_reference_token(token: &str) -> Option<u32> {
    let trimmed = token.trim();
    if !trimmed.starts_with("[^") || !trimmed.ends_with(']') {
        return None;
    }
    trimmed[2..trimmed.len() - 1].trim().parse().ok()
}

/// Render `word/footnotes.xml` for the given catalog.
pub fn render_footnotes_xml(catalog: &HashMap<u32, String>) -> Result<String, FormatError> {
    let mut ids: Vec<u32> = catalog.keys().copied().collect();
    ids.sort_unstable();

    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    );
    for id in ids {
        let body = catalog.get(&id).expect("key from ids");
        xml.push_str("\n  <w:footnote w:id=\"");
        xml.push_str(&id.to_string());
        xml.push_str("\"><w:p><w:r><w:t>");
        write_xml_text(&mut xml, body)?;
        xml.push_str("</w:t></w:r></w:p></w:footnote>");
    }
    xml.push_str("\n</w:footnotes>");
    Ok(xml)
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
