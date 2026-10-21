use oak_html::{HtmlLanguage, HtmlParser};
use oak_testing::parsing::ParserTester;
use std::{path::Path, time::Duration};

#[test]
fn html_parser_matches_fixtures() -> Result<(), oak_core::OakError> {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = here.join("tests/html/parser");
    let language = HtmlLanguage::default();
    let parser = HtmlParser::new(&language);
    let test_runner = ParserTester::new(fixtures).with_extension("html").with_timeout(Duration::from_secs(5));
    test_runner.run_tests(&parser)
}
