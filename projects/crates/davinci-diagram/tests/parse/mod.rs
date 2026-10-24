use davinci_diagram::{mermaid_to_scene, parse_mermaid};
use davinci_scene::codes;
use davinci_types::DiagnosticSeverity;
use oak_mermaid::ast::{FlowElement, MermaidDiagram};

#[test]
fn flowchart_source_builds_a_diagram() {
    let root = parse_mermaid("flowchart TD\n  A-->B").expect("flowchart source should build an AST");
    assert_eq!(root.diagrams.len(), 1);

    let MermaidDiagram::Flowchart(flowchart) = &root.diagrams[0]
    else {
        panic!("expected a flowchart diagram, got {:?}", root.diagrams[0]);
    };
    assert_eq!(flowchart.orientation, "TD");

    let ids: Vec<&str> = flowchart
        .elements
        .iter()
        .filter_map(|element| match element {
            FlowElement::Node { id, .. } => Some(id.as_str()),
            FlowElement::Edge { .. } => None,
        })
        .collect();
    assert_eq!(ids, vec!["A", "B"]);

    let edges: Vec<(&str, &str)> = flowchart
        .elements
        .iter()
        .filter_map(|element| match element {
            FlowElement::Edge { from, to, .. } => Some((from.as_str(), to.as_str())),
            FlowElement::Node { .. } => None,
        })
        .collect();
    assert_eq!(edges, vec![("A", "B")]);
}

#[test]
fn empty_source_reports_ast_unavailable() {
    let error = parse_mermaid("").expect_err("empty AST is rejected");
    assert_eq!(error.code(), codes::AST_UNAVAILABLE);
    assert_eq!(error.diagnostic().severity(), DiagnosticSeverity::Error);
    assert_eq!(error.diagnostic().origin().component(), "diagram");
}

#[test]
fn unsupported_diagram_family_reports_ast_unavailable() {
    let error = mermaid_to_scene("sequenceDiagram\n  A->>B: hello").expect_err("sequence diagrams are not lowered");
    assert_eq!(error.code(), codes::AST_UNAVAILABLE);
    assert!(error.message().contains("no diagrams"), "{}", error.message());
}
