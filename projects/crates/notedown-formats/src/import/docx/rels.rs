use std::collections::HashMap;

use crate::FormatError;

use super::map_opc_error;

/// Parses OPC relationship targets keyed by relationship id.
pub fn parse_relationship_targets(xml: &[u8]) -> Result<HashMap<String, String>, FormatError> {
    acorn_docx::parse_relationship_targets(xml).map_err(map_opc_error)
}

/// Derives the OPC relationships part path for a document part.
pub fn document_rels_part_path(document_part: &str) -> String {
    let normalized = acorn_docx::normalize_part_path(document_part);
    if let Some((dir, file)) = normalized.rsplit_once('/') {
        format!("{dir}/_rels/{file}.rels")
    } else {
        format!("_rels/{normalized}.rels")
    }
}

/// Reads document relationships for the given main document part when present.
pub fn read_document_relationships(
    package: &acorn_docx::OpcPackage,
    document_part: &str,
    budget: &acorn_core::ParseBudget,
) -> Result<HashMap<String, String>, FormatError> {
    package
        .read_relationship_targets(&document_rels_part_path(document_part), budget)
        .map_err(map_opc_error)
}
