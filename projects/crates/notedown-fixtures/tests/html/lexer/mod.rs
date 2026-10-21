use oak_html::{HtmlLanguage, HtmlLexer};
use oak_testing::lexing::LexerTester;
use std::{path::Path, time::Duration};

#[test]
fn html_lexer_matches_fixtures() -> Result<(), oak_core::OakError> {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = here.join("tests/html/lexer");
    let language = HtmlLanguage::default();
    let lexer = HtmlLexer::new(&language);
    let test_runner = LexerTester::new(fixtures).with_extension("html").with_timeout(Duration::from_secs(5));
    test_runner.run_tests(&lexer)
}
