use notedown_ir::{
    Block, DocumentGraph, IdAllocator, Inline, LossMarker, SemanticStatus, SourceKind, SourceRef,
};

#[test]
fn construct_document_without_parser() {
    let mut ids = IdAllocator::default();
    let doc_id = ids.document_id();
    let mut graph = DocumentGraph::new(doc_id);

    let paragraph = graph.push_block(Block::Paragraph {
        content: vec![Inline::Text {
            text: "Hello".to_string(),
        }],
    });

    graph.attach_source(SourceRef {
        node: paragraph,
        kind: SourceKind::Manual,
        precision: SemanticStatus::Resolved,
    });

    let json = serde_json::to_string(&graph).expect("serialize");
    let restored: DocumentGraph = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.blocks.len(), 1);
    assert_eq!(restored.id, doc_id);
}

#[test]
fn loss_is_explicit() {
    let mut graph = DocumentGraph::new(IdAllocator::default().document_id());
    graph.push_loss(LossMarker {
        code: "panduck.unsupported.style".to_string(),
        message: "native style retained as extension".to_string(),
        status: SemanticStatus::Lossy,
    });
    assert!(!graph.coverage.complete);
    assert_eq!(graph.coverage.loss.len(), 1);
}
