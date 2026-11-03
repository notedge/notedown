use davinci_diagram::spec::{DiagramSpec, Direction};

#[test]
fn new_spec_is_empty_and_keeps_the_direction() {
    let spec = DiagramSpec::new(Direction::LeftToRight);
    assert_eq!(spec.direction, Direction::LeftToRight);
    assert!(spec.nodes.is_empty());
    assert!(spec.edges.is_empty());
    assert!(spec.clusters.is_empty());
    assert!(spec.constraints.is_empty());
}

/// The language-neutral acceptance gate.
///
/// `DiagramSpec` is the only input the compile/render pipeline accepts, so a
/// spec produced by any frontend must yield exactly the scene the Mermaid
/// frontend yields. Rebuilding the pipeline from the spec alone (never touching
/// the Mermaid AST) and comparing plan plus scene bytes is what proves the
/// frontend is a pure lowering step rather than a second implementation.
#[cfg(feature = "mermaid")]
#[test]
fn a_spec_alone_reproduces_the_mermaid_scene_byte_for_byte() {
    use davinci_diagram::{
        compile::{LayoutOptions, compile},
        mermaid_to_scene, mermaid_to_spec,
        render::render,
    };
    use davinci_scene::SceneResolver;

    for source in [
        "flowchart TD\n  A[Start] --> B{Ready?}\n  B -->|yes| C[Stop]\n  B -->|no| D[Retry]\n",
        "classDiagram\n  class A {\n    +run()\n  }\n  A <|-- B\n",
        "flowchart LR\n  subgraph group[Group]\n    A --> B\n  end\n",
        "sequenceDiagram\n  participant A\n  participant B\n  A->>B: hello\n  B-->>A: world\n",
    ] {
        let frontend = mermaid_to_scene(source).expect("frontend pipeline should lower");
        let spec = mermaid_to_spec(source).expect("spec should lower");
        let plan = compile(&spec, &LayoutOptions::default());
        let scene = render(&plan).resolve(&SceneResolver::default());

        assert_eq!(plan, frontend.plan, "the spec pipeline must reproduce the plan for\n{source}");
        let rebuilt = scene.to_svg().expect("rebuilt scene should serialize");
        let original = frontend.scene.to_svg().expect("frontend scene should serialize");
        assert_eq!(rebuilt, original, "the spec pipeline must reproduce the scene bytes for\n{source}");

        let again = render(&compile(&spec, &LayoutOptions::default())).resolve(&SceneResolver::default());
        assert_eq!(
            again.to_svg().expect("repeat should serialize"),
            rebuilt,
            "recompiling the same spec twice must stay byte-identical for\n{source}"
        );
    }
}

/// A declaration that does not change geometry would be cosmetic, so the axis,
/// message order, and activation span must all reach the plan.
#[test]
fn time_ordered_declaration_drives_axis_message_order_and_activations() {
    use davinci_diagram::{
        compile::{LayoutOptions, compile},
        plan::PlanLayout,
        render::render,
        spec::{Activation, DiagramEdge, DiagramNode, DiagramSpec, Direction, EdgeStyle, Endpoint, LayoutModel, NodeShape, SequenceLayout},
    };

    let node = |id: &str| DiagramNode { id: id.to_string(), label: None, shape: NodeShape::Box, ports: Vec::new(), cluster: None };
    let message = |from: &str, to: &str, label: &str| DiagramEdge {
        from: Endpoint { node: from.to_string(), port: None },
        to: Endpoint { node: to.to_string(), port: None },
        label: Some(label.to_string()),
        style: EdgeStyle::Arrow,
    };

    let mut spec = DiagramSpec::new(Direction::TopDown);
    spec.nodes = vec![node("A"), node("B"), node("C")];
    spec.edges = vec![message("A", "B", "one"), message("B", "C", "two")];
    spec.layout = LayoutModel::TimeOrdered(SequenceLayout {
        participants: vec!["C".to_string(), "A".to_string(), "B".to_string()],
        messages: vec![1, 0],
        activations: vec![Activation { participant: "B".to_string(), from_message: 0, to_message: 1 }],
    });

    let plan = compile(&spec, &LayoutOptions::default());
    assert_eq!(plan.layout, PlanLayout::TimeOrdered);
    let rows: Vec<&str> = plan.edges.iter().map(|edge| edge.label.as_deref().expect("label")).collect();
    assert_eq!(rows, ["two", "one"], "edges paint in the declared message order");
    let xs: Vec<f64> = ["C", "A", "B"].iter().map(|id| plan.node(id).expect(id).rect.center().x).collect();
    assert!(xs[0] < xs[1] && xs[1] < xs[2], "participants follow the declared axis: {xs:?}");

    assert_eq!(plan.activations.len(), 1);
    let activation = &plan.activations[0];
    assert_eq!(activation.participant, "B");
    assert_eq!(activation.rect.center().x, plan.node("B").expect("B").rect.center().x);
    assert!(activation.rect.height > 0.0 && activation.rect.bottom() <= plan.canvas.height);
    assert_eq!(plan.diagnostics.len(), 0, "a well-formed declaration reports no loss: {:?}", plan.diagnostics);

    let scene = render(&plan);
    assert!(scene.nodes.iter().any(|node| node.id.as_deref() == Some("sequence-activation-0")), "activation bars must reach the scene");
}
