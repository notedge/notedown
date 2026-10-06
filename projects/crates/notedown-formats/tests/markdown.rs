use notedown_formats::{export::markdown::export_markdown, import::markdown::import_markdown_bytes};
use notedown_ir::{Block, DocumentGraph, DocumentId, Inline, RelationEndpoint, RelationKind, SemanticStatus};

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
    let source = "Hello **bold** and *italic* with [link](https://example.com/a(b)).\n";
    let graph = import_markdown_bytes("inline.md", source).expect("import markdown");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("**bold**"));
    assert!(markdown.contains("*italic*"));
    assert!(markdown.contains(r"[link](https://example.com/a\(b\))"), "markdown: {markdown:?}");
    assert!(markdown.contains(r" with [link](https://example.com/a\(b\))."));
}

#[test]
fn markdown_reference_links_resolve_definitions_and_reopen_as_inline_links() {
    let source = "Read [the guide][Guide] and [guide][].\n\n[guide]: <https://example.com/guide> \"Guide\"\n";
    let graph = import_markdown_bytes("reference.md", source).expect("import markdown");
    let markdown = export_markdown(&graph).expect("export markdown");
    let reopened = import_markdown_bytes("reopened.md", &markdown).expect("re-import markdown");
    let Block::Paragraph { content } = &reopened.blocks[0].block else { panic!("paragraph") };
    let links = content
        .iter()
        .filter_map(|inline| match inline {
            Inline::Styled { style, children } if style == "link" => Some(children),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(links.len(), 2, "{content:#?}; output={markdown:?}");
    assert!(links.iter().all(|children| children[1] == Inline::Text { text: "https://example.com/guide".into() }));
    assert!(graph.validate().is_valid());
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "import.markdown.reference_definition_title"));
}

#[test]
fn markdown_shortcut_reference_resolves_and_reopens() {
    let source = "Read [Guide] now.\n\n[guide]: https://example.com/guide\n";
    let graph = import_markdown_bytes("shortcut.md", source).expect("import markdown");
    let markdown = export_markdown(&graph).expect("export markdown");
    let reopened = import_markdown_bytes("shortcut-output.md", &markdown).expect("reopen markdown");
    let Block::Paragraph { content } = &reopened.blocks[0].block else { panic!("paragraph") };
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Styled { style, children }
            if style == "link" && children.iter().any(|child| matches!(child, Inline::Text { text } if text == "https://example.com/guide"))
    )));
    assert!(graph.coverage.loss.is_empty(), "{:#?}", graph.coverage.loss);
}

#[test]
fn markdown_undefined_shortcut_is_literal_without_link_loss() {
    let source = "Keep [literal] and ![literal image] unchanged.";
    let graph = import_markdown_bytes("literal-brackets.md", source).expect("import markdown");
    assert!(graph.coverage.loss.is_empty(), "{:#?}", graph.coverage.loss);
    assert!(graph.assets.is_empty());
    let Block::Paragraph { content } = &graph.blocks[0].block else { panic!("paragraph") };
    assert!(content.iter().all(|inline| matches!(inline, Inline::Text { .. })));
    let markdown = export_markdown(&graph).expect("export");
    let reopened = import_markdown_bytes("literal-output.md", &markdown).expect("reopen");
    assert_eq!(reopened.blocks[0].block, graph.blocks[0].block);
}

#[test]
fn markdown_unresolved_reference_links_are_preserved_and_reported() {
    let graph = import_markdown_bytes("unresolved.md", "before [label][missing] after\n\n").expect("import markdown");
    let Block::Paragraph { content } = &graph.blocks[0].block else { panic!("paragraph") };
    assert!(content.iter().any(|inline| matches!(inline, Inline::Text { text } if text.contains("[label][missing]"))), "{content:#?}");
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "import.markdown.unresolved_reference_link"));
}

#[test]
fn markdown_reference_images_resolve_and_register_assets() {
    let source = "![diagram][figure]\n\n[figure]: assets/diagram.png\n";
    let graph = import_markdown_bytes("reference-image.md", source).expect("import markdown");
    assert_eq!(graph.assets.len(), 1);
    assert_eq!(graph.assets[0].source.as_deref(), Some("assets/diagram.png"));
    assert!(matches!(
        &graph.blocks[0].block,
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
}

#[test]
fn markdown_round_trip_validates() {
    let source = "# Route\n\nBody.\n";
    let graph = import_markdown_bytes("route.md", source).expect("import");
    assert!(graph.validate().is_valid());
    let markdown = export_markdown(&graph).expect("export");
    assert!(markdown.contains("# Route"));
}

#[test]
fn markdown_math_reopen_preserves_semantics_and_following_text() {
    let source = "before $x^2 + y^2$ after\n\n$$E = mc^2$$\n";
    let graph = import_markdown_bytes("math.md", source).expect("import");
    let Block::Paragraph { content } = &graph.blocks[0].block else { panic!("paragraph") };
    assert_eq!(content, &vec![
        Inline::Text { text: "before ".into() },
        Inline::InlineMath { content: "x^2 + y^2".into(), language: Some("latex".into()) },
        Inline::Text { text: " after".into() },
    ]);
    assert!(graph.blocks.iter().any(|node| node.block == Block::Math {
        content: "E = mc^2".into(), language: Some("latex".into()),
    }));
    let markdown = export_markdown(&graph).expect("export");
    let reopened = import_markdown_bytes("math-round.md", &markdown).expect("reopen");
    assert_eq!(reopened.blocks[0].block, graph.blocks[0].block);
    assert!(reopened.blocks.iter().any(|node| node.block == Block::Math {
        content: "E = mc^2".into(), language: Some("latex".into()),
    }));
}

#[test]
fn markdown_footnote_reopen_preserves_reference_body_and_following_text() {
    let source = "See[^1] after\n\n[^1]: Body.\n";
    let graph = import_markdown_bytes("footnote.md", source).expect("import");
    let Block::Paragraph { content } = &graph.blocks[0].block else { panic!("paragraph") };
    assert_eq!(content, &vec![
        Inline::Text { text: "See".into() },
        Inline::Styled { style: "footnote_reference".into(), children: vec![Inline::Text { text: "1".into() }] },
        Inline::Text { text: " after".into() },
    ]);
    let markdown = export_markdown(&graph).expect("export");
    let reopened = import_markdown_bytes("footnote-round.md", &markdown).expect("reopen");
    assert_eq!(reopened.blocks[0].block, graph.blocks[0].block);
    assert!(reopened.blocks.iter().any(|node| matches!(&node.block,
        Block::Opaque { kind, payload_hint, .. } if kind == "footnote_definition" && payload_hint.trim() == "[^1]: Body."
    )));
}

#[test]
fn markdown_export_does_not_silently_drop_reference_identity() {
    let mut graph = DocumentGraph::new(DocumentId(11));
    let target = graph.push_block(Block::Paragraph { content: vec![Inline::Text { text: "target".into() }] });
    graph.push_block(Block::Paragraph { content: vec![Inline::Reference { target, display: "see target".into() }] });
    assert!(export_markdown(&graph).expect_err("reference identity cannot be discarded").to_string().contains("anchor mapping"));
}

#[test]
fn markdown_export_preserves_nested_section_content_on_reopen() {
    let mut graph = DocumentGraph::new(DocumentId(9));
    let lead = graph.push_block(Block::Paragraph {
        content: vec![Inline::Text { text: "lead paragraph".into() }],
    });
    let nested_body = graph.push_block(Block::Paragraph {
        content: vec![Inline::Text { text: "nested body".into() }],
    });
    let nested_section = graph.push_block(Block::Section {
        level: 2,
        title: vec![Inline::Text { text: "Nested".into() }],
        children: vec![nested_body],
    });
    graph.push_block(Block::Section {
        level: 1,
        title: vec![Inline::Text { text: "Root".into() }],
        children: vec![lead, nested_section],
    });

    let markdown = export_markdown(&graph).expect("export");
    let reopened = import_markdown_bytes("nested.md", &markdown).expect("reopen");
    let reopened_markdown = reopened
        .blocks
        .iter()
        .filter_map(|node| match &node.block {
            Block::Section { title, .. } => Some(title
                .iter()
                .filter_map(|inline| match inline {
                    Inline::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>()),
            Block::Paragraph { content } => {
                let text = content
                .iter()
                .filter_map(|inline| match inline {
                    Inline::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>();
                (!text.is_empty()).then_some(text)
            }
            other => panic!("unexpected reopened block: {other:?}"),
        })
        .collect::<Vec<_>>();

    assert_eq!(reopened_markdown, ["Root", "lead paragraph", "Nested", "nested body"]);
}

#[test]
fn markdown_image_reopen_preserves_asset_relation_and_surrounding_text() {
    let source = "before ![diagram](images/diagram.png) after\n";
    let graph = import_markdown_bytes("image.md", source).expect("import");
    let markdown = export_markdown(&graph).expect("export");
    let reopened = import_markdown_bytes("image-roundtrip.md", &markdown).expect("reopen");

    assert_eq!(reopened.assets.len(), 1);
    let asset = &reopened.assets[0];
    assert_eq!(asset.source.as_deref(), Some("images/diagram.png"));
    assert_eq!(asset.status, SemanticStatus::Unresolved);
    assert!(reopened.relations.iter().any(|relation| {
        relation.kind == RelationKind::Embeds
            && relation.target == RelationEndpoint::Asset(asset.id)
    }));
    let Block::Paragraph { content } = &reopened.blocks[0].block else {
        panic!("expected reopened paragraph")
    };
    assert!(matches!(&content[0], Inline::Text { text } if text == "before "));
    assert!(matches!(&content[1], Inline::Styled { style, children } if style == "image" && children.len() == 2));
    assert!(matches!(&content[2], Inline::Text { text } if text == " after"));
}

#[test]
fn markdown_export_rejects_containment_cycles() {
    let mut graph = DocumentGraph::new(DocumentId(10));
    let section = graph.push_block(Block::Section {
        level: 1,
        title: vec![Inline::Text { text: "Cycle".into() }],
        children: Vec::new(),
    });
    if let Block::Section { children, .. } = &mut graph.blocks[0].block {
        children.push(section);
    }

    let error = export_markdown(&graph).expect_err("invalid graph must not recurse");
    assert!(error.to_string().contains("ContainmentCycle"));
}

#[test]
fn markdown_export_escapes_literal_syntax_characters() {
    let mut graph = DocumentGraph::new(DocumentId(1));
    graph.push_block(Block::Paragraph { content: vec![Inline::Text { text: "literal *stars* and [brackets] | marker".into() }] });

    let markdown = export_markdown(&graph).expect("export");
    assert!(markdown.contains(r"literal \*stars\* and \[brackets\] \| marker"));

    let reopened = import_markdown_bytes("escaped.md", &markdown).expect("re-import");
    assert_eq!(
        reopened.blocks[0].block,
        Block::Paragraph { content: vec![Inline::Text { text: "literal *stars* and [brackets] | marker".into() }] }
    );
}

#[test]
fn markdown_export_rejects_unrepresentable_inline_styles() {
    let mut graph = DocumentGraph::new(DocumentId(1));
    graph.push_block(Block::Paragraph {
        content: vec![Inline::Styled { style: "small_caps".into(), children: vec![Inline::Text { text: "meaningful style".into() }] }],
    });

    let error = export_markdown(&graph).expect_err("style must not be silently dropped");
    assert!(error.to_string().contains("small_caps"));
}

#[test]
fn markdown_export_chooses_safe_code_delimiters() {
    let mut graph = DocumentGraph::new(DocumentId(2));
    graph.push_block(Block::Paragraph { content: vec![Inline::InlineCode { text: "`a``b`".into() }] });
    graph.push_block(Block::Code { language: None, content: "line with ``` inside".into() });

    let markdown = export_markdown(&graph).expect("export");
    assert!(markdown.starts_with("``` `a``b` ```\n\n"));
    assert!(markdown.contains("````\nline with ``` inside\n````"));
}
