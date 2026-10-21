use notedown_formats::export::markdown_project::export_markdown_project;
use notedown_ir::{Asset, AssetId, AssetKind, Block, DocumentGraph, DocumentId, Inline, NodeId, SemanticStatus, SourceKind, SourceRef};

#[test]
fn export_markdown_project_materializes_assets_and_rewrites_image_paths() {
    let mut graph = DocumentGraph::new(DocumentId(1));
    graph.push_asset(Asset {
        id: AssetId(1),
        kind: AssetKind::Image,
        content_identity: None,
        source: Some("media/logo.png".into()),
        media_type: Some("image/png".into()),
        status: SemanticStatus::Resolved,
        bytes: Some(b"\x89PNG\r\n".to_vec()),
    });
    graph.push_block_with_id(
        NodeId(1),
        Block::Paragraph {
            content: vec![Inline::Image {
                source: "media/logo.png".into(),
                alt: vec![Inline::Text { text: "Logo".into() }],
                title: None,
                asset: Some(AssetId(1)),
            }],
        },
    );

    let project = export_markdown_project(&graph).expect("export project");
    assert!(project.index_markdown.contains("![Logo](assets/logo.png)"));
    assert_eq!(project.assets.len(), 1);
    assert_eq!(project.assets[0].relative_path, "assets/logo.png");
    assert_eq!(project.assets[0].bytes, b"\x89PNG\r\n");
    assert!(project.unresolved_asset_sources.is_empty());
    assert!(project.chapters.is_empty());
}

#[test]
fn export_markdown_project_keeps_unresolved_sources_in_markdown() {
    let mut graph = DocumentGraph::new(DocumentId(1));
    graph.push_asset(Asset {
        id: AssetId(1),
        kind: AssetKind::Image,
        content_identity: None,
        source: Some("media/missing.png".into()),
        media_type: Some("image/png".into()),
        status: SemanticStatus::Unresolved,
        bytes: None,
    });
    graph.push_block_with_id(
        NodeId(1),
        Block::Paragraph {
            content: vec![Inline::Image {
                source: "media/missing.png".into(),
                alt: vec![Inline::Text { text: "Missing".into() }],
                title: None,
                asset: Some(AssetId(1)),
            }],
        },
    );

    let project = export_markdown_project(&graph).expect("export project");
    assert!(project.index_markdown.contains("![Missing](media/missing.png)"));
    assert!(project.assets.is_empty());
    assert_eq!(project.unresolved_asset_sources, vec!["media/missing.png".to_string()]);
}

#[test]
fn export_markdown_project_splits_epub_spine_members_into_chapters() {
    let mut graph = DocumentGraph::new(DocumentId(1));
    graph.metadata.title = Some("Sample Book".into());
    let first = graph.push_block(Block::Section { level: 1, title: vec![Inline::Text { text: "Chapter One".into() }], children: vec![] });
    graph.attach_source(SourceRef {
        node: first,
        kind: SourceKind::PackageMember { path: "OEBPS/chapter-one.xhtml".into() },
        precision: SemanticStatus::Partial,
    });
    let second = graph.push_block(Block::Section { level: 1, title: vec![Inline::Text { text: "Chapter Two".into() }], children: vec![] });
    graph.attach_source(SourceRef {
        node: second,
        kind: SourceKind::PackageMember { path: "OEBPS/chapter-two.xhtml".into() },
        precision: SemanticStatus::Partial,
    });

    let project = export_markdown_project(&graph).expect("export project");
    assert_eq!(project.chapters.len(), 2);
    assert!(project.index_markdown.contains("# Sample Book"));
    assert!(project.index_markdown.contains("chapters/000-chapter-one.md"));
    assert!(project.index_markdown.contains("chapters/001-chapter-two.md"));
    assert!(project.chapters[0].markdown.contains("# Chapter One"));
    assert!(project.chapters[1].markdown.contains("# Chapter Two"));
}
