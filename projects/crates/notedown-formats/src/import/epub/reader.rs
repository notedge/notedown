use std::path::Path;

use acorn_core::ParseBudget;
use acorn_epub::OcfPackage;
use notedown_ir::{
    DocumentGraph, DocumentId, DocumentMetadata, LossMarker, SemanticStatus, SourceKind, SourceRef,
};

use super::map_ocf_error;
use super::xhtml::blocks_from_xhtml;
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

    for spine_item in &opf.spine {
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
        let blocks = blocks_from_xhtml(&xhtml)?;
        for block in blocks {
            let node = graph.push_block(block);
            graph.attach_source(SourceRef {
                node,
                kind: SourceKind::PackageMember { path: member_path.clone() },
                precision: SemanticStatus::Partial,
            });
        }
    }

    graph.push_loss(LossMarker {
        code: "import.epub.partial_coverage".into(),
        message: "EPUB import currently maps OPF metadata and basic XHTML headings/paragraphs without oak-html lowering".into(),
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
        hash = hash * 31 + u64::from(byte);
    }
    DocumentId(hash)
}
