use notedown_formats::export::markdown::export_markdown;
use notedown_formats::import::notedown::import_notedown_bytes;

#[test]
fn notedown_import_lowers_heading_and_paragraph() {
    let source = "# Title\n\nHello world.\n";
    let graph = import_notedown_bytes("sample.nd", source).expect("import notedown");
    assert!(graph.blocks.len() >= 2);
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("# Title"));
    assert!(markdown.contains("Hello world") || markdown.contains("Helloworld"));
}

#[test]
fn notedown_import_lowers_list_items() {
    let source = "- Alpha\n- Beta\n";
    let graph = import_notedown_bytes("list.nd", source).expect("import notedown");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("- Alpha"));
    assert!(markdown.contains("- Beta"));
}

#[test]
fn notedown_import_lowers_links() {
    let source = "Visit [home](https://example.com) today.\n";
    let graph = import_notedown_bytes("link.nd", source).expect("import notedown");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("[home](https://example.com)"));
}
