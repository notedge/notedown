use oak_org_mode::{OrgModeLanguage, OrgModeLexer};
use oak_testing::lexing::LexerTester;
use std::{path::Path, time::Duration};

#[test]
fn org_mode_lexer_matches_fixtures() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/org-mode/lexer");
    let language = OrgModeLanguage::default();
    let lexer = OrgModeLexer::new(&language);
    let test_runner = LexerTester::new(fixtures).with_extension("org").with_timeout(Duration::from_secs(5));
    test_runner.run_tests::<OrgModeLanguage, _>(&lexer).expect("oak-org-mode lexer fixtures");
}
