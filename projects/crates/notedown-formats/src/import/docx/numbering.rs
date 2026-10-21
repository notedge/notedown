use std::collections::HashMap;

use crate::FormatError;

use super::oak_xml_util::{
    attribute_value, child_element, document_root, elements_by_local_name, parse_xml_bytes,
    u32_attribute,
};

/// Resolved list marker styles from `word/numbering.xml`.
#[derive(Debug, Default, Clone)]
pub struct NumberingCatalog {
    abstract_levels: HashMap<u32, HashMap<u32, String>>,
    num_to_abstract: HashMap<u32, u32>,
}

impl NumberingCatalog {
    /// Returns whether the list marker is ordered when numbering metadata is known.
    pub fn is_ordered(&self, num_id: u32, ilvl: u32) -> Option<bool> {
        let abstract_id = self.num_to_abstract.get(&num_id)?;
        let levels = self.abstract_levels.get(abstract_id)?;
        let format = levels.get(&ilvl)?;
        Some(num_fmt_is_ordered(format))
    }
}

/// Parses `word/numbering.xml` into a lookup table for list marker styles.
pub fn parse_numbering_xml(xml: &[u8]) -> Result<NumberingCatalog, FormatError> {
    let value = parse_xml_bytes(xml)?;
    let root = document_root(&value)?;
    let mut catalog = NumberingCatalog::default();

    for abstract_num in elements_by_local_name(root, "abstractNum") {
        let Some(abstract_id) = u32_attribute(abstract_num, "abstractNumId") else {
            continue;
        };
        for lvl in elements_by_local_name(abstract_num, "lvl") {
            let Some(ilvl) = u32_attribute(lvl, "ilvl") else {
                continue;
            };
            if let Some(num_fmt) = child_element(lvl, "numFmt") {
                if let Some(format) = attribute_value(num_fmt, "val") {
                    catalog
                        .abstract_levels
                        .entry(abstract_id)
                        .or_default()
                        .insert(ilvl, format);
                }
            }
        }
    }

    for num in elements_by_local_name(root, "num") {
        let Some(num_id) = u32_attribute(num, "numId") else {
            continue;
        };
        if let Some(abstract_num_id) = child_element(num, "abstractNumId") {
            if let Some(abstract_id) = u32_attribute(abstract_num_id, "val") {
                catalog.num_to_abstract.insert(num_id, abstract_id);
            }
        }
    }

    Ok(catalog)
}

fn num_fmt_is_ordered(format: &str) -> bool {
    !matches!(
        format,
        "bullet" | "none" | "chart" | "image" | "customBullet"
    )
}

/// Best-effort numbering parse that ignores malformed fragments.
pub fn parse_numbering_xml_lossy(xml: &[u8]) -> NumberingCatalog {
    parse_numbering_xml(xml).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_decimal_numbering_as_ordered() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0">
      <w:numFmt w:val="decimal"/>
    </w:lvl>
  </w:abstractNum>
  <w:num w:numId="1">
    <w:abstractNumId w:val="0"/>
  </w:num>
</w:numbering>"#;
        let catalog = parse_numbering_xml(xml).expect("parse numbering");
        assert_eq!(catalog.is_ordered(1, 0), Some(true));
    }

    #[test]
    fn resolves_bullet_numbering_as_unordered() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0">
      <w:numFmt w:val="bullet"/>
    </w:lvl>
  </w:abstractNum>
  <w:num w:numId="2">
    <w:abstractNumId w:val="1"/>
  </w:num>
</w:numbering>"#;
        let catalog = parse_numbering_xml(xml).expect("parse numbering");
        assert_eq!(catalog.is_ordered(2, 0), Some(false));
    }
}
