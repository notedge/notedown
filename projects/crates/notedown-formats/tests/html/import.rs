use notedown_formats::{
    export::html::export_html,
    import::html::{HtmlLinkRole, import_html_text, import_html_with_links},
};
use notedown_ir::{Block, BreakKind, DocumentGraph, DocumentId, Inline, RelationKind, SemanticStatus, SourceKind};

#[test]
fn html_diagram_round_trips_through_the_diagram_attribute() {
    let mut graph = DocumentGraph::new(DocumentId(1));
    graph.push_block(Block::Diagram { language: "mermaid".into(), source: "flowchart TD\n  A --> B".into() });

    let html = export_html(&graph).expect("export html");
    assert!(html.contains(r#"data-diagram-language="mermaid""#), "{html}");
    let reopened = import_html_text("diagram.html", &html).expect("import html");
    assert_eq!(reopened.blocks[0].block, graph.blocks[0].block);
}

#[test]
fn html_import_reads_rendered_diagram_figures_without_leaking_svg_text() {
    let source = "flowchart TD\n  A[Start] --> B{Done?}\n  B -->|yes| C[Stop]";
    let hex = source.bytes().map(|byte| format!("{byte:02x}")).collect::<String>();
    let html = format!(
        "<body><article><figure class=\"diagram\" data-diagram-language=\"mermaid\" data-diagram-source-hex=\"{hex}\"><svg viewBox=\"0 0 10 10\"><text>rendered label</text></svg></figure></article></body>"
    );

    let reopened = import_html_text("rendered-diagram.html", &html).expect("import rendered figure");
    assert_eq!(single_diagram(&reopened), ("mermaid".to_string(), source.to_string()));
    assert!(
        !reopened.blocks.iter().any(|node| match &node.block {
            Block::Paragraph { content } => content.iter().map(Inline::plain_text).collect::<String>().contains("rendered label"),
            _ => false,
        }),
        "svg payload text must not leak into paragraphs: {:?}",
        reopened.blocks
    );
}

fn single_diagram(graph: &DocumentGraph) -> (String, String) {
    let diagrams = graph
        .blocks
        .iter()
        .filter_map(|node| match &node.block {
            Block::Diagram { language, source } => Some((language.clone(), source.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(diagrams.len(), 1, "exactly one diagram expected: {:?}", graph.blocks);
    diagrams.into_iter().next().unwrap()
}

#[test]
fn html_import_promotes_mermaid_container_classes_to_diagram_blocks() {
    let flowchart = "flowchart TD\n  A[Start] --> B{Done?}\n  B -->|yes| C[Stop]";

    let div = import_html_text("mermaid-div.html", &format!("<body><article><div class=\"mermaid\">\n{flowchart}\n</div></article></body>"))
        .expect("import div container");
    assert_eq!(single_diagram(&div), ("mermaid".to_string(), flowchart.to_string()));

    let pre = import_html_text("mermaid-pre.html", &format!("<body><pre class=\"mermaid\">\n{flowchart}\n</pre></body>")).expect("import pre");
    assert_eq!(single_diagram(&pre), ("mermaid".to_string(), flowchart.to_string()));

    let fenced =
        import_html_text("mermaid-fenced.html", &format!("<body><pre><code class=\"language-mermaid\">{flowchart}</code></pre></body>"))
            .expect("import fenced code");
    assert_eq!(single_diagram(&fenced), ("mermaid".to_string(), flowchart.to_string()));

    let shouted =
        import_html_text("mermaid-shouted.html", &format!("<body><pre><code class=\"LANGUAGE-MERMAID\">{flowchart}</code></pre></body>"))
            .expect("import upper-case marker");
    assert_eq!(single_diagram(&shouted), ("mermaid".to_string(), flowchart.to_string()));
}

#[test]
fn html_import_keeps_image_only_mermaid_container_as_bitmap_with_loss() {
    let graph = import_html_text(
        "mermaid-bitmap.html",
        r#"<body><div class="mermaid"><img src="assets/flow.png" alt="flowchart" width="640" height="480"></div></body>"#,
    )
    .expect("import image-only mermaid container");

    assert!(
        !graph.blocks.iter().any(|node| matches!(node.block, Block::Diagram { .. })),
        "an image-only container must not be promoted to an empty diagram: {:?}",
        graph.blocks
    );

    let image = graph
        .blocks
        .iter()
        .flat_map(|node| block_images(&node.block))
        .next()
        .unwrap_or_else(|| panic!("the bitmap must survive as an image: {:?}", graph.blocks));
    assert_eq!(image.0, "assets/flow.png");
    assert_eq!(image.1, vec![("width".to_string(), "640".to_string()), ("height".to_string(), "480".to_string())]);

    assert_eq!(graph.assets.len(), 1, "the bitmap must be registered as an asset: {:?}", graph.assets);
    assert_eq!(graph.assets[0].source.as_deref(), Some("assets/flow.png"));
    assert!(
        graph.coverage.loss.iter().any(|loss| loss.code == "import.html.empty_diagram_container" && loss.status == SemanticStatus::Partial),
        "the empty container must surface a structured loss: {:?}",
        graph.coverage.loss
    );
}

#[test]
fn html_import_keeps_image_only_mermaid_pre_on_the_default_path() {
    let graph = import_html_text("mermaid-pre-bitmap.html", r#"<body><pre class="mermaid"><img src="chart.png" alt="chart"></pre></body>"#)
        .expect("import image-only mermaid pre");

    assert!(
        !graph.blocks.iter().any(|node| matches!(node.block, Block::Diagram { .. } | Block::Code { .. })),
        "an image-only pre must be neither an empty diagram nor an empty code block: {:?}",
        graph.blocks
    );
    assert_eq!(graph.assets.len(), 1, "the bitmap must be registered as an asset: {:?}", graph.assets);
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "import.html.empty_diagram_container"));
}

#[test]
fn html_import_keeps_plain_images_and_textual_mermaid_containers_unchanged() {
    // Plain images keep their exact behavior: no diagram promotion, no empty
    // container loss, and no attribute wrapper when no size was authored.
    let plain = import_html_text("plain-image.html", r#"<body><p><img src="a.png" alt="A"></p></body>"#).expect("import plain image");
    let Block::Paragraph { content } = &plain.blocks[0].block
    else {
        panic!("expected a paragraph: {:?}", plain.blocks)
    };
    assert!(matches!(&content[0], Inline::Image { source, .. } if source == "a.png"), "a size-less image stays unwrapped: {:?}", content);
    assert!(!plain.coverage.loss.iter().any(|loss| loss.code == "import.html.empty_diagram_container"));

    // Textual mermaid containers still promote with their authored source.
    let flowchart = "flowchart TD\n  A --> B";
    let textual = import_html_text("mermaid-textual.html", &format!("<body><div class=\"mermaid\">\n{flowchart}\n</div></body>"))
        .expect("import textual");
    assert_eq!(single_diagram(&textual), ("mermaid".to_string(), flowchart.to_string()));
    assert!(!textual.coverage.loss.iter().any(|loss| loss.code == "import.html.empty_diagram_container"));
}

/// Collect every image in a block together with its attribute pairs,
/// descending through `Attributed` wrappers.
fn block_images(block: &Block) -> Vec<(String, Vec<(String, String)>)> {
    let mut images = Vec::new();
    let content: &[Inline] = match block {
        Block::Paragraph { content } => content,
        _ => &[],
    };
    collect_inline_images(content, &[], &mut images);
    images
}

fn collect_inline_images(inlines: &[Inline], inherited: &[(String, String)], images: &mut Vec<(String, Vec<(String, String)>)>) {
    for inline in inlines {
        match inline {
            Inline::Image { source, .. } => images.push((source.clone(), inherited.to_vec())),
            Inline::Attributed { attributes, children } => {
                let pairs = attributes.pairs.iter().map(|pair| (pair.key.clone(), pair.value.clone())).collect::<Vec<_>>();
                collect_inline_images(children, &pairs, images);
            }
            Inline::Styled { children, .. } => collect_inline_images(children, inherited, images),
            _ => {}
        }
    }
}

#[test]
fn html_import_keeps_inline_svg_as_raw_markup_without_leaking_label_text() {
    let graph = import_html_text(
        "inline-svg.html",
        r#"<body><p>Status <svg viewBox="0 0 10 10" width="10" height="10"><circle cx="5" cy="5" r="4"/><text>icon</text></svg> online</p></body>"#,
    )
    .expect("import inline svg");
    let Block::Paragraph { content } = &graph.blocks[0].block
    else {
        panic!("paragraph expected, got {:?}", graph.blocks)
    };
    let svg = content.iter().find_map(|inline| match inline {
        Inline::RawHtml { html } => Some(html.clone()),
        _ => None,
    });
    let Some(svg) = svg
    else {
        panic!("inline svg must stay raw markup, got {content:?}")
    };
    assert_eq!(svg, r#"<svg viewBox="0 0 10 10" width="10" height="10"><circle cx="5" cy="5" r="4"/><text>icon</text></svg>"#);
    let prose = content.iter().map(Inline::plain_text).collect::<String>();
    assert!(!prose.contains("icon"), "svg label text must not leak into prose: {prose:?}");
    assert!(prose.contains("Status") && prose.contains("online"), "{prose:?}");
}

#[test]
fn html_import_keeps_block_svg_verbatim_and_reopens_it() {
    let markup = r#"<svg viewBox="0 0 100 40"><rect width="100" height="40"/><text x="4" y="20">gauge</text></svg>"#;
    let graph = import_html_text("block-svg.html", &format!("<body><figure>{markup}</figure></body>")).expect("import block svg");
    let Block::RawHtml { html } = &graph.blocks[0].block
    else {
        panic!("raw html block expected, got {:?}", graph.blocks)
    };
    assert_eq!(html, markup);

    let output = export_html(&graph).expect("export html");
    assert!(output.contains(markup), "{output}");
}

#[test]
fn html_import_keeps_non_diagram_classes_on_the_default_path() {
    let lookalike = import_html_text("mermaid-lookalike.html", "<body><div class=\"mermaid-theme\"><p>not a diagram</p></div></body>")
        .expect("import lookalike");
    assert!(single_diagram_opt(&lookalike).is_none(), "substring classes must not match: {:?}", lookalike.blocks);

    let rust = import_html_text("rust-code.html", "<body><pre><code class=\"language-rust\">fn main() {}</code></pre></body>")
        .expect("import rust code");
    let Block::Code { language, content } = &rust.blocks[0].block
    else {
        panic!("plain code block expected, got {:?}", rust.blocks)
    };
    assert_eq!(language.as_deref(), Some("language-rust"));
    assert_eq!(content, "fn main() {}");
}

fn single_diagram_opt(graph: &DocumentGraph) -> Option<(String, String)> {
    graph.blocks.iter().find_map(|node| match &node.block {
        Block::Diagram { language, source } => Some((language.clone(), source.clone())),
        _ => None,
    })
}

#[test]
fn html_mermaid_containers_reopen_through_markdown_as_diagrams() {
    use notedown_formats::{export::markdown::export_markdown, import::markdown::import_markdown_bytes};

    let sequence = "sequenceDiagram\n  Alice->>Bob: Hi";
    let graph =
        import_html_text("mermaid-roundtrip.html", &format!("<body><div class=\"mermaid\">\n{sequence}\n</div></body>")).expect("import html");
    assert_eq!(single_diagram(&graph).0, "mermaid");

    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("```mermaid"), "{markdown}");
    let reopened = import_markdown_bytes("mermaid-roundtrip.md", &markdown).expect("reopen markdown");
    assert_eq!(single_diagram(&reopened), ("mermaid".to_string(), sequence.to_string()));
}

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
fn html_import_numbers_ordered_list_items_and_keeps_unordered_items_bare() {
    let graph = import_html_text(
        "ordered.html",
        r#"<body><ol><li>first</li><li>second</li></ol><ol start="5"><li>shifted</li></ol><ul><li>bare</li></ul></body>"#,
    )
    .expect("import html");
    let lists: Vec<&Vec<notedown_ir::ListItem>> = graph
        .blocks
        .iter()
        .filter_map(|node| match &node.block {
            Block::List { items, .. } => Some(items),
            _ => None,
        })
        .collect();
    assert_eq!(lists.len(), 3, "two ordered lists and one unordered list");
    assert_eq!(lists[0].iter().map(|item| item.number).collect::<Vec<_>>(), vec![Some(1), Some(2)]);
    assert_eq!(lists[1].iter().map(|item| item.number).collect::<Vec<_>>(), vec![Some(5)], "`start` shifts the sequence");
    assert_eq!(lists[2].iter().map(|item| item.number).collect::<Vec<_>>(), vec![None], "unordered items stay unnumbered");
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

#[test]
fn html_footnote_reference_and_endnotes_round_trip_through_markdown() {
    use notedown_formats::{export::markdown::export_markdown, import::markdown::import_markdown_bytes};

    let source = "See[^1] after\n\n[^1]: Body.\n";
    let graph = import_markdown_bytes("footnote.md", source).expect("import markdown");
    let html = export_html(&graph).expect("export html");

    assert!(html.contains(r#"<section class="footnotes" role="doc-endnotes">"#), "{html}");
    assert!(html.contains(r#"data-footnote-label="1""#), "{html}");
    assert!(!html.contains("[^1]:"), "the authored GFM payload must not leak as prose: {html}");

    let reopened = import_html_text("footnote.html", &html).expect("import html");
    let markdown = export_markdown(&reopened).expect("export markdown");
    assert_eq!(markdown.trim_end(), source.trim_end(), "footnote round trip must preserve the authored definition byte for byte");

    let rebound = import_markdown_bytes("footnote-reopen.md", &markdown).expect("reopen markdown");
    assert!(matches!(
        &rebound.blocks[0].block,
        Block::Paragraph { content } if content.iter().any(|inline| matches!(inline, Inline::Note { label, .. } if label == "1"))
    ));
    assert!(rebound.blocks.iter().any(|node| matches!(
        &node.block,
        Block::Opaque { kind, payload_hint, .. } if kind == "footnote_definition" && payload_hint.trim() == "[^1]: Body."
    )));
}

#[test]
fn html_import_reads_pandoc_style_footnotes() {
    let html = r##"<body><p>Text<a href="#fn1" class="footnote-ref" id="fnref1" role="doc-noteref"><sup>1</sup></a></p><section class="footnotes" role="doc-endnotes"><ol><li id="fn1"><p>Body.</p></li></ol></section></body>"##;
    let graph = import_html_text("pandoc.html", html).expect("import pandoc footnotes");

    let Block::Paragraph { content } = &graph.blocks[0].block
    else {
        panic!("paragraph expected, got {:?}", graph.blocks)
    };
    assert!(content.iter().any(|inline| matches!(inline, Inline::Note { label, display: None } if label == "1")), "{content:?}");
    assert!(graph.blocks.iter().any(|node| matches!(
        &node.block,
        Block::Opaque { kind, payload_hint, .. } if kind == "footnote_definition" && payload_hint == "[^1]: Body."
    )));
}

#[test]
fn html_import_records_flattened_table_cell_blocks() {
    let graph =
        import_html_text("cell-blocks.html", "<body><table><tr><th>Head</th></tr><tr><td><p>one</p><p>two</p></td></tr></table></body>")
            .expect("import html");
    let Block::Table { rows, .. } = &graph.blocks[0].block
    else {
        panic!("table expected, got {:?}", graph.blocks)
    };
    assert_eq!(rows[1].cells[0].content.iter().map(Inline::plain_text).collect::<String>(), "onetwo");
    assert!(
        graph.coverage.loss.iter().any(|loss| loss.code == "import.html.table_cell_blocks" && loss.status == SemanticStatus::Partial),
        "{:?}",
        graph.coverage.loss
    );
}
