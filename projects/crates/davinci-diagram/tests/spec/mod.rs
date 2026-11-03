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
