use std::collections::HashMap;

use crate::FormatError;

use super::map_opc_error;
use super::oak_xml_util::{
    attribute_value, document_root, elements_by_local_name, parse_xml_bytes,
};

/// Parses OPC relationship targets keyed by relationship id.
pub fn parse_relationship_targets(xml: &[u8]) -> Result<HashMap<String, String>, FormatError> {
    let value = parse_xml_bytes(xml)?;
    let root = document_root(&value)?;
    let mut targets = HashMap::new();
    for relationship in elements_by_local_name(root, "Relationship") {
        let id = attribute_value(relationship, "Id");
        let target = attribute_value(relationship, "Target");
        if let (Some(id), Some(target)) = (id, target) {
            targets.insert(id, target);
        }
    }
    Ok(targets)
}

/// Reads `word/_rels/document.xml.rels` when present.
pub fn read_document_relationships(
    package: &acorn_docx::OpcPackage,
    budget: &acorn_core::ParseBudget,
) -> Result<HashMap<String, String>, FormatError> {
    match package.read_part("word/_rels/document.xml.rels", budget) {
        Ok(xml) => parse_relationship_targets(&xml),
        Err(acorn_docx::OpcError::PartNotFound(_)) => Ok(HashMap::new()),
        Err(error) => Err(map_opc_error(error)),
    }
}
