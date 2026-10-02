use acorn_epub::{ManifestItem, OpfDocument};
use notedown_ir::{Block, DocumentGraph, Inline, ListItem, NodeId};
use oak_core::query::QueryBudget;
use oak_html::ast::Element;
use oak_html::query::{select_css_elements, HtmlDocumentView};

use super::oak_html_util::{
    attribute_value, child_elements, element_is, first_anchor, parse_html_bytes,
};
use crate::FormatError;

/// One table-of-contents entry from EPUB navigation documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavEntry {
    pub label: String,
    pub href: String,
    pub children: Vec<NavEntry>,
}

/// Locate the EPUB3 navigation manifest item when declared via `properties="nav"`.
pub fn find_nav_manifest_item(opf: &OpfDocument) -> Option<&ManifestItem> {
    opf.manifest.values().find(|item| {
        item.properties
            .as_deref()
            .map(|properties| properties.split_whitespace().any(|token| token == "nav"))
            .unwrap_or(false)
    })
}

/// Parse a navigation XHTML document into nested TOC entries.
pub fn parse_nav_toc(xhtml: &[u8]) -> Result<Vec<NavEntry>, FormatError> {
    let entries = nav_entries_from_xhtml(xhtml)?;
    if entries.is_empty() {
        return Err(FormatError::parse(
            "epub",
            "navigation document did not yield any TOC links",
        ));
    }
    Ok(entries)
}

/// Lower navigation entries into the document graph as a nested link list.
pub fn push_navigation_toc(graph: &mut DocumentGraph, entries: &[NavEntry]) -> NodeId {
    let items = entries
        .iter()
        .map(|entry| nav_entry_to_list_item(graph, entry))
        .collect();
    graph.push_block(Block::List {
        ordered: false,
        items,
    })
}

fn nav_entry_to_list_item(graph: &mut DocumentGraph, entry: &NavEntry) -> ListItem {
    let children = if entry.children.is_empty() {
        Vec::new()
    } else {
        let nested_items = entry
            .children
            .iter()
            .map(|child| nav_entry_to_list_item(graph, child))
            .collect();
        vec![graph.push_block(Block::List {
            ordered: false,
            items: nested_items,
        })]
    };
    ListItem {
        content: vec![link_inline(entry)],
        children,
    }
}

fn link_inline(entry: &NavEntry) -> Inline {
    Inline::Styled {
        style: "link".into(),
        children: vec![
            Inline::Text {
                text: entry.label.clone(),
            },
            Inline::Text {
                text: entry.href.clone(),
            },
        ],
    }
}

fn nav_entries_from_xhtml(xhtml: &[u8]) -> Result<Vec<NavEntry>, FormatError> {
    let document = parse_html_bytes(xhtml)?;
    let view = HtmlDocumentView::from_document(&document);
    let nav = find_navigation_element(&view, &document)?;
    Ok(parse_nav_list_from_element(nav))
}

fn find_navigation_element<'a>(
    view: &'a HtmlDocumentView<'a>,
    document: &'a oak_html::HtmlDocument,
) -> Result<&'a Element, FormatError> {
    let (_, nav_elements) =
        select_css_elements(view, "nav", QueryBudget::default()).map_err(|error| {
            FormatError::parse("epub", error.to_string())
        })?;
    if let Some(nav) = nav_elements
        .iter()
        .find(|element| is_toc_navigation(element))
        .or(nav_elements.first())
    {
        return Ok(nav);
    }

    for node in &document.nodes {
        if let oak_html::ast::HtmlNode::Element(element) = node {
            if element_is(element, "nav") {
                return Ok(element);
            }
        }
    }

    Err(FormatError::parse("epub", "navigation document has no nav element"))
}

fn is_toc_navigation(nav: &Element) -> bool {
    attribute_value(nav, "id")
        .map(|value| value.eq_ignore_ascii_case("toc"))
        .unwrap_or(false)
        || attribute_value(nav, "epub:type")
            .map(|value| value.split_whitespace().any(|token| token == "toc"))
            .unwrap_or(false)
}

fn parse_nav_list_from_element(nav: &Element) -> Vec<NavEntry> {
    if let Some(list) = child_elements(nav, "ol")
        .into_iter()
        .chain(child_elements(nav, "ul"))
        .next()
    {
        return parse_list_items(list);
    }

    let view = HtmlDocumentView::from_element(nav);
    let anchors = select_css_elements(&view, "a[href]", QueryBudget::default())
        .map(|(_, elements)| elements)
        .unwrap_or_default();
    anchors
        .into_iter()
        .filter_map(|anchor| {
            first_anchor(anchor).map(|(label, href)| NavEntry {
                label,
                href,
                children: Vec::new(),
            })
        })
        .collect()
}

fn parse_list_items(list: &Element) -> Vec<NavEntry> {
    child_elements(list, "li")
        .into_iter()
        .filter_map(parse_li_entry)
        .collect()
}

fn parse_li_entry(li: &Element) -> Option<NavEntry> {
    let (label, href) = first_anchor(li)?;
    let children = child_elements(li, "ol")
        .into_iter()
        .chain(child_elements(li, "ul"))
        .flat_map(parse_list_items)
        .collect();
    Some(NavEntry {
        label,
        href,
        children,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_nav_entries() {
        const NAV: &str = r#"<nav epub:type="toc">
<ol>
  <li><a href="part.xhtml">Part One</a>
    <ol>
      <li><a href="part.xhtml#ch1">Chapter 1</a></li>
    </ol>
  </li>
</ol>
</nav>"#;
        let entries = nav_entries_from_xhtml(NAV.as_bytes()).expect("parse nav");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "Part One");
        assert_eq!(entries[0].href, "part.xhtml");
        assert_eq!(entries[0].children.len(), 1);
        assert_eq!(entries[0].children[0].label, "Chapter 1");
        assert_eq!(entries[0].children[0].href, "part.xhtml#ch1");
    }

    #[test]
    fn selects_toc_nav_via_css_selector() {
        const NAV: &str = r#"<body>
<nav id="other"><a href="skip.xhtml">Skip</a></nav>
<nav epub:type="toc"><ol><li><a href="part.xhtml">Part</a></li></ol></nav>
</body>"#;
        let entries = nav_entries_from_xhtml(NAV.as_bytes()).expect("parse nav");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].href, "part.xhtml");
    }
}
