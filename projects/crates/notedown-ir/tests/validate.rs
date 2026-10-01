use notedown_ir::{
    Asset, AssetKind, Block, DocumentGraph, IdAllocator, Inline, Relation, RelationEndpoint,
    RelationKind, SemanticStatus, ValidationIssue,
};

#[test]
fn validation_catches_unknown_inline_reference() {
    let mut ids = IdAllocator::default();
    let mut graph = DocumentGraph::new(ids.document_id());
    let _allocated = ids.node_id();
    let missing = ids.node_id();
    let _paragraph = graph.push_block(Block::Paragraph {
        content: vec![Inline::Reference {
            target: missing,
            display: "missing".to_string(),
        }],
    });

    let report = graph.validate();
    assert!(!report.is_valid());
    assert!(report
        .issues
        .iter()
        .any(|issue| matches!(issue, ValidationIssue::UnknownInlineReference { .. })));
}

#[test]
fn validation_accepts_unresolved_relation_target() {
    let mut ids = IdAllocator::default();
    let mut graph = DocumentGraph::new(ids.document_id());
    let paragraph = graph.push_block(Block::Paragraph {
        content: vec![Inline::Text {
            text: "x".to_string(),
        }],
    });

    graph.push_relation(Relation {
        id: ids.link_id(),
        kind: RelationKind::References,
        source: RelationEndpoint::Node(paragraph),
        target: RelationEndpoint::Unresolved {
            label: "[[note]]".to_string(),
        },
    });

    assert!(graph.validate().is_valid());
}

#[test]
fn validation_catches_duplicate_asset_ids() {
    let mut ids = IdAllocator::default();
    let mut graph = DocumentGraph::new(ids.document_id());
    let asset_id = ids.asset_id();

    for _ in 0..2 {
        graph.push_asset(Asset {
            id: asset_id,
            kind: AssetKind::Attachment,
            content_identity: None,
            source: None,
            media_type: None,
            status: SemanticStatus::Resolved,
        });
    }

    let report = graph.validate();
    assert!(report
        .issues
        .iter()
        .any(|issue| matches!(issue, ValidationIssue::DuplicateAssetId { .. })));
}
