//! DOT frontend acceptance: the documented subset lowers, everything else is
//! rejected with a structured diagnostic that points at the offending source.

use std::ops::Range;

use davinci_diagram::{
    DiagramError, dot_to_scene, dot_to_spec,
    spec::{Cluster, Constraint, DiagramEdge, DiagramNode, Direction, EdgeStyle, Endpoint, NodeShape, Port, Side},
};
use davinci_types::{DiagnosticLocation, codes};

fn span_of<'a>(source: &'a str, error: &DiagramError) -> &'a str {
    let span = error.span().unwrap_or_else(|| panic!("`{}` carries no span", error.message()));
    let Range { start, end } = span.clone();
    assert!(end <= source.len(), "span {start}..{end} leaves a {} byte source", source.len());
    &source[start..end]
}

/// The authored bytes a layout diagnostic's primary label covers.
fn label_text<'a>(source: &'a str, diagnostic: &davinci_types::Diagnostic) -> &'a str {
    let label = diagnostic
        .primary()
        .unwrap_or_else(|| panic!("`{}` carries no label, so it names no token", diagnostic.message().fallback().unwrap_or_default()));
    let DiagnosticLocation::Text { range, .. } = label.location()
    else {
        panic!("the primary label must be a text range");
    };
    let start = range.start() as usize;
    let end = range.end() as usize;
    assert!(end <= source.len(), "label {start}..{end} leaves a {} byte source", source.len());
    &source[start..end]
}

fn node(id: &str) -> DiagramNode {
    DiagramNode { id: id.to_string(), label: None, shape: NodeShape::Box, ports: Vec::new(), cluster: None }
}

fn edge(from: &str, to: &str, label: Option<&str>, style: EdgeStyle) -> DiagramEdge {
    DiagramEdge {
        from: Endpoint { node: from.to_string(), port: None },
        to: Endpoint { node: to.to_string(), port: None },
        label: label.map(str::to_string),
        style,
    }
}

#[test]
fn digraph_rankdir_nodes_edges_and_labels_lower_to_one_spec() {
    let source = r#"digraph pipeline {
  rankdir=LR;
  node [shape=box];
  A [label="Start", shape=circle];
  B [label="Work\nhard"];
  A -> B [label="go"];
}"#;
    let spec = dot_to_spec(source).expect("the subset should lower");

    assert_eq!(spec.direction, Direction::LeftToRight);
    assert_eq!(
        spec.nodes,
        vec![
            DiagramNode { id: "A".to_string(), label: Some("Start".to_string()), shape: NodeShape::Circle, ports: Vec::new(), cluster: None },
            DiagramNode { id: "B".to_string(), label: Some("Work\nhard".to_string()), shape: NodeShape::Box, ports: Vec::new(), cluster: None },
        ]
    );
    assert_eq!(spec.edges, vec![edge("A", "B", Some("go"), EdgeStyle::Arrow)]);
    assert!(spec.clusters.is_empty());
    assert!(spec.constraints.is_empty());
}

#[test]
fn undirected_graph_lowers_plain_lines() {
    let spec = dot_to_spec("graph g {\n  A -- B;\n}").expect("the subset should lower");
    assert_eq!(spec.direction, Direction::TopDown);
    assert_eq!(spec.edges, vec![edge("A", "B", None, EdgeStyle::Line)]);
    assert_eq!(spec.nodes, vec![node("A"), node("B")]);
}

#[test]
fn compass_ports_declare_boundary_anchors_on_both_endpoints() {
    let spec = dot_to_spec("digraph {\n  A:n -> B:s;\n}").expect("the subset should lower");
    assert_eq!(spec.nodes[0].ports, vec![Port { id: "n".to_string(), side: Side::Top, offset: 0.5 }]);
    assert_eq!(spec.nodes[1].ports, vec![Port { id: "s".to_string(), side: Side::Bottom, offset: 0.5 }]);
    assert_eq!(spec.edges[0].from.port.as_deref(), Some("n"));
    assert_eq!(spec.edges[0].to.port.as_deref(), Some("s"));

    let lowered = dot_to_scene("digraph {\n  A:n -> B:s;\n}").expect("the scene should lower");
    assert!(lowered.diagnostics.iter().all(|record| record.code().as_str() != codes::APPROXIMATED), "{:?}", lowered.diagnostics);
}

#[test]
fn corner_compass_is_anchored_and_reported_as_an_approximation() {
    let source = "digraph {\n  A:ne -> B;\n}";
    let spec = dot_to_spec(source).expect("the subset should lower");
    assert_eq!(spec.nodes[0].ports, vec![Port { id: "ne".to_string(), side: Side::Right, offset: 0.0 }]);

    let lowered = dot_to_scene(source).expect("the scene should lower");
    let approximation = lowered
        .diagnostics
        .iter()
        .find(|record| record.code().as_str() == codes::APPROXIMATED)
        .expect("a corner compass has no exact scene anchor");
    let label = approximation.primary().expect("approximations point at the source");
    assert_eq!(label.role(), davinci_types::LabelRole::Primary);
    assert!(matches!(label.location(), davinci_types::DiagnosticLocation::Text { .. }), "the label must be a text range");
}

#[test]
fn one_level_cluster_and_rank_same_keep_membership_and_ordering() {
    let source = r#"digraph {
  subgraph cluster_0 {
    label="Group";
    A -> B;
  }
  subgraph rhs {
    rank=same;
    C;
    D;
  }
  B -> C;
}"#;
    let spec = dot_to_spec(source).expect("the subset should lower");

    assert_eq!(
        spec.clusters,
        vec![Cluster { id: "0".to_string(), label: Some("Group".to_string()), members: vec!["A".to_string(), "B".to_string()] }]
    );
    assert_eq!(spec.nodes[0].cluster.as_deref(), Some("0"));
    assert_eq!(spec.nodes[1].cluster.as_deref(), Some("0"));
    assert_eq!(spec.nodes[2].cluster, None);
    assert_eq!(spec.constraints, vec![Constraint::SameRank(vec!["C".to_string(), "D".to_string()])]);
    assert_eq!(spec.edges, vec![edge("A", "B", None, EdgeStyle::Arrow), edge("B", "C", None, EdgeStyle::Arrow)]);
}

#[test]
fn node_defaults_apply_at_creation_time_and_explicit_attributes_win() {
    let source = "digraph {\n  node [shape=diamond];\n  A;\n  B [shape=box];\n  C;\n}";
    let spec = dot_to_spec(source).expect("the subset should lower");
    assert_eq!(spec.nodes[0].shape, NodeShape::Rhombus);
    assert_eq!(spec.nodes[1].shape, NodeShape::Box);
    assert_eq!(spec.nodes[2].shape, NodeShape::Rhombus);
}

#[test]
fn html_labels_are_rejected_with_the_attribute_span() {
    let source = "digraph {\n  A [label=<b>x</b>];\n}";
    let error = dot_to_spec(source).expect_err("HTML labels are outside the subset");
    assert_eq!(error.code(), codes::UNSUPPORTED);
    let slice = span_of(source, &error);
    assert!(slice.contains("<b>x</b>"), "the span must cover the HTML label, got `{slice}` from `{}`", error.message());
}

#[test]
fn record_shapes_and_named_ports_are_rejected_with_the_port_span() {
    let source = "digraph {\n  A [shape=record];\n}";
    let error = dot_to_spec(source).expect_err("record shapes are outside the subset");
    assert_eq!(error.code(), codes::UNSUPPORTED);
    assert!(span_of(source, &error).contains("shape=record"));

    let source = "digraph {\n  A:f0 -> B;\n}";
    let error = dot_to_spec(source).expect_err("named ports are outside the subset");
    assert_eq!(error.code(), codes::UNSUPPORTED);
    assert!(span_of(source, &error).contains("f0"), "the span must name the field port, got `{}`", span_of(source, &error));
}

#[test]
fn layout_and_interaction_attributes_are_rejected_with_their_spans() {
    for (source, expected) in [
        ("digraph {\n  A [pos=\"10,20\"];\n}", "pos=\"10,20\""),
        ("digraph {\n  splines=ortho;\n  A -> B;\n}", "splines=ortho"),
        ("digraph {\n  A [URL=\"https://example.com\"];\n}", "URL=\"https://example.com\""),
    ] {
        let error = dot_to_spec(source).expect_err("the attribute is outside the subset");
        assert_eq!(error.code(), codes::UNSUPPORTED, "for\n{source}");
        let slice = span_of(source, &error);
        assert!(slice.contains(expected), "expected the span to contain `{expected}`, got `{slice}`");
        assert!(error.labelled("dot").primary().is_some(), "rejections must label the source");
    }
}

#[test]
fn color_attributes_are_rejected_instead_of_silently_ignored() {
    for key in ["fillcolor", "color", "fontcolor", "labelfontcolor"] {
        let source = format!("digraph {{\n  A [{key}=\"#ff0000\"];\n}}");
        let error = dot_to_spec(&source).expect_err("a color attribute has no paint path on the theme-owned scene surface");
        assert_eq!(error.code(), codes::UNSUPPORTED, "`{key}` must be refused, got {}", error.message());
        let slice = span_of(&source, &error);
        assert!(slice.contains(key), "the span must name `{key}`, got `{slice}`");
    }
}

#[test]
fn multiple_graphs_are_rejected_at_the_second_graph() {
    let source = "digraph { A }\ndigraph { B }\n";
    let error = dot_to_spec(source).expect_err("a fence carries exactly one graph");
    assert_eq!(error.code(), codes::UNSUPPORTED);
    assert!(span_of(source, &error).trim_start().starts_with("digraph"), "the span must cover the second graph");
}

#[test]
fn nested_clusters_and_rankless_subgraphs_are_rejected() {
    let source = "digraph {\n  subgraph cluster_0 {\n    subgraph cluster_1 { A; }\n  }\n}";
    let error = dot_to_spec(source).expect_err("nested clusters are outside the subset");
    assert_eq!(error.code(), codes::UNSUPPORTED);
    assert!(error.message().contains("nested cluster"));

    let source = "digraph {\n  subgraph plain { A; }\n}";
    let error = dot_to_spec(source).expect_err("a non-cluster subgraph needs rank=same");
    assert_eq!(error.code(), codes::UNSUPPORTED);
    assert!(error.message().contains("rank=same"));
}

#[test]
fn strict_graphs_unknown_styles_and_mismatched_edge_operators_are_rejected() {
    let source = "strict digraph { A }\n";
    let error = dot_to_spec(source).expect_err("`strict` is outside the subset");
    assert_eq!(error.code(), codes::UNSUPPORTED);

    let source = "digraph {\n  A [style=invis];\n}";
    let error = dot_to_spec(source).expect_err("unknown style tokens are not ignored");
    assert_eq!(error.code(), codes::UNSUPPORTED);
    assert!(span_of(source, &error).contains("invis"));

    let source = "digraph {\n  A -- B;\n}";
    let error = dot_to_spec(source).expect_err("`--` is not a `digraph` operator");
    assert_eq!(error.code(), codes::UNSUPPORTED);
    assert!(span_of(source, &error).contains("--"), "the span must cover the edge statement");

    let source = "graph {\n  A -> B;\n}";
    let error = dot_to_spec(source).expect_err("`->` is not a `graph` operator");
    assert_eq!(error.code(), codes::UNSUPPORTED);
}

#[test]
fn a_cycle_reports_the_back_edge_and_its_authored_span() {
    let source = "digraph {\n  a -> b\n  b -> a\n}\n";
    let lowered = dot_to_scene(source).expect("a cycle still lays out and routes every edge");

    let cycle = lowered
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.code().as_str() == codes::APPROXIMATED && diagnostic.message().fallback().unwrap_or_default().contains("cycle")
        })
        .unwrap_or_else(|| panic!("the cycle must be reported: {:?}", lowered.diagnostics));

    assert_eq!(label_text(source, cycle), "b -> a", "the label must cover the edge the layered model reverses");
    assert!(cycle.message().fallback().unwrap_or_default().contains("b->a"), "the message must name the reversed edge");
    assert_eq!(lowered.plan.edges.len(), 2, "breaking the cycle must not drop an edge");
}

#[test]
fn a_self_edge_is_refused_at_its_own_statement() {
    let source = "digraph {\n  a -> b\n  b -> b\n}\n";
    let lowered = dot_to_scene(source).expect("the rest of the graph still lays out");

    let refusal = lowered
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code().as_str() == codes::UNSUPPORTED)
        .unwrap_or_else(|| panic!("a self edge must be refused: {:?}", lowered.diagnostics));

    assert_eq!(label_text(source, refusal), "b -> b", "the label must cover the self edge statement");
    assert_eq!(lowered.plan.edges.len(), 1, "the self edge is skipped and every other edge is routed");
}
