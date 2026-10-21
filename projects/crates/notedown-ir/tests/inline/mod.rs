use notedown_ir::{
    Asset, AssetId, AssetKind, AttributePair, Attributes, Block, BreakKind, DocumentEnvelope, DocumentGraph, DocumentId, Inline, LinkId,
    RelationEndpoint, SemanticStatus, ValidationIssue,
};

fn paragraph(graph: &mut DocumentGraph, content: Vec<Inline>) -> notedown_ir::NodeId {
    graph.push_block(Block::Paragraph { content })
}

#[test]
fn link_keeps_display_separate_from_destination_and_allows_unresolved_target() {
    let mut graph = DocumentGraph::new(DocumentId(1));
    paragraph(
        &mut graph,
        vec![Inline::Link {
            target: RelationEndpoint::Unresolved { label: "[[missing]]".to_string() },
            display: vec![Inline::Text { text: "missing".to_string() }],
            title: Some("tooltip".to_string()),
        }],
    );

    assert!(graph.validate().is_valid());
    let Block::Paragraph { content } = &graph.blocks[0].block
    else {
        panic!("expected paragraph");
    };
    let Inline::Link { target, display, title } = &content[0]
    else {
        panic!("expected link");
    };
    assert_eq!(display, &vec![Inline::Text { text: "missing".to_string() }]);
    assert_eq!(title.as_deref(), Some("tooltip"));
    assert!(matches!(target, RelationEndpoint::Unresolved { .. }));
}

#[test]
fn link_reports_unknown_semantic_target_instead_of_dropping_it() {
    for target in
        [RelationEndpoint::Node(notedown_ir::NodeId(99)), RelationEndpoint::Asset(AssetId(99)), RelationEndpoint::Document(DocumentId(2))]
    {
        let mut graph = DocumentGraph::new(DocumentId(1));
        paragraph(&mut graph, vec![Inline::Link { target: target.clone(), display: Vec::new(), title: None }]);

        let report = graph.validate();
        assert!(
            report.issues.iter().any(|issue| matches!(issue, ValidationIssue::UnknownInlineLinkTarget { .. })),
            "expected unresolved endpoint for {target:?}"
        );
    }
}

#[test]
fn link_to_existing_asset_and_document_endpoints_is_valid() {
    let mut graph = DocumentGraph::new(DocumentId(1));
    let asset_id = graph.push_asset(Asset {
        id: AssetId(1),
        kind: AssetKind::Attachment,
        content_identity: None,
        source: Some("data.csv".to_string()),
        media_type: None,
        status: SemanticStatus::Resolved,
        bytes: None,
    });
    paragraph(
        &mut graph,
        vec![Inline::Link { target: RelationEndpoint::Asset(asset_id), display: vec![Inline::Text { text: "data".to_string() }], title: None }],
    );

    assert!(graph.validate().is_valid());
}

#[test]
fn image_asset_binding_is_validated_without_faking_a_location() {
    let mut graph = DocumentGraph::new(DocumentId(1));
    let asset_id = graph.push_asset(Asset {
        id: AssetId(1),
        kind: AssetKind::Image,
        content_identity: None,
        source: Some("assets/diagram.png".to_string()),
        media_type: Some("image/png".to_string()),
        status: SemanticStatus::Resolved,
        bytes: None,
    });
    paragraph(
        &mut graph,
        vec![Inline::Image {
            source: "graphics/renamed.png".to_string(),
            alt: vec![Inline::Text { text: "diagram".to_string() }],
            title: None,
            asset: Some(asset_id),
        }],
    );
    assert!(graph.validate().is_valid());

    let mut unbound = DocumentGraph::new(DocumentId(2));
    paragraph(
        &mut unbound,
        vec![Inline::Image { source: "assets/diagram.png".to_string(), alt: Vec::new(), title: None, asset: Some(AssetId(7)) }],
    );
    assert!(unbound.validate().issues.iter().any(|issue| matches!(issue, ValidationIssue::UnknownInlineImageAsset { .. })));
}

#[test]
fn note_and_citation_require_a_non_empty_label() {
    let mut graph = DocumentGraph::new(DocumentId(1));
    paragraph(
        &mut graph,
        vec![Inline::Note { label: "  ".to_string(), display: None }, Inline::Citation { key: String::new(), display: None }],
    );

    let report = graph.validate();
    assert_eq!(report.issues.iter().filter(|issue| matches!(issue, ValidationIssue::EmptyInlineLabel { .. })).count(), 2);
}

#[test]
fn note_and_citation_validate_their_optional_display_content() {
    let mut graph = DocumentGraph::new(DocumentId(1));
    paragraph(
        &mut graph,
        vec![Inline::Note {
            label: "1".to_string(),
            display: Some(vec![Inline::Reference { target: notedown_ir::NodeId(42), display: "missing".to_string() }]),
        }],
    );

    assert!(graph.validate().issues.iter().any(|issue| matches!(issue, ValidationIssue::UnknownInlineReference { .. })));
}

#[test]
fn attributed_children_are_validated_and_carry_stable_order_attributes() {
    let mut graph = DocumentGraph::new(DocumentId(1));
    paragraph(
        &mut graph,
        vec![Inline::Attributed {
            attributes: Attributes {
                id: Some("intro".to_string()),
                classes: vec!["lead".to_string(), "wide".to_string()],
                pairs: vec![AttributePair { key: "data-role".to_string(), value: "summary".to_string() }],
            },
            children: vec![Inline::Link { target: RelationEndpoint::Node(notedown_ir::NodeId(9)), display: Vec::new(), title: None }],
        }],
    );

    let report = graph.validate();
    assert!(report.issues.iter().any(|issue| matches!(issue, ValidationIssue::UnknownInlineLinkTarget { .. })));

    let Block::Paragraph { content } = &graph.blocks[0].block
    else {
        panic!("expected paragraph");
    };
    let Inline::Attributed { attributes, .. } = &content[0]
    else {
        panic!("expected attributed inline");
    };
    assert_eq!(attributes.id.as_deref(), Some("intro"));
    assert_eq!(attributes.classes, vec!["lead".to_string(), "wide".to_string()]);
    assert_eq!(attributes.pairs[0].key, "data-role");
    assert!(!attributes.is_empty());
}

#[test]
fn break_kinds_are_distinct_semantics() {
    let mut graph = DocumentGraph::new(DocumentId(1));
    paragraph(
        &mut graph,
        vec![Inline::Text { text: "a".to_string() }, Inline::Break { kind: BreakKind::Soft }, Inline::Break { kind: BreakKind::Hard }],
    );

    assert!(graph.validate().is_valid());
    let Block::Paragraph { content } = &graph.blocks[0].block
    else {
        panic!("expected paragraph");
    };
    assert_eq!(content[1], Inline::Break { kind: BreakKind::Soft });
    assert_eq!(content[2], Inline::Break { kind: BreakKind::Hard });
}

#[test]
fn inline_semantics_round_trip_through_the_wire_without_losing_identity() {
    let mut graph = DocumentGraph::new(DocumentId(3));
    graph.bump_revision();
    let asset_id = graph.push_asset(Asset {
        id: AssetId(1),
        kind: AssetKind::Image,
        content_identity: None,
        source: Some("assets/diagram.png".to_string()),
        media_type: Some("image/png".to_string()),
        status: SemanticStatus::Resolved,
        bytes: None,
    });
    let link_id = LinkId(1);
    let node = paragraph(
        &mut graph,
        vec![
            Inline::Image {
                source: "assets/diagram.png".to_string(),
                alt: vec![Inline::Text { text: "diagram".to_string() }],
                title: None,
                asset: Some(asset_id),
            },
            Inline::Note { label: "1".to_string(), display: None },
            Inline::Citation { key: "knuth1984".to_string(), display: Some(vec![Inline::Text { text: "Knuth".to_string() }]) },
            Inline::Attributed {
                attributes: Attributes { id: None, classes: vec!["aside".to_string()], pairs: Vec::new() },
                children: vec![Inline::Break { kind: BreakKind::Hard }],
            },
        ],
    );
    graph.push_relation(notedown_ir::Relation {
        id: link_id,
        kind: notedown_ir::RelationKind::Embeds,
        source: RelationEndpoint::Node(node),
        target: RelationEndpoint::Asset(asset_id),
    });

    let envelope = DocumentEnvelope::new(graph.clone());
    let encoded = serde_json::to_string(&envelope).expect("serialize envelope");
    let decoded: DocumentEnvelope = serde_json::from_str(&encoded).expect("deserialize envelope");
    decoded.ensure_supported().expect("supported schema");

    assert_eq!(decoded.document, graph);
    assert!(decoded.document.validate().is_valid());

    let Block::Paragraph { content } = &decoded.document.blocks[0].block
    else {
        panic!("expected paragraph");
    };
    let inline_json = serde_json::to_string(content).expect("serialize inline content");
    assert!(!inline_json.contains("\"title\""), "empty optional link/image titles should not be serialized: {inline_json}");
    assert!(inline_json.contains(r#""Note":{"label":"1"}"#), "an omitted note display should not be serialized: {inline_json}");
    assert!(
        inline_json.contains(r#""attributes":{"classes":["aside"]}"#),
        "empty attribute identity and pairs should be omitted: {inline_json}"
    );
}

#[test]
fn document_without_new_inline_variants_still_loads_on_the_current_wire() {
    let legacy = r#"{
        "schema_version": "notedown-ir/v1",
        "document": {
            "id": 1,
            "revision": 0,
            "metadata": { "title": null, "language": null, "authors": [], "tags": [] },
            "blocks": [
                {
                    "id": 1,
                    "block": {
                        "Paragraph": {
                            "content": [
                                { "Text": { "text": "hello " } },
                                { "Styled": { "style": "bold", "children": [{ "Text": { "text": "world" } }] } }
                            ]
                        }
                    }
                }
            ],
            "relations": [],
            "assets": [],
            "sources": [],
            "coverage": { "complete": true, "loss": [] },
            "next_node": 1
        }
    }"#;

    let envelope: DocumentEnvelope = serde_json::from_str(legacy).expect("load legacy document");
    envelope.ensure_supported().expect("supported schema");
    assert!(envelope.document.validate().is_valid());
    assert_eq!(envelope.document.blocks.len(), 1);
}
