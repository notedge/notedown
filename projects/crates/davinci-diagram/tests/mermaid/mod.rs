use davinci_diagram::{
    parse::{mermaid_to_scene, mermaid_to_spec, parse_mermaid},
    spec::{Direction, NodeShape},
};
use davinci_scene::codes;

#[test]
fn flowchart_source_lowers_end_to_end() {
    let source = "flowchart TD\n  A[Start] --> B{Ready?}\n  B -->|yes| C[Stop]\n  B -->|no| D[Retry]\n";
    let lowered = mermaid_to_scene(source).expect("flowchart should lower");

    assert_eq!(lowered.plan.nodes.len(), 4);
    assert_eq!(lowered.plan.edges.len(), 3);
    assert_eq!(lowered.plan.node("A").expect("A").label, "Start");
    assert_eq!(lowered.plan.node("B").expect("B").label, "Ready?");
    assert_eq!(lowered.plan.node("B").expect("B").shape, NodeShape::Rhombus);
    assert_eq!(lowered.plan.node("C").expect("C").label, "Stop");
    assert!(lowered.diagnostics.is_empty(), "a clean flowchart must not report loss");

    let svg = lowered.scene.to_svg().expect("scene should serialize");
    assert!(svg.contains(">Start</text>"));
    assert!(svg.contains(">Stop</text>"));
    assert!(svg.contains(">yes</text>"));
}

#[test]
fn mermaid_spec_maps_direction_and_keeps_the_first_label() {
    let spec = mermaid_to_spec("graph LR\n  A[First] --> B\n  A[Second] --> B\n").expect("spec should lower");
    assert_eq!(spec.direction, Direction::LeftToRight);
    assert_eq!(spec.nodes.len(), 2);
    assert_eq!(spec.nodes[0].id, "A");
    assert_eq!(spec.nodes[0].label.as_deref(), Some("First"));
    assert!(spec.clusters.is_empty());
    assert!(spec.constraints.is_empty());
}

#[test]
fn empty_and_unsupported_sources_report_ast_unavailable() {
    let empty = parse_mermaid("").expect_err("empty source has no AST");
    assert_eq!(empty.code(), codes::AST_UNAVAILABLE);
    assert!(empty.message().contains("graph/flowchart"));

    let sequence = parse_mermaid("sequenceDiagram\n  Alice->>Bob: Hello\n").expect_err("sequence is unsupported");
    assert_eq!(sequence.code(), codes::AST_UNAVAILABLE);
    assert!(sequence.message().contains("graph/flowchart"));

    let lowered = mermaid_to_scene("sequenceDiagram\n  Alice->>Bob: Hello\n").expect_err("sequence is unsupported");
    assert_eq!(lowered.code(), codes::AST_UNAVAILABLE);
    assert!(lowered.message().contains("graph/flowchart"));
}

#[test]
fn flowchart_without_nodes_reports_empty_diagram() {
    let empty = mermaid_to_scene("flowchart TD\n").expect_err("empty flowchart has nothing to lay out");
    assert_eq!(empty.code(), codes::EMPTY_DIAGRAM);
}
