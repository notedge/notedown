use notedown_ir::{
    Asset, AssetKind, Block, DocumentGraph, IdAllocator, Inline, Relation, RelationEndpoint, RelationKind, SemanticStatus, SourceKind,
    SourceRef, ValidationIssue,
};

#[test]
fn unknown_inline_reference() {
    let mut ids = IdAllocator::default();
    let mut graph = DocumentGraph::new(ids.document_id());
    let _allocated = ids.node_id();
    let missing = ids.node_id();
    let _paragraph =
        graph.push_block(Block::Paragraph { content: vec![Inline::Reference { target: missing, display: "missing".to_string() }] });

    let report = graph.validate();
    assert!(!report.is_valid());
    assert!(report.issues.iter().any(|issue| matches!(issue, ValidationIssue::UnknownInlineReference { .. })));
}

#[test]
fn unresolved_relation_target_is_valid() {
    let mut ids = IdAllocator::default();
    let mut graph = DocumentGraph::new(ids.document_id());
    let paragraph = graph.push_block(Block::Paragraph { content: vec![Inline::Text { text: "x".to_string() }] });

    graph.push_relation(Relation {
        id: ids.link_id(),
        kind: RelationKind::References,
        source: RelationEndpoint::Node(paragraph),
        target: RelationEndpoint::Unresolved { label: "[[note]]".to_string() },
    });

    assert!(graph.validate().is_valid());
}

#[test]
fn duplicate_asset_ids() {
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
            bytes: None,
        });
    }

    let report = graph.validate();
    assert!(report.issues.iter().any(|issue| matches!(issue, ValidationIssue::DuplicateAssetId { .. })));
}

#[test]
fn duplicate_node_and_link_ids() {
    let mut graph = DocumentGraph::new(notedown_ir::DocumentId(1));
    graph.push_block_with_id(notedown_ir::NodeId(7), Block::Paragraph { content: Vec::new() });
    graph.push_block_with_id(notedown_ir::NodeId(7), Block::Paragraph { content: Vec::new() });
    let relation = Relation {
        id: notedown_ir::LinkId(3),
        kind: RelationKind::References,
        source: RelationEndpoint::Node(notedown_ir::NodeId(7)),
        target: RelationEndpoint::ExternalUri("https://example.test/".to_string()),
    };
    graph.push_relation(relation.clone());
    graph.push_relation(relation);

    let report = graph.validate();
    assert!(report.issues.iter().any(|issue| matches!(
        issue,
        ValidationIssue::DuplicateNodeId { id } if *id == notedown_ir::NodeId(7)
    )));
    assert!(report.issues.iter().any(|issue| matches!(
        issue,
        ValidationIssue::DuplicateLinkId { id } if *id == notedown_ir::LinkId(3)
    )));
}

#[test]
fn missing_source_and_relation_endpoints() {
    let mut graph = DocumentGraph::new(notedown_ir::DocumentId(1));
    graph.attach_source(SourceRef { node: notedown_ir::NodeId(10), kind: SourceKind::Manual, precision: SemanticStatus::Unresolved });
    for (id, source, target) in [
        (1, RelationEndpoint::Node(notedown_ir::NodeId(11)), RelationEndpoint::ExternalUri("https://example.test/".to_string())),
        (2, RelationEndpoint::Asset(notedown_ir::AssetId(12)), RelationEndpoint::Node(notedown_ir::NodeId(13))),
        (3, RelationEndpoint::Document(notedown_ir::DocumentId(2)), RelationEndpoint::Unresolved { label: "unknown".to_string() }),
    ] {
        graph.push_relation(Relation { id: notedown_ir::LinkId(id), kind: RelationKind::References, source, target });
    }

    let report = graph.validate();
    assert!(report.issues.iter().any(|issue| matches!(
        issue,
        ValidationIssue::UnknownNode { id } if *id == notedown_ir::NodeId(10)
    )));
    assert_eq!(report.issues.iter().filter(|issue| matches!(issue, ValidationIssue::UnknownRelationEndpoint { .. })).count(), 3);
}

#[test]
fn missing_section_and_list_children() {
    let mut graph = DocumentGraph::new(notedown_ir::DocumentId(1));
    graph.push_block(Block::Section { level: 1, title: Vec::new(), children: vec![notedown_ir::NodeId(20)] });
    graph.push_block(Block::List {
        ordered: false,
        items: vec![notedown_ir::ListItem { content: Vec::new(), children: vec![notedown_ir::NodeId(21)], number: None }],
    });

    let report = graph.validate();
    assert!(report.issues.iter().any(|issue| matches!(
        issue,
        ValidationIssue::UnknownSectionChild { section, child }
            if *section == notedown_ir::NodeId(1) && *child == notedown_ir::NodeId(20)
    )));
    assert!(report.issues.iter().any(|issue| matches!(
        issue,
        ValidationIssue::UnknownListChild { list, child }
            if *list == notedown_ir::NodeId(2) && *child == notedown_ir::NodeId(21)
    )));
}

#[test]
fn containment_cycles_rejected_relation_cycles_allowed() {
    let mut cyclic = DocumentGraph::new(notedown_ir::DocumentId(1));
    cyclic.push_block_with_id(notedown_ir::NodeId(1), Block::Section { level: 1, title: Vec::new(), children: vec![notedown_ir::NodeId(2)] });
    cyclic.push_block_with_id(
        notedown_ir::NodeId(2),
        Block::List { ordered: false, items: vec![notedown_ir::ListItem { content: Vec::new(), children: vec![notedown_ir::NodeId(1)], number: None }] },
    );
    assert!(cyclic.validate().issues.iter().any(|issue| matches!(issue, ValidationIssue::ContainmentCycle { .. })));

    let mut relations = DocumentGraph::new(notedown_ir::DocumentId(2));
    let first = relations.push_block(Block::Paragraph { content: Vec::new() });
    let second = relations.push_block(Block::Paragraph { content: Vec::new() });
    relations.push_relation(Relation {
        id: notedown_ir::LinkId(1),
        kind: RelationKind::References,
        source: RelationEndpoint::Node(first),
        target: RelationEndpoint::Node(second),
    });
    relations.push_relation(Relation {
        id: notedown_ir::LinkId(2),
        kind: RelationKind::References,
        source: RelationEndpoint::Node(second),
        target: RelationEndpoint::Node(first),
    });
    assert!(relations.validate().is_valid());

    relations.relations[0].kind = RelationKind::Contains;
    relations.relations[1].kind = RelationKind::Contains;
    assert!(relations.validate().issues.iter().any(|issue| matches!(issue, ValidationIssue::ContainmentCycle { .. })));
}

#[test]
fn deep_containment_without_stack_overflow() {
    const NODE_COUNT: u64 = 20_000;
    let mut graph = DocumentGraph::new(notedown_ir::DocumentId(1));
    for id in 1..=NODE_COUNT {
        let children = if id == NODE_COUNT { Vec::new() } else { vec![notedown_ir::NodeId(id + 1)] };
        graph.push_block_with_id(notedown_ir::NodeId(id), Block::Section { level: 1, title: Vec::new(), children });
    }
    assert!(graph.validate().is_valid());
}

#[test]
fn ordered_list_requires_increasing_nonzero_numbers() {
    let mut graph = DocumentGraph::new(notedown_ir::DocumentId(1));
    graph.push_block(Block::List {
        ordered: true,
        items: vec![
            notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(1) },
            notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(2) },
            notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(3) },
        ],
    });
    assert!(graph.validate().is_valid());
}

#[test]
fn ordered_list_allows_non_continuous_increasing_numbers() {
    let mut graph = DocumentGraph::new(notedown_ir::DocumentId(1));
    graph.push_block(Block::List {
        ordered: true,
        items: vec![
            notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(3) },
            notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(7) },
            notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(9) },
        ],
    });
    assert!(graph.validate().is_valid());
}

#[test]
fn ordered_list_missing_number_is_rejected() {
    let mut graph = DocumentGraph::new(notedown_ir::DocumentId(1));
    graph.push_block(Block::List {
        ordered: true,
        items: vec![
            notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(1) },
            notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: None },
        ],
    });
    let report = graph.validate();
    assert!(report.issues.iter().any(|issue| matches!(issue, ValidationIssue::MissingListItemNumber { item_index: 1, .. })));
}

#[test]
fn ordered_list_zero_or_non_increasing_numbers_are_rejected() {
    // Zero start
    {
        let mut graph = DocumentGraph::new(notedown_ir::DocumentId(1));
        graph.push_block(Block::List {
            ordered: true,
            items: vec![notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(0) }],
        });
        let report = graph.validate();
        assert!(report
            .issues
            .iter()
            .any(|issue| matches!(issue, ValidationIssue::OutOfOrderListItemNumber { item_index: 0, found: Some(0), .. })));
    }

    // Duplicate
    {
        let mut graph = DocumentGraph::new(notedown_ir::DocumentId(2));
        graph.push_block(Block::List {
            ordered: true,
            items: vec![
                notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(1) },
                notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(1) },
                notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(2) },
            ],
        });
        let report = graph.validate();
        assert!(report.issues.iter().any(|issue| matches!(
            issue,
            ValidationIssue::OutOfOrderListItemNumber { item_index: 1, found: Some(1), previous: Some(1), .. }
        )));
    }

    // Reverse order
    {
        let mut graph = DocumentGraph::new(notedown_ir::DocumentId(3));
        graph.push_block(Block::List {
            ordered: true,
            items: vec![
                notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(5) },
                notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(2) },
            ],
        });
        let report = graph.validate();
        assert!(report.issues.iter().any(|issue| matches!(
            issue,
            ValidationIssue::OutOfOrderListItemNumber { item_index: 1, found: Some(2), previous: Some(5), .. }
        )));
    }
}

#[test]
fn unordered_list_must_not_carry_number() {
    let mut graph = DocumentGraph::new(notedown_ir::DocumentId(1));
    graph.push_block(Block::List {
        ordered: false,
        items: vec![
            notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: None },
            notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(7) },
        ],
    });
    let report = graph.validate();
    assert!(report.issues.iter().any(|issue| matches!(
        issue,
        ValidationIssue::UnexpectedListItemNumber { item_index: 1, found: 7, .. }
    )));
}

#[test]
fn nested_ordered_lists_have_independent_numbering() {
    let mut graph = DocumentGraph::new(notedown_ir::DocumentId(1));
    let inner = graph.push_block(Block::List {
        ordered: true,
        items: vec![
            notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(1) },
            notedown_ir::ListItem { content: Vec::new(), children: Vec::new(), number: Some(2) },
        ],
    });
    graph.push_block(Block::List {
        ordered: true,
        items: vec![notedown_ir::ListItem {
            content: Vec::new(),
            children: vec![inner],
            number: Some(1),
        }],
    });
    assert!(graph.validate().is_valid());
}
