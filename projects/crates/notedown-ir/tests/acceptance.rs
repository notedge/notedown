//! Living `00` acceptance probes — no parser, AST, or source file required.

use notedown_ir::{
    Asset, AssetId, AssetKind, Block, DocumentGraph, IdAllocator, Inline, LinkId, NodeId,
    Relation, RelationEndpoint, RelationKind, SemanticStatus, SourceKind, SourceRef, TableRow,
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

#[test]
fn round_trip_preserves_identity_relations_and_assets() {
    let (graph, table_id, asset_id, link_id) = sample_graph();
    let json = serde_json::to_string(&graph).expect("serialize");
    let restored: DocumentGraph = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored.id, graph.id);
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

    let unresolved = graph
        .relations_to(&RelationEndpoint::Unresolved {
            label: "[[missing-note]]".to_string(),
        });
    assert_eq!(unresolved.len(), 1);
    assert!(graph.block(NodeId(999)).is_none());
}
