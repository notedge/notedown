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

    let payload_offset = 30 + name_len;
    let payload_len = u32::from_le_bytes([bytes[14], bytes[15], bytes[16], bytes[17]]) as usize;
    let payload = &bytes[payload_offset..payload_offset + payload_len];
    assert_eq!(payload, b"application/epub+zip");
}
