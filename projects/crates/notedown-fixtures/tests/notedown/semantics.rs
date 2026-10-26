//! Source-text to IR semantic probes.
//!
//! Each probe asserts the lowered semantic objects, relations, and assets
//! instead of matching writer output substrings, so a reader regression cannot
//! hide behind an unchanged round trip.

use notedown_formats::import::notedown::import_notedown_bytes;
use notedown_ir::{Block, DocumentGraph, Inline, ListItem, RelationEndpoint, RelationKind, SemanticStatus};

fn import(source: &str) -> DocumentGraph {
    let graph = import_notedown_bytes("probe.nd", source).expect("lower notedown source");
    assert!(graph.validate().is_valid(), "probe graph must validate: {:?}", graph.validate().issues);
    graph
}

fn paragraph_inlines(block: &Block) -> &[Inline] {
    match block {
        Block::Paragraph { content } => content,
        other => panic!("expected paragraph, got {other:?}"),
    }
}

#[test]
fn heading_projects_a_section_without_its_marker() {
    let graph = import("# Title\n");

    let node = graph.blocks.first().expect("section block");
    let Block::Section { level, title, children } = &node.block
    else {
        panic!("expected section, got {:?}", node.block);
    };
    assert_eq!(*level, 1);
    assert!(children.is_empty());
    assert_eq!(title.iter().map(Inline::plain_text).collect::<String>(), "Title");
}

#[test]
fn paragraph_projects_inline_text() {
    let graph = import("Body text.\n");

    assert_eq!(paragraph_inlines(&graph.blocks[0].block), [Inline::Text { text: "Body text.".into() }]);
}

#[test]
fn inline_code_projects_a_code_inline_without_its_delimiters() {
    let graph = import("Use `cargo test` now.\n");

    assert_eq!(
        paragraph_inlines(&graph.blocks[0].block),
        [Inline::Text { text: "Use ".into() }, Inline::InlineCode { text: "cargo test".into() }, Inline::Text { text: " now.".into() },]
    );
}

#[test]
fn strikethrough_projects_a_strike_style_without_its_delimiters() {
    let graph = import("This is ~~obsolete~~ text.\n");

    let content = paragraph_inlines(&graph.blocks[0].block);
    assert!(
        matches!(&content[1], Inline::Styled { style, children } if style == "strike" && children == &[Inline::Text { text: "obsolete".into() }]),
        "{content:?}"
    );
}

#[test]
fn unordered_list_projects_items_in_authored_order() {
    let graph = import("- Alpha\n- Beta\n");

    let Block::List { ordered, items } = &graph.blocks[0].block
    else {
        panic!("expected list, got {:?}", graph.blocks[0].block);
    };
    assert!(!ordered);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].content.iter().map(Inline::plain_text).collect::<String>(), "Alpha");
    assert_eq!(items[1].content.iter().map(Inline::plain_text).collect::<String>(), "Beta");
}

#[test]
fn ordered_list_projects_an_ordered_list_block() {
    let graph = import("1. Alpha\n2. Beta\n");

    let Block::List { ordered, items } = &graph.blocks[0].block
    else {
        panic!("expected list, got {:?}", graph.blocks[0].block);
    };
    assert!(ordered);
    assert_eq!(
        items,
        &vec![
            ListItem { content: vec![Inline::Text { text: "Alpha".into() }], children: Vec::new(), number: Some(1), ..Default::default() },
            ListItem { content: vec![Inline::Text { text: "Beta".into() }], children: Vec::new(), number: Some(2), ..Default::default() },
        ]
    );
}

#[test]
fn table_projects_header_and_body_rows_without_the_separator() {
    let graph = import("| H1 | H2 |\n|----|----|\n| C1 | C2 |\n");

    let Block::Table { rows, .. } = &graph.blocks[0].block
    else {
        panic!("expected table, got {:?}", graph.blocks[0].block);
    };
    assert_eq!(rows.len(), 2, "separator row must not become body content: {rows:?}");
    assert!(rows[0].is_header);
    assert!(!rows[1].is_header);
    assert_eq!(rows[0].cells.iter().map(|cell| cell.content[0].plain_text()).collect::<Vec<_>>(), ["H1", "H2"]);
    assert_eq!(rows[1].cells.iter().map(|cell| cell.content[0].plain_text()).collect::<Vec<_>>(), ["C1", "C2"]);
}

#[test]
fn link_projects_destination_and_display_separately() {
    let graph = import("Visit [home](https://example.com) today.\n");

    let content = paragraph_inlines(&graph.blocks[0].block);
    let link = content
        .iter()
        .find_map(|inline| match inline {
            Inline::Link { target, display, .. } => Some((target, display)),
            _ => None,
        })
        .expect("first-class link inline");
    assert_eq!(link.0, &RelationEndpoint::ExternalUri("https://example.com".into()));
    assert_eq!(link.1.iter().map(Inline::plain_text).collect::<String>(), "home");
}

#[test]
fn image_projects_a_bound_asset_and_embed_relation() {
    let graph = import("![diagram](assets/diagram.png)\n");

    let content = paragraph_inlines(&graph.blocks[0].block);
    let (source, asset) = content
        .iter()
        .find_map(|inline| match inline {
            Inline::Image { source, asset, .. } => Some((source, asset)),
            _ => None,
        })
        .expect("first-class image inline");
    assert_eq!(source, "assets/diagram.png");
    let asset = asset.expect("image occurrence binds an asset identity");
    let registered = graph.assets.iter().find(|candidate| candidate.id == asset).expect("registered asset");
    assert_eq!(registered.source.as_deref(), Some("assets/diagram.png"));
    assert_eq!(registered.media_type.as_deref(), Some("image/png"));
    assert_eq!(registered.status, SemanticStatus::Unresolved);
    assert_eq!(graph.relations.len(), 1);
    assert_eq!(graph.relations[0].kind, RelationKind::Embeds);
    assert_eq!(graph.relations[0].source, RelationEndpoint::Node(graph.blocks[0].id));
    assert_eq!(graph.relations[0].target, RelationEndpoint::Asset(asset));
}

#[test]
fn fenced_code_projects_language_and_content() {
    let graph = import("```rust\nfn main() {}\n```\n");

    let Block::Code { language, content } = &graph.blocks[0].block
    else {
        panic!("expected code block, got {:?}", graph.blocks[0].block);
    };
    assert_eq!(language.as_deref(), Some("rust"));
    assert_eq!(content, "fn main() {}");
}

#[test]
fn strong_and_emphasis_project_distinct_styled_inlines() {
    let graph = import("**bold** and *italic*\n");

    let content = paragraph_inlines(&graph.blocks[0].block);
    let styles = content
        .iter()
        .filter_map(|inline| match inline {
            Inline::Styled { style, children } => Some((style.as_str(), children.iter().map(Inline::plain_text).collect::<String>())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(styles, [("bold", "bold".to_string()), ("italic", "italic".to_string())]);
}

#[test]
fn blockquote_projects_a_quote_block() {
    let graph = import("> Quoted line\n");

    let Block::Quote { content } = &graph.blocks[0].block
    else {
        panic!("expected quote, got {:?}", graph.blocks[0].block);
    };
    assert!(content.iter().map(Inline::plain_text).collect::<String>().contains("Quoted line"));
}

#[test]
fn thematic_break_projects_a_partial_opaque_block() {
    let graph = import("---\n");

    let Block::Opaque { kind, status, .. } = &graph.blocks[0].block
    else {
        panic!("expected opaque block, got {:?}", graph.blocks[0].block);
    };
    assert_eq!(kind, "thematic_break");
    assert_eq!(*status, SemanticStatus::Partial);
}
