use notedown_formats::import::docx::numbering::parse_numbering_xml;

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
