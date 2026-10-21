use notedown_formats::import::epub::{ast_lowering::blocks_from_html_document, html_extensions::parse_html_bytes};
use notedown_ir::{Block, DocumentGraph, DocumentId};

#[test]
fn body_selector_scopes_block_lowering() {
    const XHTML: &str = r#"<html><head><title>Skip</title></head><body><p>Body</p></body></html>"#;
    let document = parse_html_bytes(XHTML.as_bytes()).expect("parse");
    let mut graph = DocumentGraph::new(DocumentId(0));
    let (blocks, _) = blocks_from_html_document(&document, &mut graph, None);
    assert_eq!(blocks.len(), 1);
    assert!(matches!(blocks[0], Block::Paragraph { .. }));
}
