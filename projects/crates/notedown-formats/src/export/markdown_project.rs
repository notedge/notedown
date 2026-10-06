use std::collections::{HashMap, HashSet};

use notedown_ir::{Asset, AssetId, AssetKind, Block, DocumentGraph, DocumentMetadata, NodeId, SourceKind};

use crate::export::markdown::export_markdown_with_image_urls_for_roots;
use crate::FormatError;

/// One chapter file in a multi-document Markdown project export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownProjectChapter {
    /// Project-relative path such as `chapters/001-chapter-one.md`.
    pub relative_path: String,
    /// Markdown body for this chapter.
    pub markdown: String,
}

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
    /// Additional chapter files for multi-spine sources such as EPUB.
    pub chapters: Vec<MarkdownProjectChapter>,
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
    let spine_groups = group_blocks_by_package_member(graph);
    if spine_groups.len() <= 1 {
        let index_markdown = export_markdown_with_image_urls_for_roots(graph, None, Some(&image_urls))?;
        return Ok(MarkdownProject {
            index_markdown,
            chapters: Vec::new(),
            assets,
            unresolved_asset_sources: unresolved,
        });
    }

    let mut chapters = Vec::with_capacity(spine_groups.len());
    for group in spine_groups {
        if is_navigation_only_group(graph, &group) {
            continue;
        }
        let markdown = export_markdown_with_image_urls_for_roots(graph, Some(&group.blocks), Some(&image_urls))?;
        let relative_path = chapter_relative_path(chapters.len(), &group.member_path, &markdown);
        chapters.push(MarkdownProjectChapter { relative_path, markdown });
    }

    if chapters.is_empty() {
        let index_markdown = export_markdown_with_image_urls_for_roots(graph, None, Some(&image_urls))?;
        return Ok(MarkdownProject {
            index_markdown,
            chapters: Vec::new(),
            assets,
            unresolved_asset_sources: unresolved,
        });
    }

    let index_markdown = build_chapter_index(&graph.metadata, &chapters);
    Ok(MarkdownProject {
        index_markdown,
        chapters,
        assets,
        unresolved_asset_sources: unresolved,
    })
}

#[derive(Debug, Clone)]
struct SpineGroup {
    member_path: String,
    blocks: Vec<NodeId>,
}

fn group_blocks_by_package_member(graph: &DocumentGraph) -> Vec<SpineGroup> {
    let mut groups: Vec<SpineGroup> = Vec::new();
    for node in &graph.blocks {
        let Some(path) = package_member_path(graph, node.id) else {
            continue;
        };
        if let Some(last) = groups.last_mut() {
            if last.member_path == path {
                last.blocks.push(node.id);
                continue;
            }
        }
        groups.push(SpineGroup { member_path: path, blocks: vec![node.id] });
    }
    groups
}

fn package_member_path(graph: &DocumentGraph, node: NodeId) -> Option<String> {
    graph.sources_for(node).into_iter().find_map(|source| match &source.kind {
        SourceKind::PackageMember { path } => Some(path.clone()),
        _ => None,
    })
}

fn is_navigation_only_group(graph: &DocumentGraph, group: &SpineGroup) -> bool {
    group.member_path.to_ascii_lowercase().contains("nav")
        && group.blocks.iter().all(|node_id| {
            graph
                .block(*node_id)
                .map(|node| matches!(node.block, Block::List { .. }))
                .unwrap_or(false)
        })
}

fn chapter_relative_path(index: usize, member_path: &str, markdown: &str) -> String {
    let slug = slug_from_member_path(member_path)
        .or_else(|| first_heading_slug(markdown))
        .unwrap_or_else(|| format!("chapter-{}", index + 1));
    format!("chapters/{index:03}-{slug}.md")
}

fn slug_from_member_path(member_path: &str) -> Option<String> {
    let file_name = member_path.rsplit('/').next()?.trim();
    let stem = file_name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(file_name);
    let slug = sanitize_slug(stem);
    if slug.is_empty() { None } else { Some(slug) }
}

fn first_heading_slug(markdown: &str) -> Option<String> {
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix('#') {
            let title = title.trim_start_matches('#').trim();
            let slug = sanitize_slug(title);
            if !slug.is_empty() {
                return Some(slug);
            }
        }
    }
    None
}

fn sanitize_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_hyphen = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_hyphen = false;
        }
        else if !previous_hyphen && !slug.is_empty() {
            slug.push('-');
            previous_hyphen = true;
        }
    }
    slug.trim_end_matches('-').to_string()
}

fn build_chapter_index(metadata: &DocumentMetadata, chapters: &[MarkdownProjectChapter]) -> String {
    let mut output = String::new();
    if let Some(title) = &metadata.title {
        output.push_str("# ");
        output.push_str(title);
        output.push_str("\n\n");
    }
    for chapter in chapters {
        let label = first_heading_slug(&chapter.markdown)
            .map(|slug| slug.replace('-', " "))
            .unwrap_or_else(|| chapter.relative_path.clone());
        output.push_str("- [");
        output.push_str(&label);
        output.push_str("](");
        output.push_str(&chapter.relative_path);
        output.push_str(")\n");
    }
    output.push('\n');
    output
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
    use notedown_ir::{Asset, AssetId, AssetKind, Block, DocumentGraph, DocumentId, Inline, NodeId, SemanticStatus, SourceKind, SourceRef};

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

    #[test]
    fn export_markdown_project_splits_epub_spine_members_into_chapters() {
        let mut graph = DocumentGraph::new(DocumentId(1));
        graph.metadata.title = Some("Sample Book".into());
        let first = graph.push_block(Block::Section {
            level: 1,
            title: vec![Inline::Text { text: "Chapter One".into() }],
            children: vec![],
        });
        graph.attach_source(SourceRef {
            node: first,
            kind: SourceKind::PackageMember { path: "OEBPS/chapter-one.xhtml".into() },
            precision: SemanticStatus::Partial,
        });
        let second = graph.push_block(Block::Section {
            level: 1,
            title: vec![Inline::Text { text: "Chapter Two".into() }],
            children: vec![],
        });
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
}
