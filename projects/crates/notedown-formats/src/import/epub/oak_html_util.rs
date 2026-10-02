use oak_core::parser::session::ParseSession;
use oak_core::query::QueryBudget;
use oak_core::{Builder, SourceText};
use oak_html::ast::{Element, HtmlDocument, HtmlNode, Text};
use oak_html::query::{select_css_elements, HtmlDocumentView};
use oak_html::{HtmlBuilder, HtmlLanguage};

use crate::FormatError;

/// Parses XHTML or HTML bytes into a typed `oak-html` document.
pub fn parse_html_bytes(html: &[u8]) -> Result<HtmlDocument, FormatError> {
    let text = std::str::from_utf8(html).map_err(|error| {
        FormatError::parse("epub", format!("html is not utf-8: {error}"))
    })?;
    parse_html_text(text)
}

/// Parses XHTML or HTML text into a typed `oak-html` document.
pub fn parse_html_text(text: &str) -> Result<HtmlDocument, FormatError> {
    let language = HtmlLanguage::default();
    let source = SourceText::new(text);
    let builder = HtmlBuilder::new(language);
    let mut cache = ParseSession::<HtmlLanguage>::default();
    let built = builder.build(&source, &[], &mut cache);
    built
        .result
        .map_err(|error| FormatError::parse("epub", error.to_string()))
}

/// Returns whether an element's tag name matches `expected` case-insensitively.
pub fn element_is(element: &Element, expected: &str) -> bool {
    element.tag_name.eq_ignore_ascii_case(expected)
}

/// Reads an attribute by name case-insensitively.
pub fn attribute_value(element: &Element, expected: &str) -> Option<String> {
    element
        .attributes
        .iter()
        .find(|attr| attr.name.eq_ignore_ascii_case(expected))
        .and_then(|attr| attr.value.clone())
}

/// Returns all direct child elements regardless of tag name.
pub fn direct_child_elements<'a>(element: &'a Element) -> Vec<&'a Element> {
    element
        .children
        .iter()
        .filter_map(as_element)
        .collect()
}

/// Returns direct child elements whose tag name matches `expected`.
pub fn child_elements<'a>(element: &'a Element, expected: &str) -> Vec<&'a Element> {
    element
        .children
        .iter()
        .filter_map(as_element)
        .filter(|child| element_is(child, expected))
        .collect()
}

fn as_element(node: &HtmlNode) -> Option<&Element> {
    match node {
        HtmlNode::Element(element) => Some(element),
        _ => None,
    }
}

/// Returns trimmed direct text content for an element.
pub fn direct_text(element: &Element) -> String {
    let mut text = String::new();
    for child in &element.children {
        if let HtmlNode::Text(Text { content, .. }) = child {
            text.push_str(content);
        }
    }
    text.trim().to_string()
}

/// Returns trimmed descendant text content for an element.
pub fn descendant_text(element: &Element) -> String {
    let mut text = String::new();
    collect_descendant_text(element, &mut text);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_descendant_text(element: &Element, out: &mut String) {
    for child in &element.children {
        match child {
            HtmlNode::Text(Text { content, .. }) => out.push_str(content),
            HtmlNode::Element(child) => collect_descendant_text(child, out),
            HtmlNode::Comment(_) => {}
        }
    }
}

/// Finds the first descendant element whose tag name matches `expected`.
pub fn first_descendant_element<'a>(element: &'a Element, expected: &str) -> Option<&'a Element> {
    if element_is(element, expected) {
        return Some(element);
    }
    for child in &element.children {
        if let HtmlNode::Element(child_element) = child {
            if let Some(found) = first_descendant_element(child_element, expected) {
                return Some(found);
            }
        }
    }
    None
}

/// Selects elements in a document using a CSS selector subset.
pub fn select_css<'a>(
    document: &'a HtmlDocument,
    selector: &str,
) -> Result<Vec<&'a Element>, FormatError> {
    let view = HtmlDocumentView::from_document(document);
    let (_, elements) = select_css_elements(&view, selector, QueryBudget::default()).map_err(
        |error| FormatError::parse("epub", error.to_string()),
    )?;
    Ok(elements)
}

/// Finds the first descendant anchor and returns its label and `href`.
pub fn first_anchor(element: &Element) -> Option<(String, String)> {
    if element_is(element, "a") {
        let href = attribute_value(element, "href")?;
        let label = descendant_text(element);
        if label.is_empty() {
            return None;
        }
        return Some((label, href));
    }
    for child in &element.children {
        if let HtmlNode::Element(child_element) = child {
            if let Some(found) = first_anchor(child_element) {
                return Some(found);
            }
        }
    }
    None
}
