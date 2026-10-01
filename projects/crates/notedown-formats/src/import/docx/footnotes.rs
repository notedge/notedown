use std::collections::{HashMap, HashSet};

use notedown_ir::{Block, DocumentGraph, SemanticStatus};

use super::oak_xml_util::{
    attribute_value, document_root, element_is, element_text, elements_by_local_name,
    parse_xml_bytes, u32_attribute,
};
use oak_xml::ast::{XmlElement, XmlValue};

/// Resolved footnote bodies keyed by `w:footnote` id.
pub type FootnoteCatalog = HashMap<u32, String>;

const FOOTNOTES_XML: &str = "word/footnotes.xml";

/// Part path for WordprocessingML footnote definitions.
pub fn footnotes_part_path() -> &'static str {
    FOOTNOTES_XML
}

/// Parses `word/footnotes.xml` into plain-text footnote bodies.
pub fn parse_footnotes_xml_lossy(xml: &[u8]) -> FootnoteCatalog {
    let Ok(value) = parse_xml_bytes(xml) else {
        return FootnoteCatalog::new();
    };
    let Ok(root) = document_root(&value) else {
        return FootnoteCatalog::new();
    };
    let mut catalog = FootnoteCatalog::new();

    for footnote in elements_by_local_name(root, "footnote") {
        let id = u32_attribute(footnote, "id");
        if id.is_none_or(|id| id == 0) {
            continue;
        }
        let id = id.expect("checked above");
        let body = footnote_body_text(footnote);
        if !body.is_empty() {
            catalog.insert(id, body);
        }
    }

    catalog
}

fn footnote_body_text(footnote: &XmlElement) -> String {
    let mut body = String::new();
    for paragraph in footnote
        .children
        .iter()
        .filter_map(XmlValue::as_element)
        .filter(|child| element_is(child, "p"))
    {
        let text = paragraph_plain_text(paragraph);
        append_paragraph(&mut body, &text);
    }
    body.trim().to_string()
}

fn paragraph_plain_text(paragraph: &XmlElement) -> String {
    let mut text = String::new();
    for run in paragraph
        .children
        .iter()
        .filter_map(XmlValue::as_element)
        .filter(|child| element_is(child, "r"))
    {
        text.push_str(&run_plain_text(run));
    }
    text
}

fn run_plain_text(run: &XmlElement) -> String {
    let mut text = String::new();
    for child in &run.children {
        match child {
            XmlValue::Element(element) if element_is(element, "t") => {
                text.push_str(&element_text(element));
            }
            XmlValue::Element(element) if element_is(element, "tab") => text.push('\t'),
            XmlValue::Element(element) if element_is(element, "br") => text.push('\n'),
            _ => {}
        }
    }
    text
}

/// Appends GFM footnote definition blocks for referenced ids with resolved bodies.
pub fn append_footnote_definitions(
    graph: &mut DocumentGraph,
    catalog: &FootnoteCatalog,
    referenced: &HashSet<u32>,
) {
    let mut ids: Vec<u32> = referenced
        .iter()
        .filter(|id| catalog.contains_key(id))
        .copied()
        .collect();
    ids.sort_unstable();
    for id in ids {
        let body = catalog.get(&id).expect("filtered above");
        graph.push_block(Block::Opaque {
            kind: "footnote_definition".into(),
            payload_hint: format!("[^{}]: {}", id, body),
            status: SemanticStatus::Resolved,
        });
    }
}

fn append_paragraph(body: &mut String, paragraph: &str) {
    let trimmed = paragraph.trim();
    if trimmed.is_empty() {
        return;
    }
    if !body.is_empty() {
        body.push('\n');
    }
    body.push_str(trimmed);
}
