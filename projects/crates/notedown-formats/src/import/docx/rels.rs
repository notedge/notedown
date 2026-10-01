use std::collections::HashMap;

use crate::FormatError;

use super::map_opc_error;

/// Parses OPC relationship targets keyed by relationship id.
pub fn parse_relationship_targets(xml: &[u8]) -> Result<HashMap<String, String>, FormatError> {
    acorn_docx::parse_relationship_targets(xml).map_err(map_opc_error)
}

/// Reads `word/_rels/document.xml.rels` when present.
pub fn read_document_relationships(
    package: &acorn_docx::OpcPackage,
    budget: &acorn_core::ParseBudget,
) -> Result<HashMap<String, String>, FormatError> {
    package
        .read_relationship_targets("word/_rels/document.xml.rels", budget)
        .map_err(map_opc_error)
}
