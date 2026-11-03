//! PlantUML frontend acceptance: the class and sequence subset lowers onto the
//! same [`DiagramSpec`] the Mermaid and DOT frontends use, everything else is
//! refused with a structured diagnostic that points at the authored line.

use std::{fs, ops::Range, path::PathBuf};

use davinci_diagram::{
    DiagramError, plantuml_to_scene, plantuml_to_spec,
    spec::{Activation, DiagramEdge, DiagramNode, DiagramSpec, Direction, EdgeStyle, Endpoint, LayoutModel, NodeShape, SequenceLayout},
};
use davinci_types::codes;

fn span_of<'a>(source: &'a str, error: &DiagramError) -> &'a str {
    let span = error.span().unwrap_or_else(|| panic!("`{}` carries no span", error.message()));
    let Range { start, end } = span.clone();
    assert!(end <= source.len(), "span {start}..{end} leaves a {} byte source", source.len());
    &source[start..end]
}

fn node(id: &str, label: Option<&str>, shape: NodeShape) -> DiagramNode {
    DiagramNode { id: id.to_string(), label: label.map(str::to_string), shape, ports: Vec::new(), cluster: None }
}

fn edge(from: &str, to: &str, label: Option<&str>, style: EdgeStyle) -> DiagramEdge {
    DiagramEdge {
        from: Endpoint { node: from.to_string(), port: None },
        to: Endpoint { node: to.to_string(), port: None },
        label: label.map(str::to_string),
        style,
    }
}

const CLASS_SOURCE: &str = r#"@startuml
class Order {
  +id : String
  -total : Int
}
interface Payable {
  +pay()
}
Order o-- Payable
Order <|-- OnlineOrder : extends
@enduml
"#;

const SEQUENCE_SOURCE: &str = r#"@startuml
actor User
participant "Order Service" as Svc
User ->> Svc : place()
activate Svc
Svc -->> User : accepted
deactivate Svc
@enduml
"#;

#[test]
fn class_declarations_members_and_relations_lower_to_one_spec() {
    let spec = plantuml_to_spec(CLASS_SOURCE).expect("the class subset should lower");

    assert_eq!(spec.direction, Direction::BottomUp);
    assert_eq!(spec.layout, LayoutModel::Layered);
    assert_eq!(
        spec.nodes,
        vec![
            node("Order", Some("Order\n+id : String\n-total : Int"), NodeShape::Box),
            node("Payable", Some("Payable\n+pay()"), NodeShape::Box),
            node("OnlineOrder", None, NodeShape::Box),
        ]
    );
    assert_eq!(
        spec.edges,
        vec![edge("Payable", "Order", None, EdgeStyle::HollowDiamond), edge("OnlineOrder", "Order", Some("extends"), EdgeStyle::HollowArrow),]
    );
    assert!(spec.clusters.is_empty());
    assert!(spec.constraints.is_empty());
}

#[test]
fn sequence_declarations_messages_and_activations_lower_to_one_spec() {
    let spec = plantuml_to_spec(SEQUENCE_SOURCE).expect("the sequence subset should lower");

    assert_eq!(spec.direction, Direction::TopDown);
    assert_eq!(spec.nodes, vec![node("User", None, NodeShape::Round), node("Svc", Some("Order Service"), NodeShape::Box)]);
    assert_eq!(
        spec.edges,
        vec![edge("User", "Svc", Some("place()"), EdgeStyle::Arrow), edge("Svc", "User", Some("accepted"), EdgeStyle::DottedArrow),]
    );
    assert_eq!(
        spec.layout,
        LayoutModel::TimeOrdered(SequenceLayout {
            participants: vec!["User".to_string(), "Svc".to_string()],
            messages: vec![0, 1],
            activations: vec![Activation { participant: "Svc".to_string(), from_message: 1, to_message: 1 }],
        })
    );
}

#[test]
fn class_and_sequence_specs_render_to_svg_with_all_labels() {
    let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("diagram-artifacts");
    fs::create_dir_all(&out_dir).expect("create diagram artifact dir");

    for (name, source, labels) in [
        ("plantuml-class", CLASS_SOURCE, &["Order", "+id : String", "-total : Int", "Payable", "+pay()", "OnlineOrder", "extends"][..]),
        ("plantuml-sequence", SEQUENCE_SOURCE, &["User", "Order Service", "place()", "accepted"][..]),
    ] {
        let lowered = plantuml_to_scene(source).unwrap_or_else(|error| panic!("{name}: lowering failed: {error}"));
        let svg = lowered.scene.to_svg().unwrap_or_else(|error| panic!("{name}: scene must serialize to SVG: {error}"));
        for label in labels {
            assert!(svg.contains(label), "{name}: SVG must draw the label `{label}`\n{svg}");
        }
        let artifact = out_dir.join(format!("{name}.svg"));
        fs::write(&artifact, &svg).expect("write plantuml svg artifact");
        eprintln!("wrote {} ({} bytes)", artifact.display(), svg.len());
    }
}

/// The Mermaid class frontend builds the same spec as the PlantUML one for a
/// source that expresses the same classes and relations.
#[test]
fn plantuml_and_mermaid_class_sources_lower_to_the_same_spec() {
    let plantuml = plantuml_to_spec(CLASS_SOURCE).expect("plantuml class source should lower");
    let mermaid = davinci_diagram::mermaid_to_spec(
        "classDiagram\n  class Order {\n    +id : String\n    -total : Int\n  }\n  class Payable {\n    +pay()\n  }\n  Order o-- Payable\n  Order <|-- OnlineOrder : extends\n",
    )
    .expect("mermaid class source should lower");

    assert_eq!(plantuml.direction, mermaid.direction);
    assert_eq!(plantuml.layout, mermaid.layout);
    assert_eq!(plantuml.nodes, mermaid.nodes);
    assert_eq!(plantuml.edges, mermaid.edges);
    assert_eq!(plantuml.clusters, mermaid.clusters);
    assert_eq!(plantuml.constraints, mermaid.constraints);
}

/// The Mermaid sequence frontend declares the same participant axis, message
/// order, and arrow styles for the same conversation.
#[test]
fn plantuml_and_mermaid_sequence_sources_lower_to_the_same_spec() {
    let plantuml = plantuml_to_spec(SEQUENCE_SOURCE).expect("plantuml sequence source should lower");
    let mermaid = davinci_diagram::mermaid_to_spec(
        "sequenceDiagram\n  actor User\n  participant Svc as Order Service\n  User->>Svc: place()\n  Svc-->>User: accepted\n",
    )
    .expect("mermaid sequence source should lower");

    assert_eq!(plantuml.direction, mermaid.direction);
    assert_eq!(plantuml.nodes, mermaid.nodes);
    assert_eq!(plantuml.edges, mermaid.edges);
    let (LayoutModel::TimeOrdered(plantuml), LayoutModel::TimeOrdered(mermaid_layout)) = (&plantuml.layout, &mermaid.layout)
    else {
        panic!("both sequence frontends must declare a time-ordered layout");
    };
    assert_eq!(plantuml.participants, mermaid_layout.participants);
    assert_eq!(plantuml.messages, mermaid_layout.messages);
    assert!(mermaid_layout.activations.is_empty(), "Mermaid has no activation gap in this source");
    assert_eq!(plantuml.activations.len(), 1, "the authored `activate` / `deactivate` pair survives lowering");
}

fn assert_refused(source: &str, expected: &str) {
    let error = plantuml_to_spec(source).expect_err("the construct is outside the accepted subset");
    assert_eq!(error.code(), codes::UNSUPPORTED, "for\n{source}");
    assert_eq!(span_of(source, &error), expected, "for\n{source}");
    assert!(error.labelled("plantuml").primary().is_some(), "refusals must label the `plantuml` source");
}

#[test]
fn preprocessor_directives_and_foreign_start_markers_are_refused() {
    assert_refused("@startuml\n!include common.iuml\n@enduml\n", "!include common.iuml");
    assert_refused("@startdot\ndigraph { A }\n@enddot\n", "@startdot");
    assert_refused("class A\n@startuml\nclass B\n@enduml\n", "class A");
}

#[test]
fn statements_outside_the_class_and_sequence_subset_are_refused() {
    for (source, expected) in [
        ("@startuml\nskinparam monochrome true\n@enduml\n", "skinparam monochrome true"),
        ("@startuml\nnote right of A : hello\n@enduml\n", "note right of A : hello"),
        ("@startuml\nstate Idle\n@enduml\n", "state Idle"),
        ("@startuml\nautonumber\nA -> B : go\n@enduml\n", "autonumber"),
    ] {
        assert_refused(source, expected);
    }
}

#[test]
fn alternate_sequence_blocks_are_refused_with_their_line() {
    let source = "@startuml\nA -> B : go\nalt success\nB -> A : ok\nend\n@enduml\n";
    assert_refused(source, "alt success");
}

#[test]
fn marker_cardinality_is_checked_before_the_parser() {
    let source = "@startuml\nclass A\n@startuml\nclass B\n@enduml\n";
    assert_refused(source, "@startuml");

    let source = "@startuml\nclass A\n";
    let error = plantuml_to_spec(source).expect_err("an unterminated document is refused");
    assert_eq!(error.code(), codes::UNSUPPORTED);
    assert_eq!(span_of(source, &error), "@startuml");
}

#[test]
fn rejected_multiplicities_are_not_silently_dropped() {
    let source = "@startuml\nclass A\nA \"1\" --> \"n\" B : owns\n@enduml\n";
    let lowered = plantuml_to_scene(source).expect("multiplicities lower as one edge label");
    let approximation = lowered
        .diagnostics
        .iter()
        .find(|record| record.code().as_str() == codes::APPROXIMATED)
        .expect("a folded multiplicity is an approximation");
    let label = approximation.primary().expect("approximations point at the source");
    assert_eq!(label.role(), davinci_types::LabelRole::Primary);
    assert!(matches!(label.location(), davinci_types::DiagnosticLocation::Text { .. }));

    let spec: DiagramSpec = plantuml_to_spec(source).expect("the multiplicity source still lowers to a spec");
    assert_eq!(spec.edges[0].label.as_deref(), Some("1 n owns"));
}
