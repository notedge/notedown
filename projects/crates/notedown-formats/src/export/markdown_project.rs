use std::collections::{HashMap, HashSet};

use notedown_ir::{Asset, AssetId, AssetKind, DocumentGraph, SemanticStatus};

use crate::export::markdown::export_markdown_with_image_urls;
use crate::FormatError;

/// One materialized asset entry for a Markdown project export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownProjectAsset {
    /// Project-relative path such as `assets/logo.png`.
    pub relative_path: String,
    /// Raw asset bytes when materialized from IR.
    pub bytes: Vec<u8>,
    /// Source IR asset identity.
    pub asset_id: AssetId,
}

/// Markdown project export payload: index text plus on-disk assets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownProject {
    /// Primary Markdown document body (`index.md` content).
    pub index_markdown: String,
    /// Materialized assets referenced from the Markdown body.
    pub assets: Vec<MarkdownProjectAsset>,
    /// Image sources that could not be materialized into `assets/`.
    pub unresolved_asset_sources: Vec<String>,
}

/// Export a Markdown project from `notedown-ir`.
///
/// Materialized image assets are written under `assets/` with stable file names.
/// Image destinations in the Markdown body are rewritten to project-relative paths.
/// Assets without bytes keep their original destination and are listed as unresolved.
pub fn export_markdown_project(graph: &DocumentGraph) -> Result<MarkdownProject, FormatError> {
    let (image_urls, assets, unresolved) = plan_project_assets(graph)?;
    let index_markdown = export_markdown_with_image_urls(graph, Some(&image_urls))?;
    Ok(MarkdownProject { index_markdown, assets, unresolved_asset_sources: unresolved })
}

fn plan_project_assets(graph: &DocumentGraph) -> Result<(HashMap<String, String>, Vec<MarkdownProjectAsset>, Vec<String>), FormatError> {
    let mut image_urls = HashMap::new();
    let mut assets = Vec::new();
    let mut unresolved = Vec::new();
    let mut used_names = HashSet::new();

    for asset in &graph.assets {
        if asset.kind != AssetKind::Image {
            continue;
        }
        let Some(source) = asset.source.as_ref() else {
            continue;
        };
        let Some(bytes) = asset.bytes.as_ref() else {
            unresolved.push(source.clone());
            continue;
        };
        let file_name = stable_asset_file_name(asset, &used_names);
        used_names.insert(file_name.clone());
        let relative_path = format!("assets/{file_name}");
        image_urls.insert(source.clone(), relative_path.clone());
        assets.push(MarkdownProjectAsset { relative_path, bytes: bytes.clone(), asset_id: asset.id });
    }

    Ok((image_urls, assets, unresolved))
}

fn stable_asset_file_name(asset: &Asset, used: &HashSet<String>) -> String {
    let fallback = format!("asset-{}", asset.id.0);
    let base = asset
        .source
        .as_deref()
        .and_then(|source| source.rsplit('/').next())
        .filter(|name| !name.is_empty())
        .map(sanitize_file_name)
        .unwrap_or_else(|| sanitize_file_name(&fallback));

    let mut candidate = base.clone();
    let mut suffix = asset.id.0;
    while used.contains(&candidate) {
        candidate = insert_name_suffix(&base, suffix);
        suffix += 1;
    }
    candidate
}

fn sanitize_file_name(name: &str) -> String {
    let mut sanitized = name
        .chars()
        .map(|character| if matches!(character, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { character })
        .collect::<String>();
    if sanitized.is_empty() {
        sanitized = "asset".into();
    }
    if sanitized == "." || sanitized == ".." {
        sanitized = format!("asset-{sanitized}");
    }
    sanitized
}

fn insert_name_suffix(base: &str, suffix: u64) -> String {
    if let Some((stem, extension)) = base.rsplit_once('.') {
        if !extension.is_empty() && extension.len() <= 8 {
            return format!("{stem}-{suffix}.{extension}");
        }
    }
    format!("{base}-{suffix}")
}

#[cfg(test)]
mod tests {
    use notedown_ir::{Asset, AssetId, AssetKind, Block, DocumentGraph, DocumentId, Inline, NodeId, SemanticStatus};

    use super::*;

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
                content: vec![Inline::Styled {
                    style: "image".into(),
                    children: vec![Inline::Text { text: "Logo".into() }, Inline::Text { text: "media/logo.png".into() }],
                }],
            },
        );

        let project = export_markdown_project(&graph).expect("export project");
        assert!(project.index_markdown.contains("![Logo](assets/logo.png)"));
        assert_eq!(project.assets.len(), 1);
        assert_eq!(project.assets[0].relative_path, "assets/logo.png");
        assert_eq!(project.assets[0].bytes, b"\x89PNG\r\n");
        assert!(project.unresolved_asset_sources.is_empty());
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
                content: vec![Inline::Styled {
                    style: "image".into(),
                    children: vec![
                        Inline::Text { text: "Missing".into() },
                        Inline::Text { text: "media/missing.png".into() },
                    ],
                }],
            },
        );

        let project = export_markdown_project(&graph).expect("export project");
        assert!(project.index_markdown.contains("![Missing](media/missing.png)"));
        assert!(project.assets.is_empty());
        assert_eq!(project.unresolved_asset_sources, vec!["media/missing.png".to_string()]);
    }
}
