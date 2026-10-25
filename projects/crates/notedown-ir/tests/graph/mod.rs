use notedown_ir::{Block, DocumentGraph, IdAllocator, Inline, LossMarker, SemanticStatus, SourceKind, SourceRef};

#[test]
fn construct_document_without_parser() {
    let mut ids = IdAllocator::default();
    let doc_id = ids.document_id();
    let mut graph = DocumentGraph::new(doc_id);

    let paragraph = graph.push_block(Block::Paragraph { content: vec![Inline::Text { text: "Hello".to_string() }] });

    graph.attach_source(SourceRef { node: paragraph, kind: SourceKind::Manual, precision: SemanticStatus::Resolved });

    let json = serde_json::to_string(&graph).expect("serialize");
    let restored: DocumentGraph = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.blocks.len(), 1);
    assert_eq!(restored.id, doc_id);
    assert_eq!(restored.revision, graph.revision);
}

#[test]
fn revision_tracks_semantic_updates() {
    let mut ids = IdAllocator::default();
    let mut graph = DocumentGraph::new(ids.document_id());
    assert_eq!(graph.revision, 0);

    graph.set_metadata(notedown_ir::DocumentMetadata {
        title: Some("Updated".to_string()),
        language: None,
        authors: Vec::new(),
        tags: Vec::new(),
    });
    assert_eq!(graph.revision, 1);

    let asset_id = ids.asset_id();
    graph.push_asset(notedown_ir::Asset {
        id: asset_id,
        kind: notedown_ir::AssetKind::Attachment,
        content_identity: None,
        source: None,
        media_type: None,
        status: SemanticStatus::Resolved,
        bytes: None,
    });
    assert!(graph.set_asset_source(asset_id, Some("attachment.bin".to_string())));
    assert_eq!(graph.revision, 2);

    assert!(!graph.set_asset_source(ids.asset_id(), None));
    assert_eq!(graph.revision, 2);

    let encoded = serde_json::to_string(&graph).expect("serialize");
    let restored: DocumentGraph = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(restored.revision, 2);
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

#[test]
fn ordered_list_number_survives_wire_roundtrip() {
    use notedown_ir::ListItem;
    let mut graph = DocumentGraph::new(IdAllocator::default().document_id());
    graph.push_block(Block::List {
        ordered: true,
        items: vec![
            ListItem { content: Vec::new(), children: Vec::new(), number: Some(42) },
            ListItem { content: Vec::new(), children: Vec::new(), number: Some(99) },
        ],
    });

    let json = serde_json::to_string(&graph).expect("serialize");
    let restored: DocumentGraph = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored.blocks.len(), 1);
    if let Block::List { ordered, items, .. } = &restored.blocks[0].block {
        assert!(ordered);
        assert_eq!(items[0].number, Some(42));
        assert_eq!(items[1].number, Some(99));
    }
    else {
        panic!("expected List block");
    }
}

#[test]
fn diagram_source_survives_wire_roundtrip() {
    let mut graph = DocumentGraph::new(IdAllocator::default().document_id());
    graph.push_block(Block::Diagram { language: "mermaid".into(), source: "flowchart TD\n  A --> B".into() });

    let json = serde_json::to_string(&graph).expect("serialize");
    assert!(
        json.contains(r#""Diagram":{"language":"mermaid","source":"flowchart TD\n  A --> B"}"#),
        "diagram keeps only language and raw source on the wire: {json}"
    );

    let restored: DocumentGraph = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.blocks[0].block, Block::Diagram { language: "mermaid".into(), source: "flowchart TD\n  A --> B".into() });
    assert!(restored.validate().is_valid());
}

#[test]
fn unordered_list_skips_number_on_wire() {
    use notedown_ir::ListItem;
    let mut graph = DocumentGraph::new(IdAllocator::default().document_id());
    graph.push_block(Block::List {
        ordered: false,
        items: vec![
            ListItem { content: Vec::new(), children: Vec::new(), number: None },
            ListItem { content: Vec::new(), children: Vec::new(), number: None },
        ],
    });

    let json = serde_json::to_string(&graph).expect("serialize");
    assert!(
        !json.contains("number"),
        "unordered list should not serialize the missing number field, got: {json}"
    );

    let restored: DocumentGraph = serde_json::from_str(&json).expect("deserialize");
    if let Block::List { items, .. } = &restored.blocks[0].block {
        assert!(items.iter().all(|item| item.number.is_none()));
    }
    else {
        panic!("expected List block");
    }
}
