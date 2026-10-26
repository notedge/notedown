//! EPUB 3 export from `notedown-ir`.
//!
//! Conservative exporter that renders a document graph into a minimal EPUB 3
//! OCF package: `mimetype`, `META-INF/container.xml`, `OEBPS/content.opf`,
//! `OEBPS/nav.xhtml`, and one XHTML chapter per spine group. Image assets with
//! materialized bytes are written under `OEBPS/assets/`.
//!
//! Known semantic gaps (why this is `Partial`):
//!
//! * `Block::Diagram` source is wrapped in a `<pre data-diagram-*>` placeholder
//!   and not converted to an image.
//! * `Block::Math` is emitted as a `<div class="math">`; no MathML output.
//! * `Inline::Citation` and unresolved references return an unsupported error
//!   rather than a graceful fallback.
//! * Opaque blocks beyond thematic breaks fail the export.
//! * No embedded CSS stylesheet — EPUB readers fall back to their default.

mod chapter;
mod container;
mod nav;
mod opf;
mod package;
mod xhtml;

use std::path::Path;

use notedown_ir::DocumentGraph;

use crate::FormatError;

/// Export a document graph to EPUB 3 bytes.
pub fn export_epub_bytes(graph: &DocumentGraph) -> Result<Vec<u8>, FormatError> {
    let validation = graph.validate();
    if !validation.is_valid() {
        return Err(FormatError::invalid_input(format!(
            "invalid document graph: {:?}",
            validation.issues
        )));
    }

    let chapters = chapter::plan_chapters(graph);
    if chapters.is_empty() {
        return Err(FormatError::unsupported(
            "epub",
            "document graph has no blocks to export",
        ));
    }
    let assets = chapter::plan_assets(graph);

    let mut opf_items = Vec::with_capacity(chapters.len() + 3);
    let mut spine_items = Vec::with_capacity(chapters.len());
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();

    // 1. mimetype — MUST be the first STORED entry in the ZIP.
    entries.push(("mimetype".to_string(), b"application/epub+zip".to_vec()));

    // 2. container.xml.
    let container_xml = container::container_xml();
    entries.push(("META-INF/container.xml".to_string(), container_xml.as_bytes().to_vec()));

    // 3. nav.xhtml (EPUB3 navigation document).
    let nav_xhtml = nav::nav_xhtml(graph, &chapters)?;
    entries.push(("OEBPS/nav.xhtml".to_string(), nav_xhtml.as_bytes().to_vec()));
    opf_items.push(opf::OpfManifestItem {
        id: "nav".into(),
        href: "nav.xhtml".into(),
        media_type: "application/xhtml+xml".into(),
        properties: Some("nav".into()),
    });

    // 4. XHTML chapters.
    for (index, plan) in chapters.iter().enumerate() {
        let xhtml = xhtml::chapter_xhtml(graph, plan)?;
        let href = chapter::chapter_href(index);
        entries.push((format!("OEBPS/{href}"), xhtml.as_bytes().to_vec()));
        let id = format!("ch{index:03}");
        opf_items.push(opf::OpfManifestItem {
            id: id.clone(),
            href: href.clone(),
            media_type: "application/xhtml+xml".into(),
            properties: None,
        });
        spine_items.push(opf::OpfSpineItem { idref: id });
    }

    // 5. Materialized image assets.
    for asset in &assets {
        let href = asset.relative_path.clone();
        let entry_path = format!("OEBPS/{href}");
        entries.push((entry_path, asset.bytes.clone()));
        let id = format!("asset-{}", asset.asset_id.0);
        opf_items.push(opf::OpfManifestItem {
            id,
            href,
            media_type: asset.media_type.clone(),
            properties: None,
        });
    }

    // 6. content.opf — must come after all members it references are known.
    let opf_xml = opf::content_opf(graph, &opf_items, &spine_items)?;
    entries.push(("OEBPS/content.opf".to_string(), opf_xml.as_bytes().to_vec()));

    Ok(package::build_epub_package(&entries))
}

/// Export a document graph to an EPUB file on disk.
pub fn export_epub(graph: &DocumentGraph, path: impl AsRef<Path>) -> Result<(), FormatError> {
    let bytes = export_epub_bytes(graph)?;
    std::fs::write(path.as_ref(), bytes).map_err(|error| {
        FormatError::invalid_input(format!(
            "failed to write {}: {error}",
            path.as_ref().display()
        ))
    })
}
