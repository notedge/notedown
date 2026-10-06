//! Living `00` acceptance probes — no parser, AST, or source file required.

use notedown_ir::{
    Asset, AssetId, AssetKind, Block, DocumentEnvelope, DocumentGraph, DocumentMetadata, IdAllocator,
    Inline, LinkId, NodeId, Relation, RelationEndpoint, RelationKind, SemanticStatus, SourceKind,
    SourceRef, TableRow,
};

fn sample_graph() -> (DocumentGraph, NodeId, AssetId, LinkId) {
    let mut ids = IdAllocator::default();
    let doc_id = ids.document_id();
    let mut graph = DocumentGraph::new(doc_id);

    let table_id = graph.push_block(Block::Table {
        rows: vec![
            TableRow {
                cells: vec![vec![Inline::Text {
                    text: "cell".to_string(),
                }]],
            },
        ],
    });

    let asset_id = ids.asset_id();
    graph.push_asset(Asset {
        id: asset_id,
        kind: AssetKind::Image,
        content_identity: Some("sha256:abc".to_string()),
        source: Some("media/photo.png".to_string()),
        media_type: Some("image/png".to_string()),
        status: SemanticStatus::Resolved,
        bytes: None,
    });

    let link_id = ids.link_id();
    graph.push_relation(Relation {
        id: link_id,
        kind: RelationKind::References,
        source: RelationEndpoint::Node(table_id),
        target: RelationEndpoint::Asset(asset_id),
    });

    graph.push_relation(Relation {
        id: ids.link_id(),
        kind: RelationKind::Backlink,
        source: RelationEndpoint::Asset(asset_id),
        target: RelationEndpoint::Node(table_id),
    });

    graph.attach_source(SourceRef {
        node: table_id,
        kind: SourceKind::PackageMember {
            path: "word/document.xml".to_string(),
        },
        precision: SemanticStatus::Resolved,
    });
    graph.attach_source(SourceRef {
        node: table_id,
        kind: SourceKind::Synthetic {
            reason: "merged runs".to_string(),
        },
        precision: SemanticStatus::Inferred,
    });

    (graph, table_id, asset_id, link_id)
}

fn same_paragraph_text() -> Vec<Inline> {
    vec![
        Inline::Text {
            text: "Hello ".to_string(),
        },
        Inline::Styled {
            style: "emphasis".to_string(),
            children: vec![Inline::Text {
                text: "world".to_string(),
            }],
        },
    ]
}

#[test]
fn round_trip_preserves_identity_relations_and_assets() {
    let (graph, table_id, asset_id, link_id) = sample_graph();
    assert!(graph.validate().is_valid());

    let envelope = DocumentEnvelope::new(graph);
    let json = serde_json::to_string(&envelope).expect("serialize");
    let restored: DocumentEnvelope = serde_json::from_str(&json).expect("deserialize");
    restored.ensure_supported().expect("schema");
    let restored = restored.document;

    assert_eq!(restored.id, envelope.document.id);
    assert_eq!(restored.blocks.len(), 1);
    assert_eq!(restored.assets.len(), 1);
    assert_eq!(restored.relations.len(), 2);
    assert_eq!(restored.sources.len(), 2);

    let block = restored.block(table_id).expect("table block");
    assert!(matches!(block.block, Block::Table { .. }));

    let asset = restored
        .assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .expect("asset");
    assert_eq!(asset.content_identity.as_deref(), Some("sha256:abc"));

    let forward = restored
        .relations
        .iter()
        .find(|relation| relation.id == link_id)
        .expect("forward ref");
    assert_eq!(forward.kind, RelationKind::References);
}

#[test]
fn multi_source_and_backlink_queries() {
    let (graph, table_id, asset_id, _) = sample_graph();

    assert_eq!(graph.sources_for(table_id).len(), 2);
    assert_eq!(
        graph.relations_from(&RelationEndpoint::Node(table_id)).len(),
        1
    );
    assert_eq!(graph.backlinks_to(&RelationEndpoint::Node(table_id)).len(), 1);
    assert_eq!(
        graph.relations_to(&RelationEndpoint::Asset(asset_id)).len(),
        1
    );
}

#[test]
fn unresolved_target_is_stored_not_absent() {
    let mut ids = IdAllocator::default();
    let mut graph = DocumentGraph::new(ids.document_id());
    let paragraph = graph.push_block(Block::Paragraph {
        content: vec![Inline::Text {
            text: "see also".to_string(),
        }],
    });

    graph.push_relation(Relation {
        id: ids.link_id(),
        kind: RelationKind::References,
        source: RelationEndpoint::Node(paragraph),
        target: RelationEndpoint::Unresolved {
            label: "[[missing-note]]".to_string(),
        },
    });

    let unresolved = graph.relations_to(&RelationEndpoint::Unresolved {
        label: "[[missing-note]]".to_string(),
    });
    assert_eq!(unresolved.len(), 1);
    assert!(graph.block(NodeId(999)).is_none());
    assert!(graph.validate().is_valid());
}

#[test]
fn same_semantic_paragraph_can_carry_different_sources() {
    let mut ids = IdAllocator::default();
    let doc_id = ids.document_id();

    let mut from_notedown = DocumentGraph::new(doc_id);
    let paragraph = from_notedown.push_block(Block::Paragraph {
        content: same_paragraph_text(),
    });
    from_notedown.attach_source(SourceRef {
        node: paragraph,
        kind: SourceKind::TextSpan { start: 0, end: 12 },
        precision: SemanticStatus::Resolved,
    });

    let mut from_docx = DocumentGraph::new(doc_id);
    let paragraph = from_docx.push_block(Block::Paragraph {
        content: same_paragraph_text(),
    });
    from_docx.attach_source(SourceRef {
        node: paragraph,
        kind: SourceKind::PackageMember {
            path: "word/document.xml".to_string(),
        },
        precision: SemanticStatus::Resolved,
    });
    from_docx.attach_source(SourceRef {
        node: paragraph,
        kind: SourceKind::Synthetic {
            reason: "merged w:r".to_string(),
        },
        precision: SemanticStatus::Resolved,
    });

    let notedown_block = from_notedown.block(paragraph).expect("paragraph");
    let docx_block = from_docx.block(paragraph).expect("paragraph");
    assert_eq!(notedown_block.block, docx_block.block);
    assert_eq!(from_notedown.sources_for(paragraph).len(), 1);
    assert_eq!(from_docx.sources_for(paragraph).len(), 2);
}

#[test]
fn provenance_is_optional_and_does_not_define_block_semantics() {
    let mut ids = IdAllocator::default();
    let mut without_source = DocumentGraph::new(ids.document_id());
    let unprovenanced = without_source.push_block(Block::Paragraph {
        content: same_paragraph_text(),
    });

    let mut with_uncertain_source = DocumentGraph::new(ids.document_id());
    let sourced = with_uncertain_source.push_block(Block::Paragraph {
        content: same_paragraph_text(),
    });
    with_uncertain_source.attach_source(SourceRef {
        node: sourced,
        kind: SourceKind::Synthetic {
            reason: "source precision is unknown".to_string(),
        },
        precision: SemanticStatus::Unsupported,
    });

    assert!(without_source.sources_for(unprovenanced).is_empty());
    assert_eq!(
        without_source.block(unprovenanced).expect("block").block,
        with_uncertain_source.block(sourced).expect("block").block
    );

    let encoded = serde_json::to_string(&with_uncertain_source).expect("serialize");
    let restored: DocumentGraph = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(restored.sources_for(sourced).len(), 1);
    assert_eq!(
        restored.sources_for(sourced)[0].precision,
        SemanticStatus::Unsupported
    );
}

#[test]
fn metadata_and_asset_path_changes_preserve_link_identity() {
    let mut ids = IdAllocator::default();
    let doc_id = ids.document_id();
    let mut graph = DocumentGraph::new(doc_id);

    let target = graph.push_block(Block::Paragraph {
        content: vec![Inline::Text {
            text: "target".to_string(),
        }],
    });
    let asset_id = ids.asset_id();
    graph.push_asset(Asset {
        id: asset_id,
        kind: AssetKind::Image,
        content_identity: Some("sha256:photo".to_string()),
        source: Some("media/old.png".to_string()),
        media_type: Some("image/png".to_string()),
        status: SemanticStatus::Resolved,
        bytes: None,
    });

    let link_id = ids.link_id();
    graph.push_relation(Relation {
        id: link_id,
        kind: RelationKind::References,
        source: RelationEndpoint::Node(target),
        target: RelationEndpoint::Asset(asset_id),
    });

    graph.set_metadata(DocumentMetadata {
        title: Some("renamed title".to_string()),
        language: None,
        authors: Vec::new(),
        tags: Vec::new(),
    });
    assert!(graph.set_asset_source(asset_id, Some("media/new.png".to_string())));

    let relation = graph
        .relations
        .iter()
        .find(|relation| relation.id == link_id)
        .expect("relation");
    assert_eq!(relation.target, RelationEndpoint::Asset(asset_id));
    assert_eq!(
        graph
            .assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .and_then(|asset| asset.source.as_deref()),
        Some("media/new.png")
    );
    assert_eq!(graph.metadata.title.as_deref(), Some("renamed title"));
    assert_eq!(graph.id, doc_id);
}

#[test]
fn bidirectional_relation_is_distinct_from_backlink_index() {
    let mut ids = IdAllocator::default();
    let mut graph = DocumentGraph::new(ids.document_id());
    let left = graph.push_block(Block::Paragraph {
        content: vec![Inline::Text {
            text: "left".to_string(),
        }],
    });
    let right = graph.push_block(Block::Paragraph {
        content: vec![Inline::Text {
            text: "right".to_string(),
        }],
    });

    graph.push_relation(Relation {
        id: ids.link_id(),
        kind: RelationKind::Bidirectional,
        source: RelationEndpoint::Node(left),
        target: RelationEndpoint::Node(right),
    });
    graph.push_relation(Relation {
        id: ids.link_id(),
        kind: RelationKind::Backlink,
        source: RelationEndpoint::Node(right),
        target: RelationEndpoint::Node(left),
    });

    let explicit = graph
        .relations
        .iter()
        .find(|relation| relation.kind == RelationKind::Bidirectional)
        .expect("bidirectional");
    assert_eq!(explicit.source, RelationEndpoint::Node(left));

    let indexed = graph.backlinks_to(&RelationEndpoint::Node(left));
    assert_eq!(indexed.len(), 1);
    assert_eq!(indexed[0].kind, RelationKind::Backlink);
}

#[test]
fn math_and_inferred_pdf_layout_keep_explicit_semantics() {
    let mut graph = DocumentGraph::new(IdAllocator::default().document_id());

    let math = graph.push_block(Block::Math {
        content: "E = mc^2".to_string(),
        language: Some("latex".to_string()),
    });
    let inferred = graph.push_block(Block::Opaque {
        kind: "pdf.reading-order".to_string(),
        payload_hint: "page-3 stream".to_string(),
        status: SemanticStatus::Inferred,
    });

    let math_block = graph.block(math).expect("math");
    assert!(matches!(math_block.block, Block::Math { .. }));
    assert!(!matches!(math_block.block, Block::Code { .. }));

    let layout = graph.block(inferred).expect("layout");
    assert!(matches!(
        layout.block,
        Block::Opaque {
            status: SemanticStatus::Inferred,
            ..
        }
    ));

    graph.attach_source(SourceRef {
        node: inferred,
        kind: SourceKind::GlyphRegion { page: 3 },
        precision: SemanticStatus::Inferred,
    });
    assert_eq!(
        graph.sources_for(inferred)[0].precision,
        SemanticStatus::Inferred
    );
}
