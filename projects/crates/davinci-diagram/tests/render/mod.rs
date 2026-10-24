use davinci_diagram::{
    compile::{LayoutOptions, compile},
    plan::DiagramLayoutPlan,
    render::render,
    spec::{Cluster, DiagramEdge, DiagramNode, DiagramSpec, Direction, EdgeStyle, Endpoint, NodeShape},
};
use davinci_scene::{NodeKind, Scene, SceneNode, SemanticRole};

fn node(id: &str) -> DiagramNode {
    DiagramNode { id: id.to_string(), label: None, shape: NodeShape::Box, ports: Vec::new(), cluster: None }
}

fn edge(from: &str, to: &str, label: Option<&str>) -> DiagramEdge {
    DiagramEdge {
        from: Endpoint { node: from.to_string(), port: None },
        to: Endpoint { node: to.to_string(), port: None },
        label: label.map(str::to_string),
        style: EdgeStyle::Arrow,
    }
}

fn example_spec() -> DiagramSpec {
    let mut first = node("A");
    first.label = Some("Start".to_string());
    first.cluster = Some("group".to_string());
    let mut spec = DiagramSpec::new(Direction::TopDown);
    spec.nodes = vec![first, node("B")];
    spec.edges = vec![edge("A", "B", Some("go"))];
    spec.clusters =
        vec![Cluster { id: "group".to_string(), label: Some("Group".to_string()), members: vec!["A".to_string(), "B".to_string()] }];
    spec
}

fn example_plan() -> DiagramLayoutPlan {
    compile(&example_spec(), &LayoutOptions::default())
}

fn scene_node<'a>(scene: &'a Scene, id: &str) -> &'a SceneNode {
    scene.nodes.iter().find(|node| node.id.as_deref() == Some(id)).unwrap_or_else(|| panic!("missing scene node `{id}`"))
}

fn source_element(node: &SceneNode) -> Option<&str> {
    node.source.as_ref().and_then(|source| source.element.as_deref())
}

#[test]
fn nodes_edges_and_clusters_carry_semantics_and_stable_ids() {
    let plan = example_plan();
    let scene = render(&plan);

    let start = scene_node(&scene, "diagram-node-A");
    assert_eq!(start.role, Some(SemanticRole::Node));
    assert_eq!(source_element(start), Some("A"));
    assert_eq!(start.hit_region, Some(plan.node("A").expect("A").rect));

    let routed = scene_node(&scene, "diagram-edge-0");
    assert_eq!(routed.role, Some(SemanticRole::Edge));
    assert_eq!(source_element(routed), Some("A->B"));
    assert!(matches!(routed.kind, NodeKind::Group { .. }));

    let label = scene_node(&scene, "diagram-edge-0-label");
    assert_eq!(label.role, Some(SemanticRole::EdgeLabel));
    assert_eq!(source_element(label), Some("A->B"));

    let cluster = scene_node(&scene, "diagram-cluster-group");
    assert_eq!(cluster.role, Some(SemanticRole::Cluster));
    assert_eq!(source_element(cluster), Some("group"));
}

#[test]
fn rendering_is_deterministic_and_serializes_to_svg() {
    let plan = example_plan();
    let scene = render(&plan);
    let first = scene.to_svg().expect("scene should serialize");
    let second = render(&plan).to_svg().expect("scene should serialize");
    assert_eq!(first, second, "rendering the same plan twice must be identical");
    assert!(first.contains("data-role=\"node\""));
    assert!(first.contains("data-source=\"A\""));
    assert!(first.contains(">Start</text>"));
    assert!(first.contains(">go</text>"));
}

#[test]
fn node_shapes_map_to_scene_primitives() {
    let shapes = [
        NodeShape::Box,
        NodeShape::Round,
        NodeShape::Stadium,
        NodeShape::Subroutine,
        NodeShape::Cylinder,
        NodeShape::Circle,
        NodeShape::Rhombus,
    ];
    let mut spec = DiagramSpec::new(Direction::TopDown);
    for (index, shape) in shapes.iter().enumerate() {
        spec.nodes.push(DiagramNode { id: format!("N{index}"), label: None, shape: *shape, ports: Vec::new(), cluster: None });
    }

    let plan = compile(&spec, &LayoutOptions::default());
    let scene = render(&plan);
    assert!(matches!(scene_node(&scene, "diagram-node-N0").kind, NodeKind::Rect { .. }));
    assert!(matches!(scene_node(&scene, "diagram-node-N1").kind, NodeKind::RoundedRect { radius, .. } if radius == 8.0));
    assert!(
        matches!(scene_node(&scene, "diagram-node-N2").kind, NodeKind::RoundedRect { radius, .. } if (radius - plan.node("N2").expect("N2").rect.height * 0.5).abs() < f64::EPSILON)
    );
    assert!(matches!(scene_node(&scene, "diagram-node-N3").kind, NodeKind::Rect { .. }));
    assert!(matches!(scene_node(&scene, "diagram-node-N4").kind, NodeKind::Group { .. }));
    assert!(matches!(scene_node(&scene, "diagram-node-N5").kind, NodeKind::Ellipse { radii, .. } if radii.width == radii.height));
    assert!(matches!(scene_node(&scene, "diagram-node-N6").kind, NodeKind::Path { .. }));
}
