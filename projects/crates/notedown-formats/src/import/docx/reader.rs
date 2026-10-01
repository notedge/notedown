use std::fs;
use std::path::Path;

use acorn_core::ParseBudget;
use acorn_docx::OpcPackage;
use notedown_ir::{AssetKind, DocumentGraph, LossMarker, SemanticStatus};

use super::footnotes::{
    append_footnote_definitions, footnotes_part_path, parse_footnotes_xml_lossy, FootnoteCatalog,
};
use super::numbering::{parse_numbering_xml_lossy, NumberingCatalog};
use super::rels::read_document_relationships;
use super::{map_opc_error, xml};
use crate::FormatError;

const DOCUMENT_XML: &str = "word/document.xml";
const NUMBERING_XML: &str = "word/numbering.xml";

/// Import a DOCX file from disk into `notedown-ir`.
pub fn import_docx(path: impl AsRef<Path>) -> Result<DocumentGraph, FormatError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        FormatError::invalid_input(format!("failed to read {}: {error}", path.display()))
    })?;
    import_docx_bytes(&path.display().to_string(), &bytes)
}

/// Import DOCX bytes into `notedown-ir`.
pub fn import_docx_bytes(label: &str, bytes: &[u8]) -> Result<DocumentGraph, FormatError> {
    if looks_like_ole(bytes) {
        return Err(FormatError::not_implemented("doc", "import"));
    }
    if !looks_like_zip(bytes) {
        return Err(FormatError::invalid_input(
            "input is not a ZIP-based DOCX package",
        ));
    }

    let package = OpcPackage::open(label.to_string(), bytes.to_vec()).map_err(map_opc_error)?;
    let budget = ParseBudget::default();
    let document_part = package
        .main_document_part_path(&budget)
        .map_err(map_opc_error)?
        .unwrap_or_else(|| DOCUMENT_XML.to_string());
    let xml = package
        .read_part(&document_part, &budget)
        .map_err(map_opc_error)?;

    let rels = read_document_relationships(&package, &document_part, &budget)?;
    let numbering = read_numbering_catalog(&package, &budget);
    let footnotes = read_footnote_catalog(&package, &budget);
    let mut graph = xml::new_graph(label);
    let referenced_footnotes =
        xml::parse_document_xml(&xml, &rels, &numbering, &footnotes, &mut graph)?;
    append_footnote_definitions(&mut graph, &footnotes, &referenced_footnotes);
    hydrate_embedded_assets(&package, &budget, &mut graph);
    graph.push_loss(LossMarker {
        code: "import.docx.partial_coverage".into(),
        message: "DOCX import currently maps paragraphs, heading styles, lists with numbering.xml marker resolution, tables, run bold/italic, hyperlinks, embedded images, footnote references, and footnote bodies from footnotes.xml".into(),
        status: SemanticStatus::Partial,
    });
    Ok(graph)
}

fn looks_like_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")
}

fn looks_like_ole(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1])
}

fn read_numbering_catalog(package: &OpcPackage, budget: &ParseBudget) -> NumberingCatalog {
    package
        .read_part(NUMBERING_XML, budget)
        .ok()
        .map(|xml| parse_numbering_xml_lossy(&xml))
        .unwrap_or_default()
}

fn read_footnote_catalog(package: &OpcPackage, budget: &ParseBudget) -> FootnoteCatalog {
    package
        .read_part(footnotes_part_path(), budget)
        .ok()
        .map(|xml| parse_footnotes_xml_lossy(&xml))
        .unwrap_or_default()
}

fn hydrate_embedded_assets(package: &OpcPackage, budget: &ParseBudget, graph: &mut DocumentGraph) {
    for asset in &mut graph.assets {
        if asset.kind != AssetKind::Image {
            continue;
        }
        let Some(source) = asset.source.as_ref() else {
            continue;
        };
        let part_path = format!("word/{source}");
        if let Ok(bytes) = package.read_part(&part_path, budget) {
            asset.bytes = Some(bytes);
        }
    }
}
