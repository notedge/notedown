use std::path::Path;

use acorn_core::ParseBudget;
use acorn_epub::OcfPackage;
use notedown_ir::{
    DocumentGraph, DocumentId, DocumentMetadata, LossMarker, SemanticStatus, SourceKind, SourceRef,
};

use super::map_ocf_error;
use super::assets::{
    find_cover_manifest_item, hydrate_package_assets, register_cover_asset,
    register_stylesheet_assets,
};
use super::navigation::{find_nav_manifest_item, parse_nav_toc, push_navigation_toc};
use super::xhtml::blocks_from_xhtml_with_context;
use crate::FormatError;

/// Import an EPUB file from disk into `notedown-ir`.
pub fn import_epub(path: impl AsRef<Path>) -> Result<DocumentGraph, FormatError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|error| {
        FormatError::invalid_input(format!("failed to read {}: {error}", path.display()))
    })?;
    import_epub_bytes(&path.display().to_string(), &bytes)
}

/// Import EPUB bytes into `notedown-ir`.
pub fn import_epub_bytes(label: &str, bytes: &[u8]) -> Result<DocumentGraph, FormatError> {
    if !looks_like_zip(bytes) {
        return Err(FormatError::invalid_input(
            "input is not a ZIP-based EPUB package",
        ));
    }

    let package = OcfPackage::open(label.to_string(), bytes.to_vec()).map_err(map_ocf_error)?;
    let budget = ParseBudget::default();
    let opf = package.read_opf(&budget).map_err(map_ocf_error)?;

    let mut graph = DocumentGraph::new(document_id_for(label));
    graph.metadata = DocumentMetadata {
        title: opf.metadata.title.clone(),
        language: opf.metadata.language.clone(),
        authors: opf.metadata.creators.clone(),
        tags: Vec::new(),
    };

    let nav_manifest_id = find_nav_manifest_item(&opf).map(|item| item.id.clone());
    if let Some(cover_item) = find_cover_manifest_item(&opf) {
        let member_path = OcfPackage::resolve_href(package.root_opf_path(), &cover_item.href);
        register_cover_asset(&mut graph, &member_path, &cover_item.href);
    }
    register_stylesheet_assets(&mut graph, package.root_opf_path(), &opf);
    if let Some(nav_item) = find_nav_manifest_item(&opf) {
        let member_path = OcfPackage::resolve_href(package.root_opf_path(), &nav_item.href);
        match package.read_member(&member_path, &budget) {
            Ok(nav_xhtml) => match parse_nav_toc(&nav_xhtml) {
                Ok(entries) => {
                    let node = push_navigation_toc(&mut graph, &entries);
                    graph.attach_source(SourceRef {
                        node,
                        kind: SourceKind::PackageMember { path: member_path },
                        precision: SemanticStatus::Partial,
                    });
                }
                Err(error) => {
                    graph.push_loss(LossMarker {
                        code: "import.epub.navigation_unresolved".into(),
                        message: error.to_string(),
                        status: SemanticStatus::Unresolved,
                    });
                }
            },
            Err(error) => {
                graph.push_loss(LossMarker {
                    code: "import.epub.navigation_missing".into(),
                    message: map_ocf_error(error).to_string(),
                    status: SemanticStatus::Unresolved,
                });
            }
        }
    }

    for spine_item in &opf.spine {
        if nav_manifest_id.as_deref() == Some(spine_item.idref.as_str()) {
            continue;
        }
        let manifest = opf
            .manifest
            .get(&spine_item.idref)
            .ok_or_else(|| {
                FormatError::parse(
                    "epub",
                    format!("spine idref `{}` missing from manifest", spine_item.idref),
                )
            })?;
        if !manifest.media_type.contains("xhtml") && !manifest.media_type.contains("html") {
            graph.push_loss(LossMarker {
                code: "import.epub.unsupported_manifest_type".into(),
                message: format!(
                    "skipped spine item `{}` with media type `{}`",
                    manifest.id, manifest.media_type
                ),
                status: SemanticStatus::Unsupported,
            });
            continue;
        }

        let member_path = OcfPackage::resolve_href(package.root_opf_path(), &manifest.href);
        let xhtml = package.read_member(&member_path, &budget).map_err(map_ocf_error)?;
        let blocks = blocks_from_xhtml_with_context(
            &xhtml,
            &mut graph,
            Some(member_path.as_str()),
        )?;
        for block in blocks {
            let node = graph.push_block(block);
            graph.attach_source(SourceRef {
                node,
                kind: SourceKind::PackageMember { path: member_path.clone() },
                precision: SemanticStatus::Partial,
            });
        }
        for loss in super::xhtml::losses_from_xhtml(&xhtml)? {
            graph.push_loss(loss);
        }
    }

    hydrate_package_assets(&mut graph, &package, &budget);

    graph.push_loss(LossMarker {
        code: "import.epub.partial_coverage".into(),
        message: "EPUB import maps OPF metadata, EPUB3 navigation TOC, spine XHTML via oak-html with container unwrapping, blockquote, pre/code, hr, tables, nested lists, embedded image assets, manifest CSS members, and external SVG image references. Inline CSS semantics remain pending".into(),
        status: SemanticStatus::Partial,
    });
    Ok(graph)
}

fn looks_like_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")
}

fn document_id_for(label: &str) -> DocumentId {
    let mut hash = 1u64;
    for byte in label.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u64::from(byte));
    }
    DocumentId(hash)
}
