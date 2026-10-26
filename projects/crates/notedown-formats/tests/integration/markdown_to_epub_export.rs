//! Markdown → EPUB 3 export round-trip integrity tests.
//!
//! Exercises `export::epub::export_epub_bytes` and verifies the resulting ZIP
//! is readable by `acorn-epub::OcfPackage` with the expected OPF, nav, and
//! chapter members.

use acorn_core::{Budget, BudgetTracker};
use acorn_epub::OcfPackage;
use notedown_formats::{
    export::epub::export_epub_bytes,
    import::markdown::import_markdown_bytes,
};

fn make_simple_markdown() -> &'static str {
    r#"# A Small Book

Some opening prose.

## Chapter One

Hello world. `let x = 1;`

## Chapter Two

A table:

| a | b |
|---|---|
| 1 | 2 |

And a list:

- one
- two
- three
"#
}

#[test]
fn epub_export_produces_valid_ocf_package() {
    let graph = import_markdown_bytes("book.md", make_simple_markdown()).expect("import markdown");
    let bytes = export_epub_bytes(&graph).expect("export epub");

    // Must start with the ZIP local file header followed by "mimetype" + payload.
    assert!(bytes.starts_with(b"PK\x03\x04"), "EPUB must start with ZIP local header");

    // OcfPackage must open without error.
    let package = OcfPackage::open("test.epub", bytes).expect("open as OCF");
    let mut ledger = BudgetTracker::new(Budget::default());

    let root_opf = package.root_opf_path();
    assert_eq!(root_opf, "OEBPS/content.opf", "container.xml must point at content.opf");

    let opf = package.read_opf(&mut ledger).expect("read OPF");
    assert!(
        opf.metadata
            .title
            .as_deref()
            .map_or(false, |s| s.contains("Small Book"))
    );
    assert_eq!(opf.metadata.language.as_deref(), Some("en"));
    assert!(!opf.manifest.is_empty(), "manifest must not be empty");
    assert!(!opf.spine.is_empty(), "spine must not be empty");

    // Nav manifest item must be flagged with properties="nav".
    let nav_item = opf
        .manifest
        .get("nav")
        .expect("manifest must contain nav");
    assert_eq!(nav_item.media_type, "application/xhtml+xml");
    assert!(
        nav_item
            .properties
            .as_deref()
            .map_or(false, |s| s.contains("nav")),
        "nav manifest item must carry properties=nav, got {:?}",
        nav_item.properties
    );

    // All spine items must exist in the manifest and have an XHTML href.
    for spine in &opf.spine {
        let item = opf
            .manifest
            .get(&spine.idref)
            .expect("spine idref must resolve in manifest");
        assert!(
            item.media_type.contains("xhtml"),
            "spine item must be XHTML, got {}",
            item.media_type
        );
        assert!(item.href.ends_with(".xhtml"), "spine href must end with .xhtml");
    }
}

#[test]
fn epub_export_mimetype_is_stored_and_first() {
    let graph = import_markdown_bytes("book.md", "# Title\n\nBody.").expect("import markdown");
    let bytes = export_epub_bytes(&graph).expect("export epub");

    eprintln!("total bytes: {}", bytes.len());
    eprintln!("first 60 bytes hex: {:02x?}", &bytes[..60.min(bytes.len())]);
    eprintln!("bytes[26..30]: {:02x?}", &bytes[26..30]);

    // First local file header must point at "mimetype".
    // Local file header layout (APPNOTE §4.3.7):
    //  0..4  signature "PK\x03\x04"
    //  4..6  version needed
    //  6..8  flags
    //  8..10 compression method (0 = stored)
    let flags = u16::from_le_bytes([bytes[6], bytes[7]]);
    let compression = u16::from_le_bytes([bytes[8], bytes[9]]);
    assert_eq!(compression, 0, "mimetype must be STORED (method 0)");
    assert!(
        flags & 0x08 == 0,
        "mimetype must not use data descriptor (bit 3)"
    );

    let name_len = u16::from_le_bytes([bytes[26], bytes[27]]) as usize;
    let name = &bytes[30..30 + name_len];
    assert_eq!(name, b"mimetype");

    let extra_len = u16::from_le_bytes([bytes[28], bytes[29]]) as usize;
    let payload_offset = 30 + name_len + extra_len;
    // Compressed size lives at offset 18..22 in the local file header (APPNOTE §4.3.7).
    let payload_len = u32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]) as usize;
    let payload = &bytes[payload_offset..payload_offset + payload_len];
    assert_eq!(payload, b"application/epub+zip");
}

/// EPUB 3 §4.2 规定 OPF 必须声明 dc:identifier / dc:title / dc:language；
/// §3.4 规定 nav 文档的 <html> 根元素必须带 epub:type="nav"。
///
/// 这些断言走原始 XML 字符串匹配，不依赖 epubcheck。
#[test]
fn epub_export_opf_has_required_dc_elements_and_nav_root_is_html_with_epub_type() {
    let graph = import_markdown_bytes("book.md", make_simple_markdown()).expect("import markdown");
    let bytes = export_epub_bytes(&graph).expect("export epub");

    let package = OcfPackage::open("test.epub", bytes).expect("open as OCF");
    let mut ledger = BudgetTracker::new(Budget::default());

    // --- 1. OPF 原始 XML 必须包含三个必填 dc 元素 -----------------------------
    let opf_bytes = package
        .read_member(package.root_opf_path(), &mut ledger)
        .expect("read OPF raw bytes");
    let opf_xml = String::from_utf8(opf_bytes).expect("OPF must be UTF-8");

    assert!(
        opf_xml.contains("<dc:identifier"),
        "OPF must declare dc:identifier (EPUB 3 §4.2.1)"
    );
    assert!(
        opf_xml.contains("<dc:title>"),
        "OPF must declare dc:title (EPUB 3 §4.2.2)"
    );
    assert!(
        opf_xml.contains("<dc:language>"),
        "OPF must declare dc:language (EPUB 3 §4.2.3)"
    );
    // dc:identifier 内容必须是 urn:uuid:<32 hex with dashes> 格式。
    let uuid_start = opf_xml
        .find("urn:uuid:")
        .expect("dc:identifier should contain urn:uuid: prefix");
    let uuid_body = &opf_xml[uuid_start + 9..uuid_start + 9 + 36];
    assert_eq!(uuid_body.chars().filter(|&c| c == '-').count(), 4, "uuid must have 4 dashes");
    let without_dashes: String = uuid_body.chars().filter(|&c| c != '-').collect();
    assert_eq!(without_dashes.len(), 32, "uuid body must be 32 hex chars without dashes");
    assert!(
        without_dashes.chars().all(|c| c.is_ascii_hexdigit()),
        "uuid body must be all hex digits, got {uuid_body}"
    );

    // --- 2. nav 文档根元素是带 epub:type="nav" 的 <html> --------------------
    let nav_bytes = package
        .read_member("OEBPS/nav.xhtml", &mut ledger)
        .expect("read nav.xhtml raw bytes");
    let nav_xml = String::from_utf8(nav_bytes).expect("nav.xhtml must be UTF-8");

    assert!(
        nav_xml.starts_with("<?xml"),
        "nav.xhtml must begin with an XML declaration"
    );
    assert!(
        nav_xml.contains(r#"<html xmlns="http://www.w3.org/1999/xhtml""#),
        "nav.xhtml root must be <html> with the XHTML namespace"
    );
    assert!(
        nav_xml.contains("xmlns:epub=\"http://www.idpf.org/2007/ops\""),
        "nav.xhtml must declare the epub namespace on root <html>"
    );
    assert!(
        nav_xml.contains(r#"epub:type="nav""#),
        "nav.xhtml root <html> must carry epub:type=\"nav\" (EPUB 3 §3.4)"
    );
    // xml:lang 和 lang 属性都必须落在根 <html> 上，且值一致。
    let xml_lang = extract_attr(&nav_xml, "xml:lang");
    let lang = extract_attr(&nav_xml, "lang");
    assert!(
        xml_lang.is_some() && lang.is_some() && xml_lang == lang,
        "nav.xhtml root <html> must have matching xml:lang and lang, got xml:lang={xml_lang:?}, lang={lang:?}"
    );
}

/// Extract the double-quoted value of an attribute from the nav string.
fn extract_attr(xml: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let idx = xml.find(&needle)?;
    let start = idx + needle.len();
    let rest = &xml[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
