use acorn_epub::{ManifestItem, OpfDocument};
use notedown_ir::{Block, DocumentGraph, Inline, ListItem, NodeId};

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
    let text = String::from_utf8_lossy(xhtml);
    let entries = scraper_nav_entries(text.as_ref());
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

fn scraper_nav_entries(text: &str) -> Vec<NavEntry> {
    let lower = text.to_ascii_lowercase();
    let nav_start = lower
        .find("<nav")
        .filter(|index| {
            let slice = &lower[*index..];
            slice.contains("epub:type=\"toc\"")
                || slice.contains("epub:type='toc'")
                || slice.contains("id=\"toc\"")
                || slice.contains("id='toc'")
        })
        .or_else(|| lower.find("<nav"));
    if nav_start.is_none() {
        return Vec::new();
    }
    let nav_start = nav_start.unwrap();
    let nav_end = lower[nav_start..]
        .find("</nav>")
        .map(|offset| nav_start + offset)
        .unwrap_or(text.len());
    let nav_slice = &text[nav_start..nav_end];
    let structured = parse_nav_list_from_fragment(nav_slice);
    if !structured.is_empty() {
        return structured;
    }
    extract_anchor_entries(nav_slice)
        .into_iter()
        .map(|entry| NavEntry {
            label: entry.label,
            href: entry.href,
            children: Vec::new(),
        })
        .collect()
}

fn parse_nav_list_from_fragment(fragment: &str) -> Vec<NavEntry> {
    let lower = fragment.to_ascii_lowercase();
    if let Some(pos) = lower.find("<ol") {
        return parse_list_container(&fragment[pos..], "ol");
    }
    if let Some(pos) = lower.find("<ul") {
        return parse_list_container(&fragment[pos..], "ul");
    }
    Vec::new()
}

fn parse_list_container(fragment: &str, tag: &str) -> Vec<NavEntry> {
    let lower = fragment.to_ascii_lowercase();
    let open = format!("<{tag}");
    let Some(list_open) = lower.find(&open) else {
        return Vec::new();
    };
    let Some((inner, _)) = slice_balanced_list(fragment, list_open, tag) else {
        return Vec::new();
    };
    parse_top_level_list_items(inner)
}

fn parse_top_level_list_items(fragment: &str) -> Vec<NavEntry> {
    let mut entries = Vec::new();
    let mut cursor = 0usize;
    while cursor < fragment.len() {
        let lower = fragment[cursor..].to_ascii_lowercase();
        let rel = lower.find("<li");
        if rel.is_none() {
            break;
        }
        let li_open = cursor + rel.unwrap();
        if !is_li_open(&fragment[li_open..]) {
            cursor = li_open + 3;
            continue;
        }
        let Some((li_body, next)) = slice_li_element(fragment, li_open) else {
            break;
        };
        if let Some(entry) = parse_li_entry(li_body) {
            entries.push(entry);
        }
        cursor = next;
    }
    entries
}

fn parse_li_entry(li_body: &str) -> Option<NavEntry> {
    let (label, href) = first_anchor_in(li_body)?;
    let lower = li_body.to_ascii_lowercase();
    let nested_start = lower.find("<ol").or_else(|| lower.find("<ul"));
    let children = nested_start
        .map(|pos| parse_nav_list_from_fragment(&li_body[pos..]))
        .unwrap_or_default();
    Some(NavEntry {
        label,
        href,
        children,
    })
}

fn first_anchor_in(fragment: &str) -> Option<(String, String)> {
    let lower = fragment.to_ascii_lowercase();
    let start = lower.find("<a")?;
    let tag_end = lower[start..]
        .find('>')
        .map(|offset| start + offset)
        .unwrap_or(start);
    let tag = &fragment[start..=tag_end];
    let href = attribute_value(tag, "href")?;
    let close = lower[tag_end..]
        .find("</a>")
        .map(|offset| tag_end + offset)
        .unwrap_or(tag_end);
    let inner = fragment[tag_end + 1..close].trim();
    let label = strip_tags(inner);
    if label.is_empty() {
        return None;
    }
    Some((label, href))
}

fn is_li_open(fragment: &str) -> bool {
    let lower = fragment.to_ascii_lowercase();
    if !lower.starts_with("<li") {
        return false;
    }
    match lower.as_bytes().get(3) {
        Some(b'>' | b' ' | b'\t' | b'\n' | b'\r' | b'/') => true,
        _ => false,
    }
}

fn slice_li_element<'a>(text: &'a str, li_open: usize) -> Option<(&'a str, usize)> {
    let lower = text.to_ascii_lowercase();
    let gt = lower[li_open..].find('>')? + li_open;
    let inner_start = gt + 1;
    let mut depth = 1usize;
    let mut cursor = inner_start;
    while cursor < text.len() {
        if is_li_open(&text[cursor..]) {
            depth += 1;
            cursor += 3;
            continue;
        }
        if lower[cursor..].starts_with("</li>") {
            depth -= 1;
            if depth == 0 {
                return Some((&text[inner_start..cursor], cursor + 5));
            }
            cursor += 5;
            continue;
        }
        cursor += 1;
    }
    None
}

fn slice_balanced_list<'a>(text: &'a str, list_open: usize, tag: &str) -> Option<(&'a str, usize)> {
    let lower = text.to_ascii_lowercase();
    let open = format!("<{tag}");
    if !lower[list_open..].starts_with(&open) {
        return None;
    }
    let gt = lower[list_open..].find('>')? + list_open;
    let inner_start = gt + 1;
    let close = format!("</{tag}>");
    let mut depth = 1usize;
    let mut cursor = inner_start;
    while cursor < text.len() {
        if is_list_open(&text[cursor..], tag) {
            depth += 1;
            cursor += open.len();
            continue;
        }
        if lower[cursor..].starts_with(&close) {
            depth -= 1;
            if depth == 0 {
                return Some((&text[inner_start..cursor], cursor + close.len()));
            }
            cursor += close.len();
            continue;
        }
        cursor += 1;
    }
    None
}

fn is_list_open(fragment: &str, tag: &str) -> bool {
    let lower = fragment.to_ascii_lowercase();
    let open = format!("<{tag}");
    if !lower.starts_with(&open) {
        return false;
    }
    matches!(lower.as_bytes().get(open.len()), Some(b'>' | b' ' | b'\t' | b'\n' | b'\r' | b'/'))
}

struct FlatNavEntry {
    label: String,
    href: String,
}

fn extract_anchor_entries(fragment: &str) -> Vec<FlatNavEntry> {
    let mut entries = Vec::new();
    let lower = fragment.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(rel) = lower[search_from..].find("<a") {
        let start = search_from + rel;
        let tag_end = lower[start..]
            .find('>')
            .map(|offset| start + offset)
            .unwrap_or(start);
        let tag = &fragment[start..=tag_end];
        let href = attribute_value(tag, "href");
        let close = lower[tag_end..]
            .find("</a>")
            .map(|offset| tag_end + offset)
            .unwrap_or(tag_end);
        let inner = fragment[tag_end + 1..close].trim();
        let label = strip_tags(inner);
        if let Some(href) = href {
            if !label.is_empty() {
                entries.push(FlatNavEntry { label, href });
            }
        }
        search_from = close + 4;
    }
    entries
}

fn attribute_value(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let needle = format!("{name}=");
    let start = lower.find(&needle)?;
    let value_start = start + needle.len();
    let quote = tag[value_start..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &tag[value_start + 1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
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
        let entries = scraper_nav_entries(NAV);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "Part One");
        assert_eq!(entries[0].href, "part.xhtml");
        assert_eq!(entries[0].children.len(), 1);
        assert_eq!(entries[0].children[0].label, "Chapter 1");
        assert_eq!(entries[0].children[0].href, "part.xhtml#ch1");
    }
}
