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

#[test]
fn subgraph_blocks_become_clusters_with_membership() {
    let source = "flowchart TD\n  subgraph One\n    A[Start] --> B[Work]\n  end\n  B --> D[End]\n";
    let spec = mermaid_to_spec(source).expect("subgraphs should lower");
    assert_eq!(spec.clusters.len(), 1);

    let cluster = &spec.clusters[0];
    assert_eq!(cluster.id, "cluster-1");
    assert_eq!(cluster.label.as_deref(), Some("One"));
    assert_eq!(cluster.members, vec!["A".to_string(), "B".to_string()]);

    let node = |id: &str| spec.nodes.iter().find(|node| node.id == id).expect("node");
    assert_eq!(node("A").cluster.as_deref(), Some("cluster-1"));
    assert!(node("D").cluster.is_none());

    let lowered = mermaid_to_scene(source).expect("subgraphs should render");
    assert!(lowered.diagnostics.is_empty(), "a single cluster must not report loss: {:?}", lowered.diagnostics);
    let svg = lowered.scene.to_svg().expect("scene should serialize");
    assert!(svg.contains(">One</text>"), "{svg}");
}

#[test]
fn nested_subgraphs_flatten_with_a_diagnostic() {
    let source = "flowchart TD\n  subgraph One\n    A[Start] --> B[Work]\n    subgraph two[Inner]\n      C[Deep]\n    end\n  end\n";
    let spec = mermaid_to_spec(source).expect("nested subgraphs should lower");
    assert_eq!(spec.clusters.len(), 2);

    let outer = spec.clusters.iter().find(|cluster| cluster.id == "cluster-1").expect("outer cluster");
    assert_eq!(outer.members, vec!["A".to_string(), "B".to_string(), "C".to_string()]);
    let inner = spec.clusters.iter().find(|cluster| cluster.id == "two").expect("inner cluster");
    assert_eq!(inner.label.as_deref(), Some("Inner"));
    assert_eq!(inner.members, vec!["C".to_string()]);

    let node = |id: &str| spec.nodes.iter().find(|node| node.id == id).expect("node");
    assert_eq!(node("A").cluster.as_deref(), Some("cluster-1"));
    assert_eq!(node("C").cluster.as_deref(), Some("two"));

    let lowered = mermaid_to_scene(source).expect("nested subgraphs should render");
    assert_eq!(lowered.diagnostics.len(), 1, "flattening a nested subgraph loses containment: {:?}", lowered.diagnostics);
    assert!(lowered.diagnostics[0].message().fallback().unwrap_or_default().contains("multiple clusters"));
    let svg = lowered.scene.to_svg().expect("scene should serialize");
    assert!(svg.contains(">One</text>"), "{svg}");
    assert!(svg.contains(">Inner</text>"), "{svg}");
}
