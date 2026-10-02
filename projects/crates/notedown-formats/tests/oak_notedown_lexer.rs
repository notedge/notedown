use oak_notedown::{NoteLanguage as NotedownLanguage, NoteLexer as NotedownLexer};
use oak_testing::lexing::LexerTester;
use std::{path::Path, time::Duration};

#[test]
fn oak_notedown_lexer_matches_fixtures() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oak_notedown_lexer");
    let language = NotedownLanguage::default();
    let lexer = NotedownLexer::new(&language);
    let test_runner = LexerTester::new(fixtures)
        .with_extension("nd")
        .with_timeout(Duration::from_secs(5));
    test_runner
        .run_tests::<NotedownLanguage, _>(&lexer)
        .expect("oak-notedown lexer fixtures");
}
