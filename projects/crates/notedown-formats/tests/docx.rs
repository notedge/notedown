use notedown_formats::export::markdown::export_markdown;
use notedown_formats::import::docx::import_docx_bytes;
use notedown_ir::{Block, Inline};

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

fn docx_zip(document_xml: &[u8]) -> Vec<u8> {
    stored_zip(&[("word/document.xml", document_xml)])
}

fn docx_zip_with_footnotes(document_xml: &[u8], footnotes_xml: &[u8]) -> Vec<u8> {
    stored_zip(&[
        ("word/document.xml", document_xml),
        ("word/footnotes.xml", footnotes_xml),
    ])
}

#[test]
fn docx_to_markdown_round_trip() {
    let zip = minimal_docx_zip();
    let graph = import_docx_bytes("sample.docx", &zip).expect("import docx");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("Hello DOCX"));
    assert!(markdown.contains("# Title"));
}

#[test]
fn docx_imports_run_bold_and_italic() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:rPr><w:b/></w:rPr><w:t>Bold</w:t></w:r>
      <w:r><w:rPr><w:i/></w:rPr><w:t> italic</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
    let zip = docx_zip(document_xml);
    let graph = import_docx_bytes("styled.docx", &zip).expect("import docx");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("**Bold**"));
    assert!(markdown.contains("* italic*"));
}

#[test]
fn docx_imports_underline_and_strike_without_losing_markdown_strike() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p>
    <w:r><w:rPr><w:u/></w:rPr><w:t>Underlined</w:t></w:r>
    <w:r><w:rPr><w:u w:val="none"/></w:rPr><w:t> plain</w:t></w:r>
    <w:r><w:rPr><w:strike/></w:rPr><w:t> crossed</w:t></w:r>
    <w:r><w:rPr><w:b/><w:i/><w:u w:val="double"/><w:strike/></w:rPr><w:t> combined</w:t></w:r>
  </w:p></w:body>
</w:document>"#;
    let graph = import_docx_bytes("character-styles.docx", &docx_zip(document_xml)).expect("import DOCX");
    let Block::Paragraph { content } = &graph.blocks[0].block else { panic!("paragraph expected") };
    assert!(matches!(&content[0], Inline::Styled { style, .. } if style == "underline"));
    assert!(matches!(&content[1], Inline::Text { text } if text == " plain"));
    assert!(matches!(&content[2], Inline::Styled { style, .. } if style == "strike"));
    assert!(matches!(&content[3], Inline::Styled { style, children } if style == "bold" && matches!(&children[0], Inline::Styled { style, children } if style == "italic" && matches!(&children[0], Inline::Styled { style, .. } if style == "underline"))));
    assert!(export_markdown(&graph).is_err(), "Markdown must not silently drop underline");
    let strike_only = notedown_ir::Inline::Styled { style: "strike".into(), children: vec![Inline::Text { text: "crossed".into() }] };
    let mut strike_graph = notedown_ir::DocumentGraph::new(notedown_ir::DocumentId(55));
    strike_graph.push_block(Block::Paragraph { content: vec![strike_only] });
    assert!(export_markdown(&strike_graph).expect("Markdown strike").contains("~~crossed~~"));
}

#[test]
fn docx_import_reports_field_semantics_that_are_not_in_ir() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> PAGE </w:instrText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t>visible fallback</w:t></w:r></w:p></w:body>
</w:document>"#;
    let graph = import_docx_bytes("field.docx", &docx_zip(document_xml)).expect("import field DOCX");
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "reader.docx.fields"));
    assert!(format!("{:?}", graph.blocks).contains("visible fallback"));
}

#[test]
fn docx_import_reports_unmapped_embedded_objects() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:r><w:object><w:oleObject r:id="rId1"/></w:object><w:pict/><w:t>visible fallback</w:t></w:r></w:p></w:body>
</w:document>"#;
    let graph = import_docx_bytes("embedded-object.docx", &docx_zip(document_xml)).expect("import embedded object DOCX");
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "reader.docx.embedded_objects"));
    assert!(format!("{:?}", graph.blocks).contains("visible fallback"));
}

#[test]
fn docx_imports_superscript_and_subscript_runs() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:rPr><w:vertAlign w:val="superscript"/></w:rPr><w:t>2</w:t></w:r><w:r><w:rPr><w:vertAlign w:val="subscript"/></w:rPr><w:t>i</w:t></w:r></w:p></w:body>
</w:document>"#;
    let graph = import_docx_bytes("vertical-align.docx", &docx_zip(document_xml)).expect("import vertical alignment");
    let Block::Paragraph { content } = &graph.blocks[0].block else { panic!("paragraph expected") };
    assert!(matches!(&content[0], Inline::Styled { style, .. } if style == "superscript"));
    assert!(matches!(&content[1], Inline::Styled { style, .. } if style == "subscript"));
}

#[test]
fn docx_import_reports_unmapped_character_properties() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:rPr><w:color w:val="FF0000"/><w:sz w:val="28"/><w:highlight w:val="yellow"/></w:rPr><w:t>colored text</w:t></w:r></p></w:body>
</w:document>"#;
    let mut document_xml = document_xml.to_vec();
    let marker = b"</w:body>";
    let insertion = document_xml.windows(marker.len()).position(|window| window == marker).expect("body marker");
    document_xml.splice(insertion..insertion, br#"<w:p><w:r><w:rPr><w:spacing w:val="20"/></w:rPr><w:t>unsupported</w:t></w:r></w:p>"#.iter().copied());
    let graph = import_docx_bytes("character-properties.docx", &docx_zip(&document_xml)).expect("import character properties");
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "reader.docx.character_properties"));
    assert!(format!("{:?}", graph.blocks).contains("colored text"));
}

#[test]
fn docx_imports_color_as_a_value_carrying_style() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:rPr><w:color w:val="FF0000"/></w:rPr><w:t>red</w:t></w:r></w:p></w:body>
</w:document>"#;
    let graph = import_docx_bytes("color.docx", &docx_zip(document_xml)).expect("import color");
    let Block::Paragraph { content } = &graph.blocks[0].block else { panic!("paragraph expected") };
    assert!(matches!(&content[0], Inline::Styled { style, .. } if style == "color:ff0000"));
    assert!(!graph.coverage.loss.iter().any(|loss| loss.code == "reader.docx.character_properties"));
}

#[test]
fn docx_imports_font_size_family_and_highlight_styles() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:rPr><w:sz w:val="28"/><w:rFonts w:ascii="Aptos"/><w:highlight w:val="yellow"/></w:rPr><w:t>styled</w:t></w:r></w:p></w:body>
</w:document>"#;
    let graph = import_docx_bytes("valued-character-styles.docx", &docx_zip(document_xml)).expect("import valued styles");
    let Block::Paragraph { content } = &graph.blocks[0].block else { panic!("paragraph expected") };
    let rendered = format!("{:?}", content);
    assert!(rendered.contains("font-size:28"), "{rendered}");
    assert!(rendered.contains("font-family:Aptos"), "{rendered}");
    assert!(rendered.contains("highlight:yellow"), "{rendered}");
    assert!(!graph.coverage.loss.iter().any(|loss| loss.code == "reader.docx.character_properties"));
}

#[test]
fn docx_imports_hyperlinks_from_relationships() {
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
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("[Example](https://example.com)"));
}

#[test]
fn docx_imports_embedded_images() {
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
    let zip = stored_zip(&[
        ("word/document.xml", document_xml),
        ("word/_rels/document.xml.rels", rels_xml),
        ("media/logo.png", b"\x89PNG\r\n"),
    ]);
    let graph = import_docx_bytes("image.docx", &zip).expect("import docx");
    assert_eq!(graph.assets.len(), 1);
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("![Logo](media/logo.png)"));
}

const DECIMAL_NUMBERING_XML: &[u8] = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
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

#[test]
fn docx_imports_ordered_lists_from_numbering_xml() {
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
    let zip = stored_zip(&[
        ("word/document.xml", document_xml),
        ("word/numbering.xml", DECIMAL_NUMBERING_XML),
    ]);
    let graph = import_docx_bytes("ol.docx", &zip).expect("import docx");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("1. One"));
    assert!(markdown.contains("2. Two"));
    assert!(
        !graph
            .coverage
            .loss
            .iter()
            .any(|loss| loss.code == "reader.docx.numbering_unresolved")
    );
}

#[test]
fn docx_imports_numbered_paragraphs_as_list() {
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
    let zip = docx_zip(document_xml);
    let graph = import_docx_bytes("list.docx", &zip).expect("import docx");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("- One"));
    assert!(markdown.contains("- Two"));
}

#[test]
fn docx_imports_unresolved_footnote_references_with_loss() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>See</w:t></w:r>
      <w:r><w:footnoteReference w:id="1"/></w:r>
      <w:r><w:t> for details.</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
    let zip = docx_zip(document_xml);
    let graph = import_docx_bytes("sample.docx", &zip).expect("import docx");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("See[^1] for details."));
    assert!(
        graph
            .coverage
            .loss
            .iter()
            .any(|loss| loss.code == "reader.docx.footnote_body")
    );
}

#[test]
fn docx_imports_footnote_bodies_from_footnotes_xml() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>See</w:t></w:r>
      <w:r><w:footnoteReference w:id="1"/></w:r>
      <w:r><w:t> for details.</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
    let footnotes_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:footnote w:id="1">
    <w:p><w:r><w:t>Footnote body.</w:t></w:r></w:p>
  </w:footnote>
</w:footnotes>"#;
    let zip = docx_zip_with_footnotes(document_xml, footnotes_xml);
    let graph = import_docx_bytes("sample.docx", &zip).expect("import docx");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("See[^1] for details."));
    assert!(markdown.contains("[^1]: Footnote body."));
    assert!(
        !graph
            .coverage
            .loss
            .iter()
            .any(|loss| loss.code == "reader.docx.footnote_body")
    );
}

#[test]
fn docx_imports_tables_as_gfm_markdown() {
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
    let zip = docx_zip(document_xml);
    let graph = import_docx_bytes("table.docx", &zip).expect("import docx");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("| H1 | H2 |"));
    assert!(markdown.contains("| --- | --- |"));
    assert!(markdown.contains("| A | B |"));
}

#[test]
fn docx_projects_inserted_text_and_reports_deleted_text() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>Keep </w:t></w:r><w:ins w:id="1"><w:r><w:t>inserted</w:t></w:r></w:ins><w:del w:id="2"><w:r><w:delText>deleted</w:delText></w:r></w:del></w:p></w:body>
</w:document>"#;
    let graph = import_docx_bytes("tracked.docx", &docx_zip(document_xml)).expect("import docx");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("Keep inserted"), "markdown was: {markdown}");
    assert!(!markdown.contains("deleted"));
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "reader.docx.tracked_changes"));
}

#[test]
fn docx_import_reports_page_and_column_break_loss() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>Before</w:t><w:br w:type="page"/><w:t>After</w:t><w:br w:type="column"/></w:r></w:p></w:body>
</w:document>"#;
    let graph = import_docx_bytes("page-breaks.docx", &docx_zip(document_xml)).expect("import page breaks");
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "reader.docx.page_breaks"));
    let markdown = export_markdown(&graph).expect("export page breaks");
    assert!(markdown.contains("Before\nAfter"), "markdown was: {markdown}");
}

#[test]
fn docx_import_reports_paragraph_and_section_layout_breaks() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:pPr><w:pageBreakBefore/></w:pPr><w:r><w:t>Before</w:t><w:lastRenderedPageBreak/><w:t>After</w:t></w:r></w:p>
    <w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/><w:cols w:num="2"/></w:sectPr>
  </w:body>
</w:document>"#;
    let graph = import_docx_bytes("layout-breaks.docx", &docx_zip(document_xml)).expect("import layout breaks");
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "reader.docx.page_breaks"));
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "reader.docx.section_layout"));
}
