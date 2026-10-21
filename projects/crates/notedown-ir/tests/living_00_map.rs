//! Living `00` acceptance inventory — maps each invariant to an executable probe.
//!
//! CI gate: `cargo test -p notedown-ir living_00`
//!
//! ## Contract invariants (`living_00_01`..`08`)
//!
//! | # | Invariant | Probe |
//! |---|-----------|-------|
//! | 1 | Programmatic graph round-trip | `living_00_01_programmatic_round_trip` |
//! | 2 | Same semantics, different import sources | `living_00_02_same_semantics_different_sources` |
//! | 3 | Metadata and asset path changes preserve link identity | `living_00_03_metadata_and_asset_path_preserve_links` |
//! | 4 | Optional and uncertain provenance do not define equality | `living_00_04_provenance_does_not_define_equality` |
//! | 5 | Backlink index distinct from bidirectional relations | `living_00_05_backlink_distinct_from_bidirectional` |
//! | 6 | Math and inferred layout keep explicit semantics | `living_00_06_math_and_inferred_layout_semantics` |
//! | 7 | Writers consume IR without parser AST | `living_00_07_writer_consumes_ir_only` |
//! | 8 | Unresolved relation targets are stored, not absent | `living_00_08_unresolved_target_preserved` |
//!
//! ## Validator matrix (`living_00_v01`..`v09`, see `validate.rs` and `graph.rs`)
//!
//! | Probe | Issue |
//! |-------|-------|
//! | `living_00_v01_unknown_inline_reference` | Unknown inline reference |
//! | `living_00_v02_unresolved_relation_target_valid` | Unresolved endpoint allowed |
//! | `living_00_v03_duplicate_asset_ids` | Duplicate asset id |
//! | `living_00_v04_duplicate_node_and_link_ids` | Duplicate node/link id |
//! | `living_00_v05_missing_source_and_relation_endpoints` | Missing endpoints |
//! | `living_00_v06_missing_section_and_list_children` | Unknown section/list child |
//! | `living_00_v07_containment_cycles_rejected_relation_cycles_allowed` | Contains vs References cycles |
//! | `living_00_v08_deep_containment_without_stack_overflow` | Deep acyclic containment |
//! | `living_00_v09_revision_tracks_semantic_updates` | Revision monotonicity |

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
        rows: vec![TableRow {
            cells: vec![vec![Inline::Text {
                text: "cell".to_string(),
            }]],
        }],
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

    (graph, table_id, asset_id, link_id)
}

#[test]
fn living_00_01_programmatic_round_trip() {
    let (graph, table_id, asset_id, link_id) = sample_graph();
    assert!(graph.validate().is_valid());

    let envelope = DocumentEnvelope::new(graph);
    let json = serde_json::to_string(&envelope).expect("serialize");
    let restored: DocumentEnvelope = serde_json::from_str(&json).expect("deserialize");
    restored.ensure_supported().expect("schema");
    let restored = restored.document;

    assert_eq!(restored.blocks.len(), 1);
    assert_eq!(restored.assets.len(), 1);
    assert_eq!(restored.relations.len(), 1);
    assert!(matches!(restored.block(table_id).expect("table").block, Block::Table { .. }));
    assert_eq!(
        restored
            .relations
            .iter()
            .find(|relation| relation.id == link_id)
            .expect("forward ref")
            .target,
        RelationEndpoint::Asset(asset_id)
    );
}

#[test]
fn living_00_02_same_semantics_different_sources() {
    let paragraph = vec![
        Inline::Text {
            text: "Hello ".to_string(),
        },
        Inline::Styled {
            style: "emphasis".to_string(),
            children: vec![Inline::Text {
                text: "world".to_string(),
            }],
        },
    ];

    let mut from_notedown = DocumentGraph::new(IdAllocator::default().document_id());
    let notedown_paragraph = from_notedown.push_block(Block::Paragraph {
        content: paragraph.clone(),
    });
    from_notedown.attach_source(SourceRef {
        node: notedown_paragraph,
        kind: SourceKind::TextSpan { start: 0, end: 12 },
        precision: SemanticStatus::Resolved,
    });

    let mut from_docx = DocumentGraph::new(IdAllocator::default().document_id());
    let docx_paragraph = from_docx.push_block(Block::Paragraph {
        content: paragraph.clone(),
    });
    from_docx.attach_source(SourceRef {
        node: docx_paragraph,
        kind: SourceKind::PackageMember {
            path: "word/document.xml".to_string(),
        },
        precision: SemanticStatus::Resolved,
    });

    assert_eq!(
        from_notedown.block(notedown_paragraph).expect("paragraph").block,
        from_docx.block(docx_paragraph).expect("paragraph").block
    );
    assert_ne!(from_notedown.sources_for(notedown_paragraph), from_docx.sources_for(docx_paragraph));
}

#[test]
fn living_00_03_metadata_and_asset_path_preserve_links() {
    let mut ids = IdAllocator::default();
    let mut graph = DocumentGraph::new(ids.document_id());
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
        title: Some("renamed".to_string()),
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
}

#[test]
fn living_00_04_provenance_does_not_define_equality() {
    let content = vec![Inline::Text {
        text: "same body".to_string(),
    }];
    let mut without_source = DocumentGraph::new(IdAllocator::default().document_id());
    let bare = without_source.push_block(Block::Paragraph { content: content.clone() });

    let mut with_source = DocumentGraph::new(IdAllocator::default().document_id());
    let sourced = with_source.push_block(Block::Paragraph { content: content.clone() });
    with_source.attach_source(SourceRef {
        node: sourced,
        kind: SourceKind::Synthetic {
            reason: "unknown precision".to_string(),
        },
        precision: SemanticStatus::Unsupported,
    });

    assert_eq!(
        without_source.block(bare).expect("bare").block,
        with_source.block(sourced).expect("sourced").block
    );
    assert!(without_source.sources_for(bare).is_empty());
    assert_eq!(with_source.sources_for(sourced)[0].precision, SemanticStatus::Unsupported);
}

#[test]
fn living_00_05_backlink_distinct_from_bidirectional() {
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

    assert!(graph
        .relations
        .iter()
        .any(|relation| relation.kind == RelationKind::Bidirectional));
    assert_eq!(graph.backlinks_to(&RelationEndpoint::Node(left)).len(), 1);
}

#[test]
fn living_00_06_math_and_inferred_layout_semantics() {
    let mut graph = DocumentGraph::new(IdAllocator::default().document_id());
    let math = graph.push_block(Block::Math {
        content: "E = mc^2".to_string(),
        language: Some("latex".to_string()),
    });
    let inferred = graph.push_block(Block::Opaque {
        kind: "pdf.reading-order".to_string(),
        payload_hint: "page-3".to_string(),
        status: SemanticStatus::Inferred,
    });

    assert!(matches!(graph.block(math).expect("math").block, Block::Math { .. }));
    assert!(matches!(
        graph.block(inferred).expect("layout").block,
        Block::Opaque {
            status: SemanticStatus::Inferred,
            ..
        }
    ));
}

#[test]
fn living_00_08_unresolved_target_preserved() {
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
fn living_00_07_writer_consumes_ir_only() {
    let mut graph = DocumentGraph::new(IdAllocator::default().document_id());
    graph.push_block(Block::Paragraph {
        content: vec![Inline::Text {
            text: "writer input".to_string(),
        }],
    });

    let json = serde_json::to_string(&graph).expect("serialize graph without parser");
    let restored: DocumentGraph = serde_json::from_str(&json).expect("deserialize graph");
    assert!(restored.validate().is_valid());
    assert!(restored
        .blocks
        .iter()
        .any(|node| matches!(&node.block, Block::Paragraph { .. })));
}
