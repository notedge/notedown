//! EPUB 3 `OEBPS/content.opf` generation.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use notedown_ir::DocumentGraph;

use crate::FormatError;

/// One `<item>` entry inside the OPF manifest.
#[derive(Debug, Clone)]
pub struct OpfManifestItem {
    /// Stable id attribute.
    pub id: String,
    /// Href relative to the OPF location.
    pub href: String,
    /// MIME media type.
    pub media_type: String,
    /// Optional `properties` attribute such as `nav` or `cover-image`.
    pub properties: Option<String>,
}

/// One `<itemref>` entry inside the OPF spine.
#[derive(Debug, Clone)]
pub struct OpfSpineItem {
    /// idref pointing back to a manifest item.
    pub idref: String,
}

/// Render the OPF XML for the given document graph and manifest/spine.
pub fn content_opf(
    graph: &DocumentGraph,
    items: &[OpfManifestItem],
    spine: &[OpfSpineItem],
) -> Result<String, FormatError> {
    let unique_id = stable_unique_identifier(graph);
    let mut out = String::new();
    out.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="BookId">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:identifier id="BookId">urn:uuid:"#);
    out.push_str(&unique_id);
    out.push_str(
        r#"</dc:identifier>
"#,
    );
    if let Some(title) = &graph.metadata.title {
        out.push_str("    <dc:title>");
        out.push_str(&escape_xml(title));
        out.push_str("</dc:title>\n");
    }
    if let Some(language) = graph.metadata.language.as_deref() {
        out.push_str("    <dc:language>");
        out.push_str(&escape_xml(language));
        out.push_str("</dc:language>\n");
    } else {
        out.push_str("    <dc:language>en</dc:language>\n");
    }
    if let Some(author) = graph.metadata.authors.first() {
        out.push_str("    <dc:creator>");
        out.push_str(&escape_xml(author));
        out.push_str("</dc:creator>\n");
    }
    out.push_str("  </metadata>\n");

    out.push_str("  <manifest>\n");
    for item in items {
        out.push_str("    <item id=\"");
        out.push_str(&escape_xml(&item.id));
        out.push_str("\" href=\"");
        out.push_str(&escape_xml(&item.href));
        out.push_str("\" media-type=\"");
        out.push_str(&escape_xml(&item.media_type));
        out.push('"');
        if let Some(props) = &item.properties {
            out.push_str(" properties=\"");
            out.push_str(&escape_xml(props));
            out.push('"');
        }
        out.push_str("/>\n");
    }
    out.push_str("  </manifest>\n");

    out.push_str("  <spine>\n");
    for itemref in spine {
        out.push_str("    <itemref idref=\"");
        out.push_str(&escape_xml(&itemref.idref));
        out.push_str("\"/>\n");
    }
    out.push_str("  </spine>\n");
    out.push_str("</package>\n");

    Ok(out)
}

fn stable_unique_identifier(graph: &DocumentGraph) -> String {
    // Build a stable pseudo-UUID from (id, revision, title). Real EPUBs expect
    // urn:uuid but do not require cryptographic randomness.
    let mut h1 = DefaultHasher::new();
    graph.id.hash(&mut h1);
    graph.revision.hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    graph.metadata.title.hash(&mut h2);
    graph.metadata.authors.hash(&mut h2);
    let a = h1.finish();
    let b = h2.finish();
    let time_low = (a >> 32) as u32;
    let time_mid = (a >> 16) as u16;
    let time_hi = (a as u16 & 0x0FFF) | 0x4000; // version 4
    let clock_seq = (b as u16 & 0x3FFF) | 0x8000; // RFC 4122 variant
    let node = b & 0xFFFF_FFFF_FFFF;
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        time_low, time_mid, time_hi, clock_seq, node
    )
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
