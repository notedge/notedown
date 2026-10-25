use davinci_diagram::{
    parse::{mermaid_to_scene, mermaid_to_spec, parse_mermaid},
    spec::{Direction, EdgeStyle, NodeShape},
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
fn empty_source_is_unavailable_while_sequence_families_parse() {
    let empty = parse_mermaid("").expect_err("empty source has no AST");
    assert_eq!(empty.code(), codes::AST_UNAVAILABLE);
    assert!(empty.message().contains("no diagrams"));

    let root = parse_mermaid("sequenceDiagram\n  Alice->>Bob: Hello\n").expect("sequence AST should be available");
    assert_eq!(root.diagrams.len(), 1);

    let spec = mermaid_to_spec("sequenceDiagram\n  Alice->>Bob: Hello\n").expect_err("the graph spec is flowchart-only");
    assert_eq!(spec.code(), codes::UNSUPPORTED);
    assert!(spec.message().contains("mermaid_to_scene"));
}

const SEQUENCE_SOURCE: &str = "sequenceDiagram\n    participant Client as Browser\n    actor Server\n    participant DB\n    Client->>Server: request\n    Server->>DB: query\n    DB->>Server: rows\n    Server->>Client: response\n    Client->Server: ping\n";

#[test]
fn sequence_source_lowers_participants_messages_and_lifelines() {
    let lowered = mermaid_to_scene(SEQUENCE_SOURCE).expect("sequence should lower");

    let ids: Vec<&str> = lowered.plan.nodes.iter().map(|node| node.id.as_str()).collect();
    assert_eq!(ids, ["Client", "Server", "DB"], "participants keep declaration order");
    assert_eq!(lowered.plan.node("Client").expect("Client").label, "Browser");
    assert_eq!(lowered.plan.node("Server").expect("Server").label, "Server");
    assert_eq!(lowered.plan.node("Client").expect("Client").shape, NodeShape::Box);
    assert_eq!(lowered.plan.node("Server").expect("Server").shape, NodeShape::Round);
    assert_eq!(lowered.plan.edges.len(), 5);
    assert!(lowered.plan.canvas.width > 0.0, "canvas width comes from participant columns");
    assert!(lowered.plan.canvas.height > 0.0, "canvas height comes from message rows");
    assert!(lowered.diagnostics.is_empty(), "a clean sequence must not report loss: {:?}", lowered.diagnostics);

    let client = lowered.plan.node("Client").expect("Client").rect;
    let server = lowered.plan.node("Server").expect("Server").rect;
    let db = lowered.plan.node("DB").expect("DB").rect;
    assert!(client.x < server.x && server.x < db.x, "participant columns advance rightwards");

    let rows: Vec<f64> = lowered.plan.edges.iter().map(|edge| edge.points[0].y).collect();
    assert!(rows.windows(2).all(|pair| pair[0] < pair[1]), "message rows advance downwards: {rows:?}");
    assert_eq!(lowered.plan.edges[0].points[0].x, client.center().x);
    assert_eq!(lowered.plan.edges[0].points[1].x, server.center().x);

    let svg = lowered.scene.to_svg().expect("scene should serialize");
    assert!(svg.contains(">Browser</text>"), "{svg}");
    assert!(svg.contains(">request</text>"), "{svg}");
    assert!(svg.contains(">response</text>"), "{svg}");
    assert!(svg.contains("diagram-sequence-lifeline-Client"), "{svg}");
}

#[test]
fn sequence_lowering_is_deterministic() {
    let first = mermaid_to_scene(SEQUENCE_SOURCE).expect("sequence should lower");
    let second = mermaid_to_scene(SEQUENCE_SOURCE).expect("sequence should lower");
    assert_eq!(first.plan, second.plan);
    assert_eq!(first.scene, second.scene);
}

#[test]
fn dotted_sequence_messages_are_approximated_with_diagnostics() {
    let source = "sequenceDiagram\n    A-->>B: ack\n    B-->A: note\n";
    let lowered = mermaid_to_scene(source).expect("dotted messages should still lower");

    assert_eq!(lowered.plan.nodes.len(), 2, "message endpoints become implicit participants");
    assert_eq!(lowered.plan.edges.len(), 2);
    assert_eq!(lowered.diagnostics.len(), 2, "both dotted styles lose their dash surface: {:?}", lowered.diagnostics);
    assert!(
        lowered.diagnostics.iter().all(|diagnostic| diagnostic.code().as_str() == codes::APPROXIMATED),
        "{:?}",
        lowered.diagnostics
    );
}

#[test]
fn self_sequence_messages_are_skipped_with_a_diagnostic() {
    let source = "sequenceDiagram\n    A->>A: retry\n    A->>B: go\n";
    let lowered = mermaid_to_scene(source).expect("the rest of the sequence should lower");

    assert_eq!(lowered.plan.nodes.len(), 2);
    assert_eq!(lowered.plan.edges.len(), 1);
    assert_eq!(lowered.diagnostics.len(), 1);
    assert!(lowered.diagnostics[0].message().fallback().unwrap_or_default().contains("self"));
}

#[test]
fn unsupported_sequence_statements_fall_back_to_source() {
    let source = "sequenceDiagram\n    participant A\n    participant B\n    Note over A,B: shared\n    A->>B: ping\n";
    let error = mermaid_to_scene(source).expect_err("notes are not lowered");

    assert_eq!(error.code(), codes::UNSUPPORTED);
    assert!(error.message().contains("Note"), "{}", error.message());
    assert!(error.message().contains("line 4"), "{}", error.message());
    assert!(error.message().contains("falls back"), "{}", error.message());
}

const CLASS_SOURCE: &str = "classDiagram\n    class Animal {\n        +int age\n        +isMammal()\n    }\n    class Duck {\n        +swim()\n    }\n    Animal <|-- Duck : is a\n    Duck --> Pond : lives in\n";

#[test]
fn class_source_lowers_boxes_members_and_relations() {
    let lowered = mermaid_to_scene(CLASS_SOURCE).expect("class diagram should lower");

    let ids: Vec<&str> = lowered.plan.nodes.iter().map(|node| node.id.as_str()).collect();
    assert_eq!(ids, ["Animal", "Duck", "Pond"], "declared classes keep order and relation endpoints become implicit nodes");
    assert_eq!(lowered.plan.node("Animal").expect("Animal").label, "Animal\n+int age\n+isMammal()");
    assert_eq!(lowered.plan.node("Duck").expect("Duck").label, "Duck\n+swim()");
    assert_eq!(lowered.plan.node("Pond").expect("Pond").label, "Pond");

    assert_eq!(lowered.plan.edges.len(), 2);
    assert_eq!(lowered.plan.edges[0].from.node, "Duck");
    assert_eq!(lowered.plan.edges[0].to.node, "Animal");
    assert_eq!(lowered.plan.edges[0].style, EdgeStyle::HollowArrow);
    assert_eq!(lowered.plan.edges[0].label.as_deref(), Some("is a"));
    assert_eq!(lowered.plan.edges[1].from.node, "Duck");
    assert_eq!(lowered.plan.edges[1].to.node, "Pond");
    assert_eq!(lowered.plan.edges[1].style, EdgeStyle::Arrow);
    assert_eq!(lowered.plan.edges[1].label.as_deref(), Some("lives in"));
    assert!(lowered.diagnostics.is_empty(), "a clean class diagram must not report loss: {:?}", lowered.diagnostics);

    let svg = lowered.scene.to_svg().expect("scene should serialize");
    assert!(svg.contains(">Animal</text>"), "{svg}");
    assert!(svg.contains(">+int age</text>"), "{svg}");
    assert!(svg.contains(">is a</text>"), "{svg}");
}

#[test]
fn class_relation_styles_and_arrow_spellings_keep_their_uml_end() {
    let source = "classDiagram\n    class A\n    class B\n    class C\n    class D\n    A *-- B\n    A o-- C\n    A ..> D\n    A --|> E\n";
    let lowered = mermaid_to_scene(source).expect("class relations should lower");

    let edge = |index: usize| &lowered.plan.edges[index];
    assert_eq!(edge(0).from.node, "B", "composition puts the filled diamond on the whole");
    assert_eq!(edge(0).to.node, "A");
    assert_eq!(edge(0).style, EdgeStyle::FilledDiamond);
    assert_eq!(edge(1).from.node, "C", "aggregation puts the hollow diamond on the whole");
    assert_eq!(edge(1).to.node, "A");
    assert_eq!(edge(1).style, EdgeStyle::HollowDiamond);
    assert_eq!(edge(2).from.node, "A");
    assert_eq!(edge(2).to.node, "D");
    assert_eq!(edge(2).style, EdgeStyle::DottedArrow);
    assert_eq!(edge(3).from.node, "A", "`--|>` points at the right operand superclass");
    assert_eq!(edge(3).to.node, "E");
    assert_eq!(edge(3).style, EdgeStyle::HollowArrow);

    assert_eq!(lowered.plan.nodes.len(), 5, "`E` becomes an implicit class");
    assert_eq!(lowered.diagnostics.len(), 1, "only the dotted dependency is approximated: {:?}", lowered.diagnostics);
    assert_eq!(lowered.diagnostics[0].code().as_str(), codes::APPROXIMATED);
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
