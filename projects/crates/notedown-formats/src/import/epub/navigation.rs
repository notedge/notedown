use acorn_epub::{ManifestItem, OpfDocument};
use notedown_ir::{Block, Inline, ListItem};

use crate::FormatError;

/// One table-of-contents entry from EPUB navigation documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavEntry {
    pub label: String,
    pub href: String,
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

/// Parse a navigation XHTML document into flat TOC entries.
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

/// Lower navigation entries to a link list block for Markdown export.
pub fn navigation_list_block(entries: &[NavEntry]) -> Block {
    Block::List {
        ordered: false,
        items: entries
            .iter()
            .map(|entry| ListItem {
                content: vec![Inline::Styled {
                    style: "link".into(),
                    children: vec![
                        Inline::Text {
                            text: entry.label.clone(),
                        },
                        Inline::Text {
                            text: entry.href.clone(),
                        },
                    ],
                }],
                children: Vec::new(),
            })
            .collect(),
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
    extract_anchor_entries(nav_slice)
}

fn extract_anchor_entries(fragment: &str) -> Vec<NavEntry> {
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
                entries.push(NavEntry { label, href });
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
