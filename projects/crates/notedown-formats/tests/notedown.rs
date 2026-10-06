use notedown_formats::{
    export::markdown::export_markdown,
    import::{markdown::import_markdown_bytes, notedown::import_notedown_bytes},
};
use notedown_ir::{Block, Inline};

#[test]
fn notedown_import_lowers_heading_and_paragraph() {
    let source = "# Title\n\nHello world.\n";
    let graph = import_notedown_bytes("sample.nd", source).expect("import notedown");
    assert!(graph.blocks.len() >= 2);
    assert!(matches!(
        &graph.blocks[0].block,
        Block::Section { title, .. } if title == &[Inline::Text { text: "Title".into() }]
    ));
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("# Title"));
    assert!(markdown.contains("Hello world"));
}

#[test]
fn notedown_import_lowers_strong_and_emphasis_without_leaking_markers() {
    let graph = import_notedown_bytes("styles.nd", "Body **bold** and *italic*.\n").expect("import notedown");
    let Block::Paragraph { content } = &graph.blocks[0].block else {
        panic!("expected paragraph: {:?}", graph.blocks);
    };
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Styled { style, children }
            if style == "bold" && children == &[Inline::Text { text: "bold".into() }]
    )), "{content:?}");
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Styled { style, children }
            if style == "italic" && children == &[Inline::Text { text: "italic".into() }]
    )));
    assert!(!content.iter().any(|inline| matches!(inline, Inline::Text { text } if text.contains("**") || text.contains("*"))));
}

#[test]
fn notedown_import_lowers_nested_emphasis() {
    let graph = import_notedown_bytes("nested-styles.nd", "**bold *italic***\n").expect("import notedown");
    let Block::Paragraph { content } = &graph.blocks[0].block else {
        panic!("expected paragraph: {:?}", graph.blocks);
    };
    assert!(matches!(
        content.first(),
        Some(Inline::Styled { style, children })
            if style == "bold" && children.iter().any(|child| matches!(
                child,
                Inline::Styled { style, children }
                    if style == "italic" && children == &[Inline::Text { text: "italic".into() }]
            ))
    ), "{content:?}");
}

#[test]
fn notedown_import_preserves_separated_nested_emphasis() {
    let graph = import_notedown_bytes("nested-styles.nd", "**bold *italic* end**\n").expect("import notedown");
    let html = notedown_formats::export::html::export_html(&graph).expect("export HTML");
    assert!(html.contains("<strong>bold <em>italic</em> end</strong>"), "{html}");
}

#[test]
fn notedown_import_lowers_underscore_emphasis() {
    let graph = import_notedown_bytes("underscore.nd", "__bold__ and _italic_.\n").expect("import notedown");
    let html = notedown_formats::export::html::export_html(&graph).expect("export HTML");
    assert!(html.contains("<strong>bold</strong> and <em>italic</em>."), "{html}");
}

#[test]
fn notedown_table_cells_preserve_emphasis() {
    let graph = import_notedown_bytes("styled-table.nd", "| Name | Value |\n|------|-------|\n| **Alpha** | *one* |\n").expect("import notedown");
    let Block::Table { rows } = &graph.blocks[0].block else {
        panic!("expected table: {:?}", graph.blocks);
    };
    assert!(rows[1].cells[0].iter().any(|inline| matches!(
        inline,
        Inline::Styled { style, children }
            if style == "bold" && children == &[Inline::Text { text: "Alpha".into() }]
    )), "{:?}", rows[1].cells[0]);
    assert!(rows[1].cells[1].iter().any(|inline| matches!(
        inline,
        Inline::Styled { style, children }
            if style == "italic" && children == &[Inline::Text { text: "one".into() }]
    )), "{:?}", rows[1].cells[1]);
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
    let source = "Visit [home](https://example.com/a(b)) today.\n";
    let graph = import_notedown_bytes("link.nd", source).expect("import notedown");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains(r"[home](https://example.com/a\(b\))"), "{markdown:?}, {graph:?}");
    let reopened = import_markdown_bytes("link.md", &markdown).expect("re-import markdown");
    assert!(
        markdown.contains(" today."),
        "trailing paragraph content was lost: {markdown:?}, {graph:?}"
    );
    assert!(matches!(
        &reopened.blocks[0].block,
        Block::Paragraph { content }
            if content.iter().any(|inline| matches!(
                inline,
                Inline::Styled { style, children }
                    if style == "link"
                        && children == &[
                            Inline::Text { text: "home".into() },
                            Inline::Text { text: "https://example.com/a(b)".into() }
                        ]
            ))
    ));
}

#[test]
fn notedown_import_lowers_images_as_semantic_inline_images() {
    let source = "![diagram](assets/diagram.png)\n";
    let graph = import_notedown_bytes("image.nd", source).expect("import notedown");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("![diagram](assets/diagram.png)"), "{markdown:?}, {graph:?}");
    assert_eq!(graph.assets.len(), 1);
    assert_eq!(graph.assets[0].source.as_deref(), Some("assets/diagram.png"));
    assert_eq!(graph.assets[0].media_type.as_deref(), Some("image/png"));
    assert_eq!(graph.relations.len(), 1);
    let reopened = import_markdown_bytes("image.md", &markdown).expect("re-import markdown");
    assert!(matches!(
        &reopened.blocks[0].block,
        Block::Paragraph { content }
            if content.iter().any(|inline| matches!(
                inline,
                Inline::Styled { style, children }
                    if style == "image"
                        && children == &[
                            Inline::Text { text: "diagram".into() },
                            Inline::Text { text: "assets/diagram.png".into() }
                        ]
            ))
    ));
    assert_eq!(reopened.assets.len(), 1);
    assert_eq!(reopened.assets[0].source.as_deref(), Some("assets/diagram.png"));
    assert_eq!(reopened.relations.len(), 1);
}

#[test]
fn notedown_import_lowers_pipe_tables() {
    let source = "| Name | Value |\n|------|-------|\n| Alpha | 1 |\n| Beta | 2 |\n";
    let graph = import_notedown_bytes("table.nd", source).expect("import notedown");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("| Name | Value |"));
    assert!(markdown.contains("| Alpha | 1 |"));
    assert!(markdown.contains("| Beta | 2 |"));
    assert!(!markdown.contains("| ------ |"), "GFM separator row should not round-trip as data: {markdown}");
}
