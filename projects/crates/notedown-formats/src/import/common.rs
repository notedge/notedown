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

/// Register one asset per authored image occurrence and bind the inline to it.
///
/// The binding is written back into `Inline::Image::asset` so the occurrence
/// keeps a stable logical identity even when the storage location is renamed
/// later.
pub fn register_image_assets(graph: &mut DocumentGraph) {
    let mut references = Vec::new();
    for node in &mut graph.blocks {
        let node_id = node.id;
        bind_block_image_assets(node_id, &mut node.block, &mut references);
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

fn bind_block_image_assets(node: notedown_ir::NodeId, block: &mut Block, references: &mut Vec<(notedown_ir::NodeId, String)>) {
    match block {
        Block::Section { title, .. } => {
            bind_inline_image_assets(node, title, references);
        }
        Block::Paragraph { content } | Block::Quote { content } => bind_inline_image_assets(node, content, references),
        Block::List { items, .. } => {
            for item in items {
                bind_inline_image_assets(node, &mut item.content, references);
            }
        }
        Block::Table { rows, .. } => {
            for row in rows {
                for cell in &mut row.cells {
                    bind_inline_image_assets(node, &mut cell.content, references);
                }
            }
        }
        _ => {}
    }
}

fn bind_inline_image_assets(node: notedown_ir::NodeId, inlines: &mut [Inline], references: &mut Vec<(notedown_ir::NodeId, String)>) {
    for inline in inlines {
        match inline {
            Inline::Image { source, asset, .. } => {
                if source.is_empty() {
                    continue;
                }
                let asset_id = AssetId((references.len() + 1) as u64);
                *asset = Some(asset_id);
                references.push((node, source.clone()));
            }
            Inline::Styled { children, .. } | Inline::Attributed { children, .. } => bind_inline_image_assets(node, children, references),
            _ => {}
        }
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
