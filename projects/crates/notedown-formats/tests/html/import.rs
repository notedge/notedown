use notedown_formats::{
    export::html::export_html,
    import::html::{HtmlLinkRole, import_html_text, import_html_with_links},
};
use notedown_ir::{Block, BreakKind, Inline, RelationKind, SemanticStatus, SourceKind};

#[test]
fn html_import_records_block_source_spans() {
    let html = "<body><h1>Title</h1><p>Hello</p></body>";
    let graph = import_html_text("spans.html", html).expect("import html");
    assert_eq!(graph.blocks.len(), 2);
    assert_eq!(graph.sources.len(), 2);
    for source in &graph.sources {
        let SourceKind::TextSpan { start, end } = source.kind
        else {
            panic!("text span expected, got {:?}", source.kind)
        };
        assert!(start < end, "{source:?}");
        assert!(end as usize <= html.len(), "{source:?}");
        assert_eq!(source.precision, SemanticStatus::Resolved);
    }
}

#[test]
fn html_import_keeps_inline_content_under_containers() {
    let graph = import_html_text(
        "container.html",
        r#"<body><div>Loose text <img src="a.png" alt="A"> tail</div><figure><img src="b.png" alt="B"></figure></body>"#,
    )
    .expect("import html");
    let paragraphs = graph.blocks.iter().filter(|node| matches!(node.block, Block::Paragraph { .. })).count();
    assert_eq!(paragraphs, 2, "{:?}", graph.blocks);
    assert_eq!(graph.assets.len(), 2);
}

#[test]
fn html_import_decodes_character_references_and_keeps_word_boundaries() {
    let graph = import_html_text(
        "entities.html",
        "<html><head><title>Case &amp; Title</title></head><body><p>hello <strong>world</strong> again</p></body></html>",
    )
    .expect("import html");
    assert_eq!(graph.metadata.title.as_deref(), Some("Case & Title"));
    let Some(node) = graph.blocks.iter().find(|node| matches!(node.block, Block::Paragraph { .. }))
    else {
        panic!("expected paragraph, got {:?}", graph.blocks)
    };
    let Block::Paragraph { content } = &node.block
    else {
        unreachable!()
    };
    assert!(matches!(content.first(), Some(Inline::Text { text }) if text == "hello "), "{content:?}");
    assert!(matches!(content.last(), Some(Inline::Text { text }) if text == " again"), "{content:?}");
}

#[test]
fn html_import_reports_link_candidates_with_base_and_source_spans() {
    let html = r#"<html><head><base href="/docs/"><script src="/skip.js"></script></head><body><a href="a.html">A</a><img src="i.png" srcset="s1.png 1x, s2.png 2x"><form action="/submit"></form></body></html>"#;
    let import = import_html_with_links("links.html", html).expect("import html");
    assert_eq!(import.base_href.as_deref(), Some("/docs/"));
    let roles = import.links.iter().map(|link| (link.target.as_str(), link.role)).collect::<Vec<_>>();
    assert_eq!(
        roles,
        vec![
            ("a.html", HtmlLinkRole::Href),
            ("i.png", HtmlLinkRole::Src),
            ("s1.png", HtmlLinkRole::SrcSet),
            ("s2.png", HtmlLinkRole::SrcSet),
            ("/submit", HtmlLinkRole::FormAction),
        ]
    );
    for link in &import.links {
        assert!(link.start < link.end, "{link:?}");
        assert!(link.end <= html.len(), "{link:?}");
    }
}

#[test]
fn html_import_keeps_link_targets_verbatim() {
    let import = import_html_with_links("verbatim.html", r#"<body><a href=" ../a?x=1#f ">A</a></body>"#).expect("import html");
    assert_eq!(import.links[0].target, "../a?x=1#f");
}

#[test]
fn html_import_lowers_common_blocks_and_reports_partial_surface() {
    let graph = import_html_text("sample.html", r#"<html><head><title>Document title</title></head><body><h1>Title</h1><p>Hello <strong>world</strong> <a href="https://example.com">link</a>.</p><ul><li>one</li></ul></body></html>"#).expect("import html");
    assert_eq!(graph.metadata.title.as_deref(), Some("Document title"));
    assert!(matches!(graph.blocks[0].block, Block::Section { .. }));
    assert!(
        matches!(&graph.blocks[1].block, Block::Paragraph { content } if content.iter().any(|inline| matches!(inline, Inline::Styled { style, .. } if style == "bold")))
    );
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
    let graph =
        import_html_text("nested.html", r#"<body><div><p>first</p><p>second</p><ul><li>outer<ul><li>inner</li></ul></li></ul></div></body>"#)
            .expect("import html");
    assert_eq!(graph.blocks.iter().filter(|node| matches!(node.block, Block::Paragraph { .. })).count(), 2);
    let list = graph
        .blocks
        .iter()
        .find(|node| matches!(&node.block, Block::List { items, .. } if items.iter().any(|item| item.content.iter().any(|inline| matches!(inline, Inline::Text { text } if text.contains("outer"))))))
        .expect("outer list");
    let Block::List { items, .. } = &list.block
    else {
        unreachable!()
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].children.len(), 1);
    let child = graph.block(items[0].children[0]).expect("nested list node");
    assert!(matches!(child.block, Block::List { .. }));
}

#[test]
fn html_import_lowers_breaks_to_a_first_class_inline() {
    let graph = import_html_text("break.html", "<body><p>first<br>second</p></body>").expect("import html");
    let Block::Paragraph { content } = &graph.blocks[0].block
    else {
        panic!("paragraph expected")
    };
    assert!(matches!(content[1], Inline::Break { kind: BreakKind::Hard }), "{content:?}");
    let output = export_html(&graph).expect("export html");
    assert!(output.contains("first<br>second"), "{output}");
}

#[test]
fn html_import_preserves_authored_inline_identity_and_classes() {
    let graph =
        import_html_text("attributed.html", r#"<body><p><span id="anchor" class="lead note">Marked</span></p></body>"#).expect("import html");
    let Block::Paragraph { content } = &graph.blocks[0].block
    else {
        panic!("paragraph expected")
    };
    let Inline::Attributed { attributes, children } = &content[0]
    else {
        panic!("attributed inline expected, got {content:?}")
    };
    assert_eq!(attributes.id.as_deref(), Some("anchor"));
    assert_eq!(attributes.classes, ["lead", "note"]);
    assert!(attributes.pairs.is_empty());
    assert_eq!(children, &[Inline::Text { text: "Marked".into() }]);
    let output = export_html(&graph).expect("export html");
    assert!(output.contains(r#"<span id="anchor" class="lead note">Marked</span>"#), "{output}");
}

#[test]
fn html_import_maps_inline_styles_to_canonical_ir_names() {
    let graph = import_html_text("styles.html", "<body><p><s>gone</s> <del>old</del> <u>under</u> <sup>up</sup> <sub>down</sub></p></body>")
        .expect("import html");
    let Block::Paragraph { content } = &graph.blocks[0].block
    else {
        panic!("paragraph expected")
    };
    let styles = content
        .iter()
        .filter_map(|inline| match inline {
            Inline::Styled { style, children } => Some((style.as_str(), children.iter().map(Inline::plain_text).collect::<String>())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        styles,
        vec![
            ("strike", "gone".to_string()),
            ("strike", "old".to_string()),
            ("underline", "under".to_string()),
            ("superscript", "up".to_string()),
            ("subscript", "down".to_string()),
        ]
    );

    let output = export_html(&graph).expect("export html");
    for expected in ["<s>gone</s>", "<s>old</s>", "<u>under</u>", "<sup>up</sup>", "<sub>down</sub>"] {
        assert!(output.contains(expected), "{expected}: {output}");
    }
}

#[test]
fn html_import_registers_unresolved_image_and_reopens_export() {
    let graph = import_html_text("image.html", r#"<body><p><img src="assets/picture.png" alt="Picture"></p></body>"#).expect("import html");
    assert_eq!(graph.assets.len(), 1);
    assert_eq!(graph.relations[0].kind, RelationKind::Embeds);
    let output = export_html(&graph).expect("export html");
    assert!(output.contains(r#"<img alt="Picture" src="assets/picture.png">"#));
}
