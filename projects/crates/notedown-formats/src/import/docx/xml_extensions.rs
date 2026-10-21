use oak_core::query::QueryBudget;
use oak_xml::{
    ast::{XmlElement, XmlValue},
    query::{XmlDocumentView, select_xpath_elements},
};

use crate::FormatError;

/// Parses UTF-8 XML bytes through `oak-xml`.
pub fn parse_xml_bytes(xml: &[u8]) -> Result<XmlValue, FormatError> {
    let text = std::str::from_utf8(xml).map_err(|error| FormatError::parse("docx", format!("xml is not utf-8: {error}")))?;
    oak_xml::parse(text).map_err(|error| FormatError::parse("docx", error.to_string()))
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
