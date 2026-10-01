use notedown_formats::export::markdown::export_markdown;
use notedown_formats::import::notedown::import_notedown_bytes;

#[test]
fn notedown_import_lowers_heading_and_paragraph() {
    let source = "# Title\n\nHello world.\n";
    let graph = import_notedown_bytes("sample.nd", source).expect("import notedown");
    assert!(graph.blocks.len() >= 2);
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("# Title"));
    assert!(markdown.contains("Hello world"));
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
fn notedown_import_lowers_fenced_code_block() {
    let source = "```rust\nfn main() {}\n```\n";
    let graph = import_notedown_bytes("code.nd", source).expect("import notedown");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("```rust"));
    assert!(markdown.contains("fn main() {}"));
}

#[test]
fn notedown_import_lowers_blockquote() {
    let source = "> Quoted line\n";
    let graph = import_notedown_bytes("quote.nd", source).expect("import notedown");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("> Quoted line"));
}

#[test]
fn notedown_import_lowers_multiline_blockquote() {
    let source = "> Line one\n> Line two\n";
    let graph = import_notedown_bytes("quote.nd", source).expect("import notedown");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("Line one"));
    assert!(markdown.contains("Line two"));
    assert!(markdown.contains('>'));
}

#[test]
fn notedown_import_lowers_horizontal_rule() {
    let source = "Before\n\n---\n\nAfter\n";
    let graph = import_notedown_bytes("hr.nd", source).expect("import notedown");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("Before"));
    assert!(markdown.contains("---"));
    assert!(markdown.contains("After"));
}

#[test]
fn notedown_import_lowers_links() {
    let source = "Visit [home](https://example.com) today.\n";
    let graph = import_notedown_bytes("link.nd", source).expect("import notedown");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("[home](https://example.com)"));
}

#[test]
fn notedown_import_lowers_pipe_tables() {
    let source = "| Name | Value |\n|------|-------|\n| Alpha | 1 |\n| Beta | 2 |\n";
    let graph = import_notedown_bytes("table.nd", source).expect("import notedown");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("| Name | Value |"));
    assert!(markdown.contains("| Alpha | 1 |"));
    assert!(markdown.contains("| Beta | 2 |"));
    assert!(
        !markdown.contains("| ------ |"),
        "GFM separator row should not round-trip as data: {markdown}"
    );
}
