use std::io::{Cursor, Write};
use std::path::Path;

use acorn_doc::extract_text_from_bytes;
use notedown_formats::export::markdown_project::export_markdown_project;
use notedown_formats::import::doc::import_doc_bytes;
use notedown_formats::import::markdown::import_markdown_bytes;
use notedown_ir::{Block, Inline};

pub(crate) fn minimal_legacy_doc_bytes(text: &str) -> Vec<u8> {
    use cfb::CompoundFile as OracleCompoundFile;

    let mut word_document = vec![0u8; 32];
    word_document[0..2].copy_from_slice(&[0xec, 0xa5]);
    let text_bytes = text.as_bytes();
    let fc_min = 32u32;
    let fc_mac = fc_min + text_bytes.len() as u32;
    word_document.extend_from_slice(text_bytes);
    word_document[24..28].copy_from_slice(&fc_min.to_le_bytes());
    word_document[28..32].copy_from_slice(&fc_mac.to_le_bytes());

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut compound = OracleCompoundFile::create(&mut buffer).expect("create compound file");
        {
            let mut stream = compound
                .create_stream("/WordDocument")
                .expect("WordDocument stream");
            stream.write_all(&word_document).expect("write WordDocument");
        }
        {
            let mut stream = compound.create_stream("/0Table").expect("0Table stream");
            stream.write_all(&[]).expect("write 0Table");
        }
    }
    buffer.into_inner()
}

#[test]
fn acorn_doc_extracts_legacy_plain_fixture() {
    let extracted = extract_text_from_bytes(notedown_fixtures::acorn::doc::LEGACY_PLAIN_DOC)
        .expect("extract fixture");
    assert!(extracted.text.contains("Hello legacy DOC"));
    assert!(extracted.text.contains("Second paragraph"));
    assert!(!extracted.complex);
    assert!(!extracted.piece_table_used);
}

#[test]
fn doc_exports_markdown_project_with_reopened_paragraphs() {
    let graph = import_doc_bytes("sample.doc", notedown_fixtures::acorn::doc::LEGACY_PLAIN_DOC)
        .expect("import doc");
    let project = export_markdown_project(&graph).expect("export markdown project");

    assert!(project.chapters.is_empty());
    assert!(project.index_markdown.contains("Hello legacy DOC"));
    assert!(project.index_markdown.contains("Second paragraph"));
    assert!(!graph.coverage.loss.is_empty());

    let reopened = import_markdown_bytes("index.md", &project.index_markdown).expect("oak markdown reopen");
    assert!(reopened.validate().is_valid());
    assert!(reopened.blocks.iter().any(|node| matches!(
        &node.block,
        Block::Paragraph { content }
            if content.iter().any(|inline| matches!(inline, Inline::Text { text } if text == "Hello legacy DOC"))
    )));
    assert!(reopened.blocks.iter().any(|node| matches!(
        &node.block,
        Block::Paragraph { content }
            if content.iter().any(|inline| matches!(inline, Inline::Text { text } if text == "Second paragraph"))
    )));
}

#[test]
#[ignore = "run manually to refresh tests/acorn/doc/legacy_plain.doc"]
fn write_legacy_plain_fixture() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/acorn/doc/legacy_plain.doc");
    if let Some(parent) = fixture_path.parent() {
        std::fs::create_dir_all(parent).expect("create fixtures dir");
    }
    let bytes = minimal_legacy_doc_bytes("Hello legacy DOC\rSecond paragraph");
    std::fs::write(&fixture_path, bytes).expect("write fixture");
}
