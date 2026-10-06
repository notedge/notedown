use notedown_formats::export::html::export_html;
use notedown_ir::{Block, DocumentGraph, DocumentId, DocumentMetadata, IdAllocator, Inline};
use oak_core::{Builder, SourceText, parser::session::ParseSession};
use oak_html::{
    HtmlBuilder, HtmlLanguage,
    query::{HtmlDocumentView, select_css_elements},
};

#[test]
fn html_export_preserves_structure_and_escapes_content() {
    let mut ids = IdAllocator::default();
    let mut graph = DocumentGraph::new(ids.document_id());
    graph.metadata = DocumentMetadata { title: Some("<Title>".into()), language: None, authors: Vec::new(), tags: Vec::new() };
    graph.push_block(Block::Paragraph {
        content: vec![
            Inline::Text { text: "a < b".into() },
            Inline::Styled { style: "bold".into(), children: vec![Inline::Text { text: "strong".into() }] },
        ],
    });
    let html = export_html(&graph).expect("html export");
    assert!(html.contains("&lt;Title&gt;"));
    assert!(html.contains("<strong>strong</strong>"));
    assert!(html.contains("a &lt; b"));
    assert!(html.contains(r#"id="node-1" data-node-id="1""#));
}

#[test]
fn html_export_preserves_value_carrying_color_style() {
    let mut graph = DocumentGraph::new(DocumentId(60));
    graph.push_block(Block::Paragraph { content: vec![Inline::Styled { style: "color:ff0000".into(), children: vec![Inline::Text { text: "red".into() }] }] });
    let html = export_html(&graph).expect("html color export");
    assert!(html.contains("<span style=\"color:#ff0000\">red</span>"));
}

#[test]
fn html_export_writes_section_children_and_reference_targets() {
    let mut ids = IdAllocator::default();
    let mut graph = DocumentGraph::new(ids.document_id());
    let paragraph = graph.push_block(Block::Paragraph { content: vec![Inline::Text { text: "child".into() }] });
    let section =
        graph.push_block(Block::Section { level: 2, title: vec![Inline::Text { text: "Section".into() }], children: vec![paragraph] });
    graph.push_block(Block::Paragraph { content: vec![Inline::Reference { display: "go".into(), target: section }] });

    let html = export_html(&graph).expect("html export");
    assert!(html.contains("<h2>Section</h2>"));
    assert!(html.contains(r#"id="node-1" data-node-id="1"><p>child</p>"#));
    assert!(html.contains(r##"<a href="#node-2">go</a>"##));
}

#[test]
fn html_export_reopens_through_oak_html_and_css_selectors() {
    let mut graph = DocumentGraph::new(IdAllocator::default().document_id());
    graph.metadata.title = Some("Reopen <test>".into());
    graph.push_block(Block::Section { level: 2, title: vec![Inline::Text { text: "Heading".into() }], children: Vec::new() });
    graph.push_block(Block::Paragraph {
        content: vec![Inline::Styled {
            style: "link".into(),
            children: vec![Inline::Text { text: "link".into() }, Inline::Text { text: "https://example.com".into() }],
        }],
    });

    let html = export_html(&graph).expect("html export");
    let source = SourceText::new(html.as_str());
    let language = HtmlLanguage::default();
    let builder = HtmlBuilder::new(language);
    let mut session = ParseSession::<HtmlLanguage>::default();
    let document = builder.build(&source, &[], &mut session).result.expect("Oak HTML reopen");
    let view = HtmlDocumentView::from_document(&document);

    let (_, headings) = select_css_elements(&view, "h1, h2", Default::default()).expect("heading selector");
    let (_, paragraphs) = select_css_elements(&view, "p", Default::default()).expect("paragraph selector");
    let (_, links) = select_css_elements(&view, "a[href]", Default::default()).expect("link selector");

    assert_eq!(headings.len(), 2);
    assert_eq!(paragraphs.len(), 1);
    assert_eq!(links.len(), 1);
}

#[test]
fn html_export_writes_image_inline_semantics() {
    let mut graph = DocumentGraph::new(IdAllocator::default().document_id());
    graph.push_block(Block::Paragraph {
        content: vec![Inline::Styled {
            style: "image".into(),
            children: vec![Inline::Text { text: "diagram".into() }, Inline::Text { text: "assets/diagram.png".into() }],
        }],
    });

    let html = export_html(&graph).expect("html export");
    assert!(html.contains(r#"<img alt="diagram" src="assets/diagram.png">"#));
}

#[test]
fn html_export_rejects_unrepresentable_inline_styles() {
    let mut graph = DocumentGraph::new(IdAllocator::default().document_id());
    graph.push_block(Block::Paragraph {
        content: vec![Inline::Styled { style: "small_caps".into(), children: vec![Inline::Text { text: "meaningful".into() }] }],
    });

    let error = export_html(&graph).expect_err("style must not be discarded");
    assert!(error.to_string().contains("small_caps"));
}

#[test]
fn html_export_rejects_missing_nested_blocks_instead_of_dropping_them() {
    let mut graph = DocumentGraph::new(IdAllocator::default().document_id());
    graph.push_block(Block::Section {
        level: 1,
        title: vec![Inline::Text { text: "Broken".into() }],
        children: vec![notedown_ir::NodeId(999)],
    });

    let error = export_html(&graph).expect_err("missing child must be rejected");
    assert!(error.to_string().contains("invalid document graph"));
}

#[test]
fn html_export_preserves_list_children_and_their_reference_targets() {
    let mut graph = DocumentGraph::new(IdAllocator::default().document_id());
    let paragraph = graph.push_block(Block::Paragraph { content: vec![Inline::Text { text: "child paragraph".into() }] });
    let code = graph.push_block(Block::Code { language: Some("rust".into()), content: "value < limit".into() });
    let quote = graph.push_block(Block::Quote { content: vec![Inline::Text { text: "child quote".into() }] });
    let nested_list = graph.push_block(Block::List {
        ordered: true,
        items: vec![notedown_ir::ListItem { content: vec![Inline::Text { text: "nested item".into() }], children: Vec::new() }],
    });
    graph.push_block(Block::List {
        ordered: false,
        items: vec![notedown_ir::ListItem {
            content: vec![Inline::Reference { display: "nested target".into(), target: nested_list }],
            children: vec![paragraph, code, quote, nested_list],
        }],
    });

    let html = export_html(&graph).expect("list children must be preserved");
    let source = SourceText::new(html.as_str());
    let builder = HtmlBuilder::new(HtmlLanguage::default());
    let mut session = ParseSession::<HtmlLanguage>::default();
    let document = builder.build(&source, &[], &mut session).result.expect("Oak HTML reopen");
    let view = HtmlDocumentView::from_document(&document);
    for selector in ["li #node-1 > p", "li #node-2 > pre > code", "li #node-3 > blockquote", "li #node-4 > ol"] {
        let (_, matches) = select_css_elements(&view, selector, Default::default()).expect("child selector");
        assert_eq!(matches.len(), 1, "{selector}: {html}");
    }
    assert_eq!(html.matches("child paragraph").count(), 1);
    assert_eq!(html.matches("child quote").count(), 1);
    assert!(html.contains("value &lt; limit"));
    assert!(html.contains(r##"href="#node-4""##));
}

#[test]
fn html_export_rejects_list_containment_cycles() {
    let mut graph = DocumentGraph::new(IdAllocator::default().document_id());
    graph.push_block(Block::List {
        ordered: false,
        items: vec![notedown_ir::ListItem { content: Vec::new(), children: vec![notedown_ir::NodeId(1)] }],
    });
    let error = export_html(&graph).expect_err("containment cycles must not recurse");
    assert!(error.to_string().contains("invalid document graph"));
}
