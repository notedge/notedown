use acorn_core::ParseBudget;
use acorn_epub::{ManifestItem, OcfPackage, OpfDocument};
use notedown_ir::{
    Asset, AssetId, AssetKind, DocumentGraph, Inline, SemanticStatus,
};
/// Locate the EPUB3 cover image manifest item when declared via `properties="cover-image"`.
pub fn find_cover_manifest_item(opf: &OpfDocument) -> Option<&ManifestItem> {
    opf.manifest.values().find(|item| {
        item.properties
            .as_deref()
            .map(|properties| properties.split_whitespace().any(|token| token == "cover-image"))
            .unwrap_or(false)
    })
}

/// Register a cover asset from the OPF manifest without requiring spine presence.
pub fn register_cover_asset(graph: &mut DocumentGraph, member_path: &str, display_href: &str) {
    if graph
        .assets
        .iter()
        .any(|asset| asset.source.as_deref() == Some(member_path))
    {
        return;
    }
    let asset_id = AssetId(graph.assets.len() as u64 + 1);
    graph.push_asset(Asset {
        id: asset_id,
        kind: AssetKind::Image,
        content_identity: None,
        source: Some(member_path.to_string()),
        media_type: media_type_for(display_href),
        status: SemanticStatus::Resolved,
        bytes: None,
    });
}

/// Register an embedded image asset resolved from a spine XHTML member.
pub fn register_image_from_src(
    graph: &mut DocumentGraph,
    member_path: &str,
    src: &str,
) {
    if src.is_empty() {
        return;
    }
    let resolved = OcfPackage::resolve_href(member_path, src);
    register_image_asset(graph, &resolved, src);
}

/// Lower an `<img>` element into a Notedown image inline.
pub fn image_inline(src: &str, alt: &str) -> Option<Inline> {
    if src.is_empty() {
        return None;
    }
    Some(Inline::Styled {
        style: "image".into(),
        children: vec![
            Inline::Text {
                text: alt.to_string(),
            },
            Inline::Text {
                text: src.to_string(),
            },
        ],
    })
}

fn register_image_asset(graph: &mut DocumentGraph, member_path: &str, display_href: &str) {
    if graph
        .assets
        .iter()
        .any(|asset| asset.source.as_deref() == Some(member_path))
    {
        return;
    }
    let asset_id = AssetId(graph.assets.len() as u64 + 1);
    graph.push_asset(Asset {
        id: asset_id,
        kind: AssetKind::Image,
        content_identity: None,
        source: Some(member_path.to_string()),
        media_type: media_type_for(display_href),
        status: SemanticStatus::Resolved,
        bytes: None,
    });
}

/// Materialize embedded image bytes for registered assets from the OCF package.
pub fn hydrate_image_assets(
    graph: &mut DocumentGraph,
    package: &OcfPackage,
    budget: &ParseBudget,
) {
    for asset in &mut graph.assets {
        if asset.kind != AssetKind::Image {
            continue;
        }
        let Some(source) = asset.source.as_ref() else {
            continue;
        };
        if let Ok(bytes) = package.read_member(source, budget) {
            asset.bytes = Some(bytes);
        }
    }
}

fn media_type_for(path: &str) -> Option<String> {
    let extension = path.rsplit('.').next()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some("image/png".into()),
        "jpg" | "jpeg" => Some("image/jpeg".into()),
        "gif" => Some("image/gif".into()),
        "webp" => Some("image/webp".into()),
        "svg" => Some("image/svg+xml".into()),
        _ => None,
    }
}
