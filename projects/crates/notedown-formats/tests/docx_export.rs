use notedown_formats::export::docx::export_docx_bytes;
use notedown_formats::export::markdown::export_markdown;
use notedown_formats::import::docx::import_docx_bytes;
use notedown_ir::{Asset, AssetId, AssetKind, Block, DocumentGraph, DocumentId, Inline, ListItem, NodeId, SemanticStatus};

fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend_from_slice(&13u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 2, 0, 0, 0]);
    bytes.extend_from_slice(&crc32(&bytes[12..]).to_be_bytes());
    bytes
}

fn graph_with_image(bytes: Vec<u8>, target: &str) -> DocumentGraph {
    let mut graph = DocumentGraph::new(DocumentId(50));
    graph.push_asset(Asset { id: AssetId(1), kind: AssetKind::Image, content_identity: None, source: Some(target.into()), media_type: None, status: SemanticStatus::Resolved, bytes: Some(bytes) });
    graph.push_block(Block::Paragraph { content: vec![Inline::Styled { style: "image".into(), children: vec![Inline::Text { text: "Image".into() }, Inline::Text { text: target.into() }] }] });
    graph
}

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
fn docx_export_round_trips_underline_and_strike_styles() {
    let mut graph = DocumentGraph::new(DocumentId(54));
    graph.push_block(Block::Paragraph {
        content: vec![
            Inline::Styled { style: "underline".into(), children: vec![Inline::Text { text: "under".into() }] },
            Inline::Styled { style: "strike".into(), children: vec![Inline::Styled { style: "bold".into(), children: vec![Inline::Text { text: " crossed".into() }] }] },
        ],
    });
    let exported = export_docx_bytes(&graph).expect("export styles");
    let xml = String::from_utf8_lossy(&exported);
    assert!(xml.contains("<w:u w:val=\"single\"/>"));
    assert!(xml.contains("<w:strike/>"));
    let reopened = import_docx_bytes("styled-round.docx", &exported).expect("reopen DOCX");
    let Block::Paragraph { content } = &reopened.blocks[0].block else { panic!("paragraph expected") };
    assert!(matches!(&content[0], Inline::Styled { style, .. } if style == "underline"));
    assert!(matches!(&content[1], Inline::Styled { style, children } if style == "bold" && matches!(&children[0], Inline::Styled { style, .. } if style == "strike")));
}

#[test]
fn docx_export_round_trips_vertical_alignment_styles() {
    let mut graph = DocumentGraph::new(DocumentId(56));
    graph.push_block(Block::Paragraph {
        content: vec![
            Inline::Styled { style: "superscript".into(), children: vec![Inline::Text { text: "2".into() }] },
            Inline::Styled { style: "subscript".into(), children: vec![Inline::Text { text: "i".into() }] },
        ],
    });
    let exported = export_docx_bytes(&graph).expect("export vertical alignment");
    let xml = String::from_utf8_lossy(&exported);
    assert!(xml.contains("w:vertAlign w:val=\"superscript\""));
    assert!(xml.contains("w:vertAlign w:val=\"subscript\""));
    let reopened = import_docx_bytes("vertical-round.docx", &exported).expect("reopen vertical alignment");
    let Block::Paragraph { content } = &reopened.blocks[0].block else { panic!("paragraph expected") };
    assert!(matches!(&content[0], Inline::Styled { style, .. } if style == "superscript"));
    assert!(matches!(&content[1], Inline::Styled { style, .. } if style == "subscript"));
}

#[test]
fn docx_export_round_trips_color_style() {
    let mut graph = DocumentGraph::new(DocumentId(57));
    graph.push_block(Block::Paragraph { content: vec![Inline::Styled { style: "color:ff0000".into(), children: vec![Inline::Text { text: "red".into() }] }] });
    let exported = export_docx_bytes(&graph).expect("export color");
    assert!(String::from_utf8_lossy(&exported).contains("<w:color w:val=\"ff0000\"/>"));
    let reopened = import_docx_bytes("color-round.docx", &exported).expect("reopen color");
    let Block::Paragraph { content } = &reopened.blocks[0].block else { panic!("paragraph expected") };
    assert!(matches!(&content[0], Inline::Styled { style, .. } if style == "color:ff0000"));
}

#[test]
fn docx_export_round_trips_valued_character_styles() {
    let mut graph = DocumentGraph::new(DocumentId(58));
    let styled = Inline::Styled {
        style: "font-size:28".into(),
        children: vec![Inline::Styled {
            style: "font-family:Aptos".into(),
            children: vec![Inline::Styled {
                style: "highlight:yellow".into(),
                children: vec![Inline::Text { text: "styled".into() }],
            }],
        }],
    };
    graph.push_block(Block::Paragraph { content: vec![styled] });
    let exported = export_docx_bytes(&graph).expect("export valued styles");
    let xml = String::from_utf8_lossy(&exported);
    assert!(xml.contains("<w:sz w:val=\"28\"/>"));
    assert!(xml.contains("<w:rFonts w:ascii=\"Aptos\" w:hAnsi=\"Aptos\"/>"));
    assert!(xml.contains("<w:highlight w:val=\"yellow\"/>"));
    let reopened = import_docx_bytes("valued-character-styles-round.docx", &exported).expect("reopen valued styles");
    let Block::Paragraph { content } = &reopened.blocks[0].block else { panic!("paragraph expected") };
    let rendered = format!("{:?}", content);
    assert!(rendered.contains("font-size:28"), "{rendered}");
    assert!(rendered.contains("font-family:Aptos"), "{rendered}");
    assert!(rendered.contains("highlight:yellow"), "{rendered}");
}

#[test]
fn docx_inline_image_order_survives_ir_edit_and_regeneration() {
    let image = r#"<w:drawing><wp:inline><wp:docPr descr="Logo"/><a:graphic><a:graphicData><a:blip r:embed="rId1"/></a:graphicData></a:graphic></wp:inline></w:drawing>"#;
    let document = format!(r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><w:body><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Before</w:t>{image}<w:t>After</w:t></w:r><w:del><w:r>{image}</w:r></w:del><w:ins><w:r><w:t>Inserted</w:t>{image}<w:t>Tail</w:t></w:r></w:ins></w:p></w:body></w:document>"#);
    let rels = br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/logo.png"/></Relationships>"#;
    let png = png_header(1, 1);
    let zip = stored_zip(&[("word/document.xml", document.as_bytes()), ("word/_rels/document.xml.rels", rels), ("word/media/logo.png", &png)]);
    let mut graph = import_docx_bytes("ordered.docx", &zip).expect("import inline order");
    let before = export_markdown(&graph).expect("render inline order");
    assert!(before.contains("**Before**![Logo](media/logo.png)**After**Inserted![Logo](media/logo.png)Tail"), "{before}");
    assert_eq!(before.matches("![Logo]").count(), 2);
    let Block::Paragraph { content } = &mut graph.blocks[0].block else { panic!("paragraph expected") };
    content.push(Inline::Text { text: "Edited".into() });
    let bytes = export_docx_bytes(&graph).expect("generate edited DOCX");
    let reopened = import_docx_bytes("edited.docx", &bytes).expect("reopen edited DOCX");
    let after = export_markdown(&reopened).expect("render edited DOCX");
    assert!(after.contains("**Before**![Logo](media/logo.png)**After**Inserted![Logo](media/logo.png)TailEdited"), "{after}");
}

#[test]
fn docx_imports_core_metadata_into_ir() {
    let document = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Body</w:t></w:r></w:p></w:body></w:document>"#;
    let core = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Document title</dc:title><dc:creator>Author</dc:creator><dc:language>zh-CN</dc:language><cp:keywords>one, two, three</cp:keywords></cp:coreProperties>"#;
    let zip = stored_zip(&[("word/document.xml", document), ("docProps/core.xml", core)]);
    let graph = import_docx_bytes("metadata.docx", &zip).expect("import docx");
    assert_eq!(graph.metadata.title.as_deref(), Some("Document title"));
    assert_eq!(graph.metadata.language.as_deref(), Some("zh-CN"));
    assert_eq!(graph.metadata.authors, vec!["Author"]);
    assert_eq!(graph.metadata.tags, vec!["one", "two", "three"]);
    let exported = export_docx_bytes(&graph).expect("export docx");
    let round = import_docx_bytes("metadata-round.docx", &exported).expect("re-import docx");
    assert_eq!(round.metadata.title.as_deref(), Some("Document title"));
    assert_eq!(round.metadata.authors, vec!["Author"]);
    assert_eq!(round.metadata.tags, vec!["one", "two", "three"]);
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
    assert!(payload.contains("w:ilvl=\"1\"><w:numFmt"));
    let round = import_docx_bytes("round.docx", &exported).expect("re-import docx");
    let markdown = export_markdown(&round).expect("export markdown");
    assert!(markdown.contains("- One"));
    assert!(markdown.contains("- Two"));
}

#[test]
fn docx_import_and_export_preserve_nested_list_levels() {
    let document_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Parent</w:t></w:r></w:p>
    <w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Child</w:t></w:r></w:p>
    <w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Sibling</w:t></w:r></w:p>
  </w:body>
</w:document>"#;
    let zip = stored_zip(&[("word/document.xml", document_xml)]);
    let graph = import_docx_bytes("nested.docx", &zip).expect("import docx");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("- Parent\n  - Child\n- Sibling"), "markdown was: {markdown}");
    let exported = export_docx_bytes(&graph).expect("export docx");
    let round = import_docx_bytes("round.docx", &exported).expect("re-import docx");
    let round_markdown = export_markdown(&round).expect("export markdown");
    assert!(round_markdown.contains("- Parent\n  - Child\n- Sibling"), "round markdown was: {round_markdown}");
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
fn docx_export_round_trips_footnotes() {
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
    let rels_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes"
    Target="footnotes.xml"/>
</Relationships>"#;
    let zip = stored_zip(&[
        ("word/document.xml", document_xml),
        ("word/_rels/document.xml.rels", rels_xml),
        ("word/footnotes.xml", footnotes_xml),
    ]);
    let graph = import_docx_bytes("footnotes.docx", &zip).expect("import docx");
    let exported = export_docx_bytes(&graph).expect("export docx");
    let payload = String::from_utf8_lossy(&exported);
    assert!(payload.contains("word/footnotes.xml"));
    let round = import_docx_bytes("round.docx", &exported).expect("re-import docx");
    let markdown = export_markdown(&round).expect("export markdown");
    assert!(markdown.contains("See[^1] for details."));
    assert!(markdown.contains("[^1]: Footnote body."));
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
    let image_bytes = png_header(640, 480);
    let zip = stored_zip(&[
        ("word/document.xml", document_xml),
        ("word/_rels/document.xml.rels", rels_xml),
        ("word/media/logo.png", image_bytes.as_slice()),
    ]);
    let graph = import_docx_bytes("image.docx", &zip).expect("import docx");
    let exported = export_docx_bytes(&graph).expect("export docx");
    let payload = String::from_utf8_lossy(&exported);
    assert!(payload.contains("word/media/logo.png"));
    let round = import_docx_bytes("round.docx", &exported).expect("re-import docx");
    let markdown = export_markdown(&round).expect("export markdown");
    assert!(markdown.contains("![Logo](media/logo.png)"));
}

#[test]
fn docx_images_use_consistent_emu_extents_without_upscaling() {
    for (width, height, extent) in [(640, 480, (4_000_000, 3_000_000)), (100, 50, (952_500, 476_250)), (50, 100, (476_250, 952_500))] {
        let graph = graph_with_image(png_header(width, height), "media/size.png");
        let exported = export_docx_bytes(&graph).expect("export sized PNG");
        let xml = String::from_utf8_lossy(&exported);
        let (emu_width, emu_height) = extent;
        assert!(xml.contains(&format!("<wp:extent cx=\"{emu_width}\" cy=\"{emu_height}\"/>")));
        assert!(xml.contains(&format!("<a:ext cx=\"{emu_width}\" cy=\"{emu_height}\"/>")));
        let reopened = import_docx_bytes("sized.docx", &exported).expect("reopen sized PNG");
        assert_eq!(reopened.assets[0].bytes, Some(png_header(width, height)));
    }
}

#[test]
fn docx_jpeg_frame_dimensions_determine_image_extent() {
    let bytes = vec![0xff, 0xd8, 0xff, 0xc0, 0, 11, 8, 0, 100, 0, 50, 1, 1, 0x11, 0, 0xff, 0xd9];
    let exported = export_docx_bytes(&graph_with_image(bytes, "media/size.jpg")).expect("export JPEG frame");
    assert!(String::from_utf8_lossy(&exported).contains("<wp:extent cx=\"476250\" cy=\"952500\"/>"));
}

#[test]
fn docx_rejects_truncated_or_zero_sized_raster_headers() {
    for bytes in [b"\x89PNG\r\n".to_vec(), png_header(0, 100), vec![0xff, 0xd8, 0xff, 0xd9]] {
        assert!(export_docx_bytes(&graph_with_image(bytes, "media/broken.png")).is_err());
    }
}

#[test]
fn docx_emits_section_and_list_children_in_semantic_order() {
    let mut graph = DocumentGraph::new(DocumentId(51));
    let paragraph = graph.push_block(Block::Paragraph { content: vec![Inline::Text { text: "child paragraph".into() }] });
    let list = graph.push_block(Block::List { ordered: true, items: vec![ListItem { content: vec![Inline::Text { text: "list item".into() }], children: vec![paragraph] }] });
    graph.push_block(Block::Section { level: 1, title: vec![Inline::Text { text: "section heading".into() }], children: vec![list] });
    let exported = export_docx_bytes(&graph).expect("export section children");
    let xml = String::from_utf8_lossy(&exported);
    assert!(xml.contains("word/numbering.xml"));
    let reopened = import_docx_bytes("section.docx", &exported).expect("reopen section children");
    let markdown = export_markdown(&reopened).expect("render section children");
    assert_eq!(markdown.matches("child paragraph").count(), 1);
    assert_eq!(markdown.matches("list item").count(), 1);
    assert!(markdown.find("section heading").unwrap() < markdown.find("list item").unwrap());
    assert!(markdown.find("list item").unwrap() < markdown.find("child paragraph").unwrap());
}

#[test]
fn docx_rejects_cyclic_or_missing_containment_before_rendering() {
    let mut graph = DocumentGraph::new(DocumentId(52));
    graph.push_block_with_id(NodeId(1), Block::Section { level: 1, title: Vec::new(), children: vec![NodeId(1)] });
    assert!(export_docx_bytes(&graph).is_err());
    graph.blocks.clear();
    graph.push_block_with_id(NodeId(1), Block::Section { level: 1, title: Vec::new(), children: vec![NodeId(2)] });
    assert!(export_docx_bytes(&graph).is_err());
}

#[test]
fn docx_rejects_lists_deeper_than_numbering_definitions() {
    let mut graph = DocumentGraph::new(DocumentId(53));
    let mut children = Vec::new();
    for depth in 0..10 {
        let list = graph.push_block(Block::List { ordered: false, items: vec![ListItem { content: vec![Inline::Text { text: format!("depth {depth}") }], children }] });
        children = vec![list];
    }
    assert!(graph.validate().is_valid());
    assert!(export_docx_bytes(&graph).is_err());
}
