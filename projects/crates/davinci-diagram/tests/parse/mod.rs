use davinci_diagram::{mermaid_to_scene, parse_mermaid};
use davinci_scene::codes;
use davinci_types::DiagnosticSeverity;

#[test]
fn flowchart_source_reports_ast_unavailable() {
    let error = parse_mermaid("flowchart TD\n  A-->B").expect_err("pinned oak-mermaid builder produces an empty AST");
    assert_eq!(error.code(), codes::AST_UNAVAILABLE);
    assert!(error.message().contains("oak-mermaid"), "{}", error.message());
    assert!(error.message().contains("no diagrams"), "{}", error.message());
    assert_eq!(error.diagnostic().severity(), DiagnosticSeverity::Error);
    assert_eq!(error.diagnostic().origin().component(), "diagram");
}

#[test]
fn empty_source_reports_ast_unavailable() {
    let error = parse_mermaid("").expect_err("empty AST is rejected");
    assert_eq!(error.code(), codes::AST_UNAVAILABLE);
}

#[test]
fn source_pipeline_does_not_fake_a_scene() {
    let error = mermaid_to_scene("sequenceDiagram\n  A->>B: hello").expect_err("source pipeline cannot lower without an AST");
    assert_eq!(error.code(), codes::AST_UNAVAILABLE);
}
