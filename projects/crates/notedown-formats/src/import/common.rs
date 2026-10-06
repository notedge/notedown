use notedown_ir::{Asset, AssetId, AssetKind, Block, DocumentGraph, Inline, LinkId, Relation, RelationEndpoint, RelationKind, SemanticStatus};

pub(super) fn decode_escaped_text(text: &str) -> String {
    let mut decoded = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\\' {
            if let Some(next) = characters.peek().copied() {
                if next.is_ascii_punctuation() {
                    if let Some(escaped) = characters.next() {
                        decoded.push(escaped);
                        continue;
                    }
                }
            }
        }
        decoded.push(character);
    }
    decoded
}

pub(super) fn register_image_assets(graph: &mut DocumentGraph) {
    let mut references = Vec::new();
    for node in &graph.blocks {
        collect_image_references(node.id, &node.block, &mut references);
    }

    for (index, (node, source)) in references.into_iter().enumerate() {
        let asset_id = AssetId((index + 1) as u64);
        graph.push_asset(Asset {
            id: asset_id,
            kind: AssetKind::Image,
            content_identity: None,
            source: Some(source.clone()),
            media_type: media_type_for_source(&source),
            status: SemanticStatus::Unresolved,
            bytes: None,
        });
        graph.push_relation(Relation {
            id: LinkId((index + 1) as u64),
            kind: RelationKind::Embeds,
            source: RelationEndpoint::Node(node),
            target: RelationEndpoint::Asset(asset_id),
        });
    }
}

fn collect_image_references(
    node: notedown_ir::NodeId,
    block: &Block,
    references: &mut Vec<(notedown_ir::NodeId, String)>,
) {
    match block {
        Block::Section { title, .. } => {
            collect_image_inlines(node, title, references);
        }
        Block::Paragraph { content } | Block::Quote { content } => collect_image_inlines(node, content, references),
        Block::List { items, .. } => {
            for item in items {
                collect_image_inlines(node, &item.content, references);
            }
        }
        Block::Table { rows } => {
            for row in rows {
                for cell in &row.cells {
                    collect_image_inlines(node, cell, references);
                }
            }
        }
        _ => {}
    }
}

fn collect_image_inlines(node: notedown_ir::NodeId, inlines: &[Inline], references: &mut Vec<(notedown_ir::NodeId, String)>) {
    for inline in inlines {
        match inline {
            Inline::Styled { style, children } if style == "image" => {
                if children.len() >= 2 {
                    let source = inline_plain_text(&children[1]);
                    if !source.is_empty() {
                        references.push((node, source));
                    }
                }
            }
            Inline::Styled { children, .. } => collect_image_inlines(node, children, references),
            _ => {}
        }
    }
}

fn inline_plain_text(inline: &Inline) -> String {
    match inline {
        Inline::Text { text } | Inline::InlineCode { text } => text.clone(),
        Inline::Styled { children, .. } => children.iter().map(inline_plain_text).collect(),
        Inline::InlineMath { content, .. } => content.clone(),
        Inline::Reference { display, .. } => display.clone(),
    }
}

fn media_type_for_source(source: &str) -> Option<String> {
    let extension = source.split('?').next().and_then(|value| value.rsplit('.').next())?.to_ascii_lowercase();
    let media_type = match extension.as_str() {
        "avif" => "image/avif",
        "gif" => "image/gif",
        "jpeg" | "jpg" => "image/jpeg",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => return None,
    };
    Some(media_type.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use notedown_ir::{DocumentId, ListItem, TableRow};

    fn image(source: &str) -> Inline {
        Inline::Styled {
            style: "image".into(),
            children: vec![Inline::Text { text: "diagram".into() }, Inline::Text { text: source.into() }],
        }
    }

    #[test]
    fn nested_blocks_register_each_image_occurrence_once() {
        let mut graph = DocumentGraph::new(DocumentId(1));
        let paragraph = graph.push_block(Block::Paragraph { content: vec![image("diagram.png")] });
        let list = graph.push_block(Block::List {
            ordered: false,
            items: vec![ListItem { content: Vec::new(), children: vec![paragraph] }],
        });
        graph.push_block(Block::Section { level: 1, title: Vec::new(), children: vec![list] });

        register_image_assets(&mut graph);

        assert_eq!(graph.assets.len(), 1);
        assert_eq!(graph.relations.len(), 1);
        assert_eq!(graph.relations[0].source, RelationEndpoint::Node(paragraph));
        assert_eq!(graph.relations[0].target, RelationEndpoint::Asset(graph.assets[0].id));
        assert!(graph.validate().is_valid());
    }

    #[test]
    fn table_images_keep_separate_occurrence_identity_and_node_endpoint() {
        let mut graph = DocumentGraph::new(DocumentId(2));
        let table = graph.push_block(Block::Table {
            rows: vec![TableRow { cells: vec![vec![image("same.png")], vec![image("same.png")]] }],
        });

        register_image_assets(&mut graph);

        assert_eq!(graph.assets.len(), 2);
        assert_ne!(graph.assets[0].id, graph.assets[1].id);
        assert!(graph.assets.iter().all(|asset| asset.source.as_deref() == Some("same.png")));
        assert_eq!(graph.relations.len(), 2);
        assert!(graph.relations.iter().all(|relation| {
            relation.kind == RelationKind::Embeds && relation.source == RelationEndpoint::Node(table)
        }));
        assert!(graph.validate().is_valid());
    }
}
