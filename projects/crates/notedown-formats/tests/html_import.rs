use notedown_formats::{export::html::export_html, import::html::import_html_text};
use notedown_ir::{Block, Inline, RelationKind, SemanticStatus};

#[test]
fn html_import_lowers_common_blocks_and_reports_partial_surface() {
    let graph = import_html_text("sample.html", r#"<html><head><title>Document title</title></head><body><h1>Title</h1><p>Hello <strong>world</strong> <a href="https://example.com">link</a>.</p><ul><li>one</li></ul></body></html>"#).expect("import html");
    assert_eq!(graph.metadata.title.as_deref(), Some("Document title"));
    assert!(matches!(graph.blocks[0].block, Block::Section { .. }));
    assert!(matches!(&graph.blocks[1].block, Block::Paragraph { content } if content.iter().any(|inline| matches!(inline, Inline::Styled { style, .. } if style == "bold"))));
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "import.html.partial_coverage" && loss.status == SemanticStatus::Partial));
}

#[test]
fn html_import_reports_unsupported_interactive_elements() {
    let graph = import_html_text("interactive.html", "<body><script>alert(1)</script><form><input></form></body>").expect("import html");
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "import.html.unsupported_script"));
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "import.html.unsupported_form"));
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "import.html.unsupported_input"));
}

#[test]
fn html_import_does_not_drop_sibling_blocks_or_nested_list_content() {
    let graph = import_html_text(
        "nested.html",
        r#"<body><div><p>first</p><p>second</p><ul><li>outer<ul><li>inner</li></ul></li></ul></div></body>"#,
    )
    .expect("import html");
    assert_eq!(graph.blocks.iter().filter(|node| matches!(node.block, Block::Paragraph { .. })).count(), 2);
    let list = graph
        .blocks
        .iter()
        .find(|node| matches!(&node.block, Block::List { items, .. } if items.iter().any(|item| item.content.iter().any(|inline| matches!(inline, Inline::Text { text } if text.contains("outer"))))))
        .expect("outer list");
    let Block::List { items, .. } = &list.block else { unreachable!() };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].children.len(), 1);
    let child = graph.block(items[0].children[0]).expect("nested list node");
    assert!(matches!(child.block, Block::List { .. }));
}

#[test]
fn html_import_registers_unresolved_image_and_reopens_export() {
    let graph = import_html_text("image.html", r#"<body><p><img src="assets/picture.png" alt="Picture"></p></body>"#).expect("import html");
    assert_eq!(graph.assets.len(), 1);
    assert_eq!(graph.relations[0].kind, RelationKind::Embeds);
    let output = export_html(&graph).expect("export html");
    assert!(output.contains(r#"<img alt="Picture" src="assets/picture.png">"#));
}
