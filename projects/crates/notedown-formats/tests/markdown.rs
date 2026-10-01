use notedown_formats::export::markdown::export_markdown;
use notedown_formats::import::markdown::import_markdown_bytes;

#[test]
fn markdown_import_lowers_heading_and_paragraph() {
    let source = "# Title\n\nHello world.\n";
    let graph = import_markdown_bytes("sample.md", source).expect("import markdown");
    assert_eq!(graph.blocks.len(), 2);
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("# Title"));
    assert!(markdown.contains("Hello world."));
}

#[test]
fn markdown_import_lowers_inline_styles_and_links() {
    let source = "Hello **bold** and *italic* with [link](https://example.com).\n";
    let graph = import_markdown_bytes("inline.md", source).expect("import markdown");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("**bold**"));
    assert!(markdown.contains("*italic*"));
    assert!(markdown.contains("[link](https://example.com)"));
}

#[test]
fn markdown_round_trip_validates() {
    let source = "# Route\n\nBody.\n";
    let graph = import_markdown_bytes("route.md", source).expect("import");
    assert!(graph.validate().is_valid());
    let markdown = export_markdown(&graph).expect("export");
    assert!(markdown.contains("# Route"));
}
