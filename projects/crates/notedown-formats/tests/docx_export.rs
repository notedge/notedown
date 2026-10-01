use notedown_formats::export::docx::export_docx_bytes;
use notedown_formats::export::markdown::export_markdown;
use notedown_formats::import::docx::import_docx_bytes;

fn stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = Vec::new();
    let mut central = Vec::new();

    for (path, payload) in entries {
        let name = path.as_bytes();
        let crc = crc32(payload);
        let local_offset = archive.len();

        archive.extend_from_slice(b"PK\x03\x04");
        archive.extend_from_slice(&[0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        archive.extend_from_slice(&crc.to_le_bytes());
        archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        archive.extend_from_slice(&(name.len() as u16).to_le_bytes());
        archive.extend_from_slice(&[0x00, 0x00]);
        archive.extend_from_slice(name);
        archive.extend_from_slice(payload);

        central.extend_from_slice(b"PK\x01\x02");
        let mut cd_fixed = [0u8; 46];
        cd_fixed[0..2].copy_from_slice(&[0x14, 0x00]);
        cd_fixed[2..4].copy_from_slice(&[0x14, 0x00]);
        cd_fixed[12..16].copy_from_slice(&crc.to_le_bytes());
        cd_fixed[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        cd_fixed[20..24].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        cd_fixed[24..26].copy_from_slice(&(name.len() as u16).to_le_bytes());
        cd_fixed[38..42].copy_from_slice(&(local_offset as u32).to_le_bytes());
        central.extend_from_slice(&cd_fixed);
        central.extend_from_slice(name);
    }

    let cd_offset = archive.len();
    archive.extend_from_slice(&central);
    let cd_size = central.len();
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(cd_size as u32).to_le_bytes());
    archive.extend_from_slice(&(cd_offset as u32).to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);
    archive
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let bit = crc & 1;
            crc >>= 1;
            if bit != 0 {
                crc ^= 0xEDB8_8320;
            }
        }
    }
    crc ^ 0xFFFF_FFFF
}

fn minimal_docx_zip() -> Vec<u8> {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello DOCX</w:t></w:r></w:p>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>Title</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
    stored_zip(&[("word/document.xml", document_xml)])
}

#[test]
fn docx_export_round_trips_paragraphs_and_headings() {
    let zip = minimal_docx_zip();
    let graph = import_docx_bytes("sample.docx", &zip).expect("import docx");
    let exported = export_docx_bytes(&graph).expect("export docx");
    let round = import_docx_bytes("round.docx", &exported).expect("re-import docx");
    let markdown = export_markdown(&round).expect("export markdown");
    assert!(markdown.contains("Hello DOCX"));
    assert!(markdown.contains("# Title"));
}

#[test]
fn docx_export_round_trips_lists_with_numbering() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>One</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Two</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
    let zip = stored_zip(&[("word/document.xml", document_xml)]);
    let graph = import_docx_bytes("list.docx", &zip).expect("import docx");
    let exported = export_docx_bytes(&graph).expect("export docx");
    let payload = String::from_utf8_lossy(&exported);
    assert!(payload.contains("word/numbering.xml"));
    let round = import_docx_bytes("round.docx", &exported).expect("re-import docx");
    let markdown = export_markdown(&round).expect("export markdown");
    assert!(markdown.contains("- One"));
    assert!(markdown.contains("- Two"));
}

#[test]
fn docx_export_includes_minimal_opc_parts() {
    let zip = minimal_docx_zip();
    let graph = import_docx_bytes("sample.docx", &zip).expect("import docx");
    let exported = export_docx_bytes(&graph).expect("export docx");
    let members = String::from_utf8_lossy(&exported);
    assert!(members.contains("[Content_Types].xml"));
    assert!(members.contains("_rels/.rels"));
    assert!(members.contains("word/_rels/document.xml.rels"));
    assert!(members.contains("word/document.xml"));
}

#[test]
fn docx_export_round_trips_tables() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>H1</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>H2</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
  </w:body>
</w:document>"#;
    let zip = stored_zip(&[("word/document.xml", document_xml)]);
    let graph = import_docx_bytes("table.docx", &zip).expect("import docx");
    let exported = export_docx_bytes(&graph).expect("export docx");
    let round = import_docx_bytes("round.docx", &exported).expect("re-import docx");
    let markdown = export_markdown(&round).expect("export markdown");
    assert!(markdown.contains("| H1 | H2 |"));
    assert!(markdown.contains("| A | B |"));
}

#[test]
fn docx_export_round_trips_hyperlinks() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p>
      <w:hyperlink r:id="rId1">
        <w:r><w:t>Example</w:t></w:r>
      </w:hyperlink>
    </w:p>
  </w:body>
</w:document>"#;
    let rels_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink"
    Target="https://example.com" TargetMode="External"/>
</Relationships>"#;
    let zip = stored_zip(&[
        ("word/document.xml", document_xml),
        ("word/_rels/document.xml.rels", rels_xml),
    ]);
    let graph = import_docx_bytes("links.docx", &zip).expect("import docx");
    let exported = export_docx_bytes(&graph).expect("export docx");
    let payload = String::from_utf8_lossy(&exported);
    assert!(payload.contains("https://example.com"));
    let round = import_docx_bytes("round.docx", &exported).expect("re-import docx");
    let markdown = export_markdown(&round).expect("export markdown");
    assert!(markdown.contains("[Example](https://example.com)"));
}

#[test]
fn docx_export_round_trips_embedded_images() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing">
  <w:body>
    <w:p>
      <w:r>
        <w:drawing>
          <wp:inline>
            <wp:docPr descr="Logo"/>
            <a:graphic>
              <a:graphicData>
                <a:blip r:embed="rId2"/>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#;
    let rels_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId2"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
    Target="media/logo.png"/>
</Relationships>"#;
    let image_bytes = b"\x89PNG\r\n";
    let zip = stored_zip(&[
        ("word/document.xml", document_xml),
        ("word/_rels/document.xml.rels", rels_xml),
        ("word/media/logo.png", image_bytes),
    ]);
    let graph = import_docx_bytes("image.docx", &zip).expect("import docx");
    let exported = export_docx_bytes(&graph).expect("export docx");
    let payload = String::from_utf8_lossy(&exported);
    assert!(payload.contains("word/media/logo.png"));
    let round = import_docx_bytes("round.docx", &exported).expect("re-import docx");
    let markdown = export_markdown(&round).expect("export markdown");
    assert!(markdown.contains("![Logo](media/logo.png)"));
}
