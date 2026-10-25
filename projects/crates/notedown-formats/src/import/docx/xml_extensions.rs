use oak_core::query::QueryBudget;
use oak_xml::{
    ast::{XmlElement, XmlValue},
    query::{XmlDocumentView, select_xpath_elements},
};

use crate::FormatError;

/// Parses UTF-8 XML bytes through `oak-xml`.
///
/// `oak-xml` tokenizes `>`, `?`, `=`, quotes, `;` and entity references as
/// dedicated tokens, and its AST builder keeps only text and identifier leaves.
/// The raw slice is therefore used to restore the text content of leaf elements
/// so escaped characters such as `&gt;` and `&quot;` survive import.
pub fn parse_xml_bytes(xml: &[u8]) -> Result<XmlValue, FormatError> {
    let text = std::str::from_utf8(xml).map_err(|error| FormatError::parse("docx", format!("xml is not utf-8: {error}")))?;
    let mut value = oak_xml::parse(text).map_err(|error| FormatError::parse("docx", error.to_string()))?;
    restore_leaf_text(&mut value, text);
    Ok(value)
}

fn restore_leaf_text(value: &mut XmlValue, source: &str) {
    match value {
        XmlValue::Element(element) => restore_element_leaf_text(element, source),
        XmlValue::Fragment(values) => {
            for child in values {
                restore_leaf_text(child, source);
            }
        }
        _ => {}
    }
}

fn restore_element_leaf_text(element: &mut XmlElement, source: &str) {
    let mut has_child_elements = false;
    for child in &mut element.children {
        match child {
            XmlValue::Element(_) => has_child_elements = true,
            XmlValue::Fragment(values) => {
                for value in values {
                    if let XmlValue::Element(_) = value {
                        has_child_elements = true;
                    }
                }
            }
            _ => {}
        }
        restore_leaf_text(child, source);
    }
    if has_child_elements {
        return;
    }
    let Some((start, end)) = element_content_range(element, source)
    else {
        return;
    };
    let decoded = decode_xml_entities(&source[start..end]);
    element.children.clear();
    if !decoded.is_empty() {
        element.children.push(XmlValue::Text(decoded));
    }
}

/// Returns the raw content range of a leaf element, between the start tag's
/// closing `>` and the closing tag's `<`.
fn element_content_range(element: &XmlElement, source: &str) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let start = element.span.start;
    let end = element.span.end.min(source.len());
    if start >= end || bytes.get(start) != Some(&b'<') {
        return None;
    }
    let mut index = start;
    let mut quote = None;
    let content_start = loop {
        let byte = *bytes.get(index)?;
        match quote {
            Some(open) => {
                if byte == open {
                    quote = None;
                }
            }
            None if byte == b'"' || byte == b'\'' => quote = Some(byte),
            None if byte == b'>' => {
                if index > start && bytes[index - 1] == b'/' {
                    return None;
                }
                break index + 1;
            }
            None => {}
        }
        index += 1;
    };
    let tail = source.get(content_start..end)?;
    let tag_start = tail.rfind("</")? + content_start;
    Some((content_start, tag_start))
}

/// Decodes the XML entities a WordprocessingML writer emits for text content.
fn decode_xml_entities(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(position) = rest.find('&') {
        output.push_str(&rest[..position]);
        let candidate = &rest[position..];
        let Some(semicolon) = candidate.find(';') else {
            output.push_str(candidate);
            return output;
        };
        let entity = &candidate[1..semicolon];
        match entity {
            "amp" => output.push('&'),
            "lt" => output.push('<'),
            "gt" => output.push('>'),
            "quot" => output.push('"'),
            "apos" => output.push('\''),
            _ => match numeric_character_reference(entity) {
                Some(character) => output.push(character),
                None => output.push_str(&candidate[..=semicolon]),
            },
        }
        rest = &candidate[semicolon + 1..];
    }
    output.push_str(rest);
    output
}

fn numeric_character_reference(entity: &str) -> Option<char> {
    let digits = entity.strip_prefix('#')?;
    let code = match digits.strip_prefix(['x', 'X']) {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => digits.parse().ok()?,
    };
    char::from_u32(code)
}

/// Returns the local name after an optional namespace prefix.
pub fn local_name(name: &str) -> &str {
    name.rsplit_once(':').map(|(_, local)| local).unwrap_or(name)
}

/// Returns whether an element's local tag name matches `expected`.
pub fn element_is(element: &XmlElement, expected: &str) -> bool {
    local_name(&element.name) == expected
}

/// Reads an attribute by local name.
pub fn attribute_value(element: &XmlElement, expected: &str) -> Option<String> {
    element.attributes.iter().find(|attr| local_name(&attr.name) == expected).map(|attr| attr.value.clone())
}

/// Reads a `u32` attribute by local name.
pub fn u32_attribute(element: &XmlElement, expected: &str) -> Option<u32> {
    attribute_value(element, expected)?.parse().ok()
}

/// Interprets a WordprocessingML boolean flag element.
pub fn bool_from_element(element: Option<&XmlElement>, default: bool) -> bool {
    match element {
        Some(element) => attribute_value(element, "val").map(|value| !matches!(value.as_str(), "0" | "false" | "off")).unwrap_or(default),
        None => default,
    }
}

/// Returns the first direct child element with the given local tag name.
pub fn child_element<'a>(element: &'a XmlElement, expected: &str) -> Option<&'a XmlElement> {
    element.children.iter().filter_map(XmlValue::as_element).find(|child| element_is(child, expected))
}

/// Collects descendant elements whose local tag name matches `expected`.
pub fn elements_by_local_name<'a>(element: &'a XmlElement, expected: &str) -> Vec<&'a XmlElement> {
    let xpath = format!("//{expected}");
    elements_by_xpath(element, &xpath).unwrap_or_default()
}

/// Collects elements matched by an Oak XPath subset rooted at `element`.
pub fn elements_by_xpath<'a>(element: &'a XmlElement, xpath: &str) -> Result<Vec<&'a XmlElement>, FormatError> {
    let view = XmlDocumentView::from_element(element);
    let (_result, elements) =
        select_xpath_elements(&view, xpath, QueryBudget::default()).map_err(|error| FormatError::parse("docx", error.to_string()))?;
    Ok(elements)
}

/// Returns trimmed text content for an element.
pub fn element_text(element: &XmlElement) -> String {
    let mut text = String::new();
    collect_element_text(element, &mut text);
    text
}

fn collect_element_text(element: &XmlElement, out: &mut String) {
    for child in &element.children {
        match child {
            XmlValue::Text(value) => out.push_str(value),
            XmlValue::CData(value) => out.push_str(value),
            XmlValue::Element(child) => collect_element_text(child, out),
            XmlValue::Fragment(values) => {
                for value in values {
                    if let XmlValue::Text(text) = value {
                        out.push_str(text);
                    }
                    else if let XmlValue::Element(child) = value {
                        collect_element_text(child, out);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Unwraps the document root element from an `oak-xml` value.
pub fn document_root(value: &XmlValue) -> Result<&XmlElement, FormatError> {
    match value {
        XmlValue::Element(element) => Ok(element),
        XmlValue::Fragment(values) => {
            values.iter().find_map(XmlValue::as_element).ok_or_else(|| FormatError::parse("docx", "xml fragment has no root element"))
        }
        _ => Err(FormatError::parse("docx", "xml has no root element")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_leaf_text(xml: &str) -> String {
        let value = parse_xml_bytes(xml.as_bytes()).expect("parse xml");
        element_text(document_root(&value).expect("document root"))
    }

    #[test]
    fn leaf_text_restores_escaped_characters_dropped_by_the_parser() {
        assert_eq!(parse_leaf_text("<w:t xmlns:w=\"urn:x\">A --&gt; B{Done?}</w:t>"), "A --> B{Done?}");
        assert_eq!(parse_leaf_text("<w:t xmlns:w=\"urn:x\">a &quot;quoted&quot; &amp; &lt;tag&gt;</w:t>"), "a \"quoted\" & <tag>");
    }

    #[test]
    fn leaf_text_decodes_character_references() {
        assert_eq!(parse_leaf_text("<w:t xmlns:w=\"urn:x\">&#62; &#x3F;</w:t>"), "> ?");
    }

    #[test]
    fn leaf_text_keeps_unknown_entities_verbatim() {
        assert_eq!(parse_leaf_text("<w:t xmlns:w=\"urn:x\">R&amp;D &unknown;</w:t>"), "R&D &unknown;");
    }

    #[test]
    fn self_closing_leaf_has_no_text() {
        assert_eq!(parse_leaf_text("<w:br xmlns:w=\"urn:x\"/>"), "");
    }
}
