use davinci_diagram::{lower_flowchart, lower_mermaid};
use davinci_scene::codes;
use oak_mermaid::{
    MermaidRoot,
    ast::{ClassDiagram, FlowElement, Flowchart, MermaidDiagram, NodeShape, Sequence},
};

fn root(diagram: MermaidDiagram) -> MermaidRoot {
    MermaidRoot { diagrams: vec![diagram], span: (0..0).into() }
}

#[test]
fn single_flowchart_root_routes_to_flowchart_lowering() {
    let flowchart = Flowchart {
        orientation: "TD".to_string(),
        elements: vec![FlowElement::Node { id: "A".to_string(), text: Some("Alpha".to_string()), shape: NodeShape::Box }],
    };
    let lowered = lower_mermaid(&root(MermaidDiagram::Flowchart(flowchart.clone()))).expect("single flowchart lowers");
    assert_eq!(lowered, lower_flowchart(&flowchart).expect("direct lowering"));
}

#[test]
fn sequence_diagrams_are_rejected_explicitly() {
    let sequence = Sequence { participants: vec!["A".to_string()], messages: Vec::new() };
    let error = lower_mermaid(&root(MermaidDiagram::Sequence(sequence))).expect_err("sequence is unsupported");
    assert_eq!(error.code(), codes::UNSUPPORTED);
    assert!(error.message().contains("sequence"), "{}", error.message());
}

#[test]
fn class_diagrams_are_rejected_explicitly() {
    let error = lower_mermaid(&root(MermaidDiagram::ClassDiagram(ClassDiagram { classes: Vec::new(), relations: Vec::new() })))
        .expect_err("class diagram is unsupported");
    assert_eq!(error.code(), codes::UNSUPPORTED);
    assert!(error.message().contains("class"), "{}", error.message());
}

#[test]
fn empty_and_multi_diagram_roots_are_rejected() {
    let empty = MermaidRoot { diagrams: Vec::new(), span: (0..0).into() };
    assert_eq!(lower_mermaid(&empty).expect_err("empty root").code(), codes::AST_UNAVAILABLE);

    let flowchart = || Flowchart { orientation: "TD".to_string(), elements: Vec::new() };
    let multiple =
        MermaidRoot { diagrams: vec![MermaidDiagram::Flowchart(flowchart()), MermaidDiagram::Flowchart(flowchart())], span: (0..0).into() };
    let error = lower_mermaid(&multiple).expect_err("multiple diagrams");
    assert_eq!(error.code(), codes::UNSUPPORTED);
    assert!(error.message().contains("exactly one diagram"), "{}", error.message());
}
