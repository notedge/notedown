//! Oak `oak-xml` lexer golden tests (migrated from upstream oak.rs).

use oak_testing::lexing::LexerTester;
use oak_xml::{XmlLanguage, XmlLexer};
use std::{path::Path, time::Duration};

#[test]
#[ignore = "manual baseline regeneration helper; do not run in CI"]
fn generate_baseline() {
    use oak_core::{Lexer, ParseSession, SourceText, TokenType, source::Source};
    use serde_json::json;
    use std::fs;

    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture_dir = here.join("tests/fixtures/oak_xml_lexer");
    let source_path = fixture_dir.join("basic.xml");
    let source_text = fs::read_to_string(source_path)
        .expect("Failed to read source")
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let source = SourceText::new(source_text);
    let language = XmlLanguage::default();
    let lexer = XmlLexer::new(&language);
    let mut cache = ParseSession::default();
    let result = lexer.lex(&source, &[], &mut cache);

    let tokens = result.result.expect("Lexing failed");
    let token_data: Vec<_> = tokens
        .iter()
        .filter(|t| !t.kind.is_ignored())
        .map(|t| {
            let text = source.get_text_in(t.span.clone()).to_string();
            json!({
                "kind": format!("{:?}", t.kind),
                "text": text,
                "start": t.span.start,
                "end": t.span.end
            })
        })
        .collect();

    let output = json!({
        "success": true,
        "count": token_data.len(),
        "tokens": token_data,
        "errors": []
    });

    let output_path = fixture_dir.join("basic.xml.lexed.json");
    fs::write(output_path, serde_json::to_string_pretty(&output).unwrap()).expect("Failed to write baseline");
}

#[test]
fn oak_xml_lexer_matches_fixtures() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oak_xml_lexer");
    let language = XmlLanguage::default();
    let lexer = XmlLexer::new(&language);
    let test_runner = LexerTester::new(fixtures)
        .with_extension("xml")
        .with_timeout(Duration::from_secs(5));
    test_runner
        .run_tests::<XmlLanguage, _>(&lexer)
        .expect("oak-xml lexer fixtures");
}
