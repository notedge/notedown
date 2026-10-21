use std::{fs, path::Path};

use acorn_core::{Budget, BudgetTracker};
use acorn_docx::OpcPackage;
use notedown_ir::{AssetKind, DocumentGraph, DocumentMetadata, LossMarker, SemanticStatus};

use super::{
    footnotes::{FootnoteCatalog, append_footnote_definitions, footnotes_part_path, parse_footnotes_xml_lossy},
    map_opc_error,
    numbering::{NumberingCatalog, parse_numbering_xml_lossy},
    rels::read_document_relationships,
    xml,
    xml_extensions::{document_root, element_text, elements_by_local_name, parse_xml_bytes},
};
use crate::FormatError;

const DOCUMENT_XML: &str = "word/document.xml";
const NUMBERING_XML: &str = "word/numbering.xml";

/// Import a DOCX file from disk into `notedown-ir`.
pub fn import_docx(path: impl AsRef<Path>) -> Result<DocumentGraph, FormatError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| FormatError::invalid_input(format!("failed to read {}: {error}", path.display())))?;
    import_docx_bytes(&path.display().to_string(), &bytes)
}

/// Import DOCX bytes into `notedown-ir`.
pub fn import_docx_bytes(label: &str, bytes: &[u8]) -> Result<DocumentGraph, FormatError> {
    if looks_like_ole(bytes) {
        return Err(FormatError::not_implemented("doc", "import"));
    }
    if !looks_like_zip(bytes) {
        return Err(FormatError::invalid_input("input is not a ZIP-based DOCX package"));
    }

    let package = OpcPackage::open(label.to_string(), bytes.to_vec()).map_err(map_opc_error)?;
    let mut ledger = BudgetTracker::new(Budget::default());
    let document_part = package.main_document_part_path(&mut ledger).map_err(map_opc_error)?.unwrap_or_else(|| DOCUMENT_XML.to_string());
    let xml = package.read_part(&document_part, &mut ledger).map_err(map_opc_error)?;

    let rels = read_document_relationships(&package, &document_part, &mut ledger)?;
    let numbering = read_numbering_catalog(&package, &mut ledger);
    let footnotes = read_footnote_catalog(&package, &mut ledger);
    let mut graph = xml::new_graph(label);
    let referenced_footnotes = xml::parse_document_xml(&xml, &rels, &numbering, &footnotes, &mut graph)?;
    append_footnote_definitions(&mut graph, &footnotes, &referenced_footnotes);
    hydrate_embedded_assets(&package, &mut ledger, &mut graph);
    hydrate_core_metadata(&package, &mut ledger, &mut graph);
    graph.push_loss(LossMarker {
        code: "import.docx.partial_coverage".into(),
        message: "DOCX import currently maps paragraphs, heading styles, nested lists with numbering.xml marker resolution, tables, run bold/italic/underline/strike/superscript/subscript/color/font-size/font-family/highlight, hyperlinks, embedded images, footnote references, footnote bodies from footnotes.xml, and core metadata. Tracked changes, comments, page-break layout, and complex character properties remain partial".into(),
        status: SemanticStatus::Partial,
    });
    Ok(graph)
}

fn hydrate_core_metadata(package: &OpcPackage, ledger: &mut BudgetTracker, graph: &mut DocumentGraph) {
    let Ok(xml) = package.read_part("docProps/core.xml", ledger)
    else {
        return;
    };
    let Ok(value) = parse_xml_bytes(&xml)
    else {
        graph.push_loss(LossMarker {
            code: "reader.docx.invalid_core_metadata".into(),
            message: "docProps/core.xml could not be parsed as XML".into(),
            status: SemanticStatus::Partial,
        });
        return;
    };
    let Ok(root) = document_root(&value)
    else {
        return;
    };
    let text = |name: &str| {
        elements_by_local_name(root, name).first().map(|element| element_text(element).trim().to_owned()).filter(|value| !value.is_empty())
    };
    let tags = text("keywords")
        .map(|value| {
            value.split(',').flat_map(|part| part.split(';')).map(str::trim).filter(|item| !item.is_empty()).map(str::to_owned).collect()
        })
        .unwrap_or_default();
    graph.metadata =
        DocumentMetadata { title: text("title"), language: text("language"), authors: text("creator").into_iter().collect(), tags };
}

fn looks_like_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")
}

fn looks_like_ole(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1])
}

fn read_numbering_catalog(package: &OpcPackage, ledger: &mut BudgetTracker) -> NumberingCatalog {
    package.read_part(NUMBERING_XML, ledger).ok().map(|xml| parse_numbering_xml_lossy(&xml)).unwrap_or_default()
}

fn read_footnote_catalog(package: &OpcPackage, ledger: &mut BudgetTracker) -> FootnoteCatalog {
    package.read_part(footnotes_part_path(), ledger).ok().map(|xml| parse_footnotes_xml_lossy(&xml)).unwrap_or_default()
}

fn hydrate_embedded_assets(package: &OpcPackage, ledger: &mut BudgetTracker, graph: &mut DocumentGraph) {
    for asset in &mut graph.assets {
        if asset.kind != AssetKind::Image {
            continue;
        }
        let Some(source) = asset.source.as_ref()
        else {
            continue;
        };
        let part_path = format!("word/{source}");
        if let Ok(bytes) = package.read_part(&part_path, ledger) {
            asset.bytes = Some(bytes);
        }
    }
}
