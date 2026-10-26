//! Chapter and asset planning for EPUB export.

use std::collections::{HashMap, HashSet};

use notedown_ir::{Asset, AssetId, AssetKind, Block, DocumentGraph, NodeId, SourceKind};

/// One chapter in the EPUB spine — either a source package member group or a
/// synthetic fallback built from top-level blocks.
#[derive(Debug, Clone)]
pub struct ChapterPlan {
    /// Stable display label, used by the nav TOC.
    pub label: String,
    /// Block node ids belonging to this chapter.
    pub blocks: Vec<NodeId>,
    /// True when this chapter was synthesized (no package member provenance).
    #[allow(dead_code)]
    pub synthetic: bool,
}

/// One materialized asset entry for the EPUB package.
#[derive(Debug, Clone)]
pub struct PlannedAsset {
    /// Package-relative path such as `assets/cover.png` (OEBPS is the root).
    pub relative_path: String,
    /// Raw asset bytes.
    pub bytes: Vec<u8>,
    /// Media type for the manifest item.
    pub media_type: String,
    /// Source IR asset identity.
    pub asset_id: AssetId,
}

/// Plan the spine chapters for a document graph.
///
/// Strategy:
///
/// 1. If any block carries `SourceKind::PackageMember`, group consecutive top-level
///    blocks by their member path and skip navigation-only groups.
/// 2. Otherwise split at each top-level `Section` heading, producing one chapter per
///    `h1`-rooted subtree plus a lead chapter for any prelude blocks before the first
///    heading.
/// 3. If neither strategy yields chapters (single flat document), wrap everything in
///    a single synthetic chapter.
pub fn plan_chapters(graph: &DocumentGraph) -> Vec<ChapterPlan> {
    if let Some(groups) = plan_by_package_member(graph) {
        return groups;
    }
    if let Some(groups) = plan_by_top_level_sections(graph) {
        return groups;
    }
    plan_single_synthetic(graph)
}

fn plan_by_package_member(graph: &DocumentGraph) -> Option<Vec<ChapterPlan>> {
    let top_level = top_level_block_ids(graph);
    let mut groups: Vec<SpineGroup> = Vec::new();
    for node_id in top_level {
        let Some(path) = package_member_path(graph, node_id) else {
            // Mixed provenance — bail out, caller will try the next strategy.
            return None;
        };
        if let Some(last) = groups.last_mut() {
            if last.member_path == path {
                last.blocks.push(node_id);
                continue;
            }
        }
        groups.push(SpineGroup {
            member_path: path,
            blocks: vec![node_id],
        });
    }
    if groups.is_empty() {
        return None;
    }
    let mut plans = Vec::new();
    for group in groups {
        if is_navigation_only_group(graph, &group) {
            continue;
        }
        let label = chapter_label_from_member(&group.member_path, graph, group.blocks.first().copied());
        plans.push(ChapterPlan {
            label,
            blocks: group.blocks,
            synthetic: false,
        });
    }
    if plans.is_empty() {
        None
    } else {
        Some(plans)
    }
}

fn plan_by_top_level_sections(graph: &DocumentGraph) -> Option<Vec<ChapterPlan>> {
    let top_level = top_level_block_ids(graph);
    if top_level.is_empty() {
        return None;
    }
    let mut prelude = Vec::new();
    let mut sections: Vec<(NodeId, Vec<NodeId>)> = Vec::new();
    for node_id in top_level {
        let Some(node) = graph.block(node_id) else {
            continue;
        };
        match &node.block {
            Block::Section { level, title, children } if *level == 1 => {
                let mut chapter_blocks = vec![node_id];
                chapter_blocks.extend(children.iter().copied());
                sections.push((node_id, chapter_blocks));
            }
            _ => prelude.push(node_id),
        }
    }
    let mut plans = Vec::new();
    if !prelude.is_empty() {
        let label = graph
            .metadata
            .title
            .clone()
            .unwrap_or_else(|| "Front Matter".into());
        plans.push(ChapterPlan {
            label,
            blocks: prelude,
            synthetic: true,
        });
    }
    for (node_id, blocks) in sections {
        let label = section_title_plain_text(graph, node_id).unwrap_or_else(|| "Chapter".into());
        plans.push(ChapterPlan {
            label,
            blocks,
            synthetic: true,
        });
    }
    if plans.is_empty() { None } else { Some(plans) }
}

fn plan_single_synthetic(graph: &DocumentGraph) -> Vec<ChapterPlan> {
    let blocks = top_level_block_ids(graph);
    let label = graph.metadata.title.clone().unwrap_or_else(|| "Document".into());
    vec![ChapterPlan {
        label,
        blocks,
        synthetic: true,
    }]
}

fn top_level_block_ids(graph: &DocumentGraph) -> Vec<NodeId> {
    let nested = nested_block_ids(graph);
    graph
        .blocks
        .iter()
        .filter(|node| !nested.contains(&node.id))
        .map(|node| node.id)
        .collect()
}

fn nested_block_ids(graph: &DocumentGraph) -> HashSet<NodeId> {
    let mut ids = HashSet::new();
    for node in &graph.blocks {
        match &node.block {
            Block::Section { children, .. } => ids.extend(children.iter().copied()),
            Block::List { items, .. } => {
                for item in items {
                    ids.extend(item.children.iter().copied());
                }
            }
            _ => {}
        }
    }
    ids
}

fn package_member_path(graph: &DocumentGraph, node: NodeId) -> Option<String> {
    graph.sources_for(node).into_iter().find_map(|source| match &source.kind {
        SourceKind::PackageMember { path } => Some(path.clone()),
        _ => None,
    })
}

fn is_navigation_only_group(graph: &DocumentGraph, group: &SpineGroup) -> bool {
    group.member_path.to_ascii_lowercase().contains("nav")
        && group
            .blocks
            .iter()
            .all(|node_id| graph.block(*node_id).map(|node| matches!(node.block, Block::List { .. })).unwrap_or(false))
}

fn chapter_label_from_member(member_path: &str, graph: &DocumentGraph, first_block: Option<NodeId>) -> String {
    if let Some(first) = first_block {
        if let Some(label) = section_title_plain_text(graph, first) {
            return label;
        }
    }
    let file_name = member_path.rsplit('/').next().unwrap_or(member_path);
    let stem = file_name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(file_name);
    sanitize_label(stem)
}

fn section_title_plain_text(graph: &DocumentGraph, node_id: NodeId) -> Option<String> {
    let node = graph.block(node_id)?;
    if let Block::Section { title, .. } = &node.block {
        let plain = plain_text(title);
        if !plain.is_empty() {
            return Some(plain);
        }
    }
    None
}

fn plain_text(inlines: &[notedown_ir::Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            notedown_ir::Inline::Text { text } => out.push_str(text),
            notedown_ir::Inline::Styled { children, .. } => out.push_str(&plain_text(children)),
            notedown_ir::Inline::Link { display, .. } => out.push_str(&plain_text(display)),
            notedown_ir::Inline::Reference { display, .. } => out.push_str(display),
            notedown_ir::Inline::InlineCode { text } => out.push_str(text),
            notedown_ir::Inline::Note { display, .. } => {
                if let Some(display) = display {
                    out.push_str(&plain_text(display));
                }
            }
            _ => {}
        }
    }
    out.trim().to_string()
}

fn sanitize_label(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    if out.is_empty() {
        "chapter".into()
    } else {
        out.into()
    }
}

/// Build the XHTML href for the chapter at the given spine index.
pub fn chapter_href(index: usize) -> String {
    format!("chapter-{index:03}.xhtml")
}

// ---------------------------------------------------------------------------
// Asset planning
// ---------------------------------------------------------------------------

/// Collect materialized image assets from the graph.
pub fn plan_assets(graph: &DocumentGraph) -> Vec<PlannedAsset> {
    let mut assets = Vec::new();
    let mut used_names: HashSet<String> = HashSet::new();
    for asset in &graph.assets {
        if asset.kind != AssetKind::Image {
            continue;
        }
        let Some(bytes) = asset.bytes.as_ref() else {
            continue;
        };
        let file_name = stable_asset_file_name(asset, &used_names);
        used_names.insert(file_name.clone());
        let media_type = guess_media_type(&file_name).unwrap_or_else(|| guess_from_bytes(bytes).unwrap_or("application/octet-stream".to_string()));
        let relative_path = format!("assets/{file_name}");
        assets.push(PlannedAsset {
            relative_path,
            bytes: bytes.clone(),
            media_type,
            asset_id: asset.id,
        });
    }
    assets
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
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
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

fn guess_media_type(file_name: &str) -> Option<String> {
    let ext = file_name.rsplit_once('.')?.1.to_ascii_lowercase();
    let media = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        _ => return None,
    };
    Some(media.into())
}

fn guess_from_bytes(bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(b"\x89PNG\r\n") {
        return Some("image/png".into());
    }
    if bytes.starts_with(b"\xFF\xD8") {
        return Some("image/jpeg".into());
    }
    if bytes.starts_with(b"GIF8") {
        return Some("image/gif".into());
    }
    if bytes.starts_with(b"RIFF") && bytes.len() > 11 && &bytes[8..12] == b"WEBP" {
        return Some("image/webp".into());
    }
    if bytes.starts_with(b"<?xml") || bytes.starts_with(b"<svg") {
        return Some("image/svg+xml".into());
    }
    None
}

#[derive(Debug, Clone)]
struct SpineGroup {
    member_path: String,
    blocks: Vec<NodeId>,
}

/// Map from IR asset source to package-relative asset path, used when rewriting
/// image `src` attributes inside the XHTML body.
pub fn asset_source_map(assets: &[PlannedAsset]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for asset in assets {
        let file_name = asset
            .relative_path
            .rsplit('/')
            .next()
            .unwrap_or(&asset.relative_path)
            .to_string();
        // Both bare file name and full relative path map to the href used inside OEBPS/.
        let href = asset.relative_path.clone();
        map.insert(file_name, href.clone());
        map.insert(asset.relative_path.clone(), href);
    }
    map
}
