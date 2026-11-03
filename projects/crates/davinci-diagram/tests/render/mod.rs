use davinci_diagram::{
    compile::{LayoutOptions, compile},
    plan::DiagramLayoutPlan,
    render::render,
    spec::{Cluster, DiagramEdge, DiagramNode, DiagramSpec, Direction, EdgeStyle, Endpoint, NodeShape},
};
use davinci_scene::{SceneResolver, SemanticKind, SemanticNode, SemanticRole, SemanticScene};

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

fn scene_node<'a>(scene: &'a SemanticScene, id: &str) -> &'a SemanticNode {
    scene.nodes.iter().find(|node| node.id.as_deref() == Some(id)).unwrap_or_else(|| panic!("missing scene node `{id}`"))
}

fn source_element(node: &SemanticNode) -> Option<&str> {
    node.source.as_ref().and_then(|source| source.element.as_deref())
}

#[test]
fn nodes_edges_and_clusters_carry_semantics_and_stable_ids() {
    let plan = example_plan();
    let scene = render(&plan);

    let start = scene_node(&scene, "node-A");
    assert_eq!(start.role, Some(SemanticRole::Node));
    assert_eq!(source_element(start), Some("A"));
    assert_eq!(start.hit_region, Some(plan.node("A").expect("A").rect));

    let routed = scene_node(&scene, "edge-0");
    assert_eq!(routed.role, Some(SemanticRole::Edge));
    assert_eq!(source_element(routed), Some("A->B"));
    assert!(matches!(routed.kind, SemanticKind::Group { .. }));

    let label = scene_node(&scene, "edge-0-label");
    assert_eq!(label.role, Some(SemanticRole::EdgeLabel));
    assert_eq!(source_element(label), Some("A->B"));

    let cluster = scene_node(&scene, "cluster-group");
    assert_eq!(cluster.role, Some(SemanticRole::Cluster));
    assert_eq!(source_element(cluster), Some("group"));
}

#[test]
fn rendering_is_deterministic_and_serializes_to_svg() {
    let plan = example_plan();
    let first = render(&plan).resolve(&SceneResolver::default()).to_svg().expect("scene should serialize");
    let second = render(&plan).resolve(&SceneResolver::default()).to_svg().expect("scene should serialize");
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
    assert!(matches!(scene_node(&scene, "node-N0").kind, SemanticKind::Rect { .. }));
    assert!(matches!(scene_node(&scene, "node-N1").kind, SemanticKind::RoundedRect { radius, .. } if radius == 5.0));
    assert!(
        matches!(scene_node(&scene, "node-N2").kind, SemanticKind::RoundedRect { radius, .. } if (radius - plan.node("N2").expect("N2").rect.height * 0.5).abs() < f64::EPSILON)
    );
    assert!(matches!(scene_node(&scene, "node-N3").kind, SemanticKind::Rect { .. }));
    assert!(matches!(scene_node(&scene, "node-N4").kind, SemanticKind::Group { .. }));
    assert!(matches!(scene_node(&scene, "node-N5").kind, SemanticKind::Ellipse { radii, .. } if radii.width == radii.height));
    assert!(matches!(scene_node(&scene, "node-N6").kind, SemanticKind::Path { .. }));
}

#[test]
fn edge_labels_paint_a_backing_plate_under_the_text() {
    let mut spec = DiagramSpec::new(Direction::TopDown);
    spec.nodes = vec![node("A"), node("B")];
    spec.edges = vec![edge("A", "B", Some("go"))];
    let plan = compile(&spec, &LayoutOptions::default());
    let scene = render(&plan);

    let plate = scene_node(&scene, "edge-0-label-bg");
    assert_eq!(plate.role, Some(SemanticRole::EdgeLabelBackdrop));
    let SemanticKind::RoundedRect { rect, radius, .. } = plate.kind else {
        panic!("the backing plate must be a rounded rect: {:?}", plate.kind)
    };
    assert_eq!(radius, 2.0, "the backing plate keeps rx = 2");

    // The plate covers the routed line at the label position.
    let routed = &plan.edges[0];
    let label_box = routed.label_box.expect("label box");
    assert_eq!(rect, label_box, "the plate is exactly the reserved label box");
    let midpoint = {
        let length: f64 = routed.points.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
        let mut remaining = length * 0.5;
        let mut center = routed.points[0];
        for pair in routed.points.windows(2) {
            let segment = pair[0].distance(pair[1]);
            if remaining <= segment && segment > 0.0 {
                let ratio = remaining / segment;
                center = davinci_scene::Point::new(pair[0].x + (pair[1].x - pair[0].x) * ratio, pair[0].y + (pair[1].y - pair[0].y) * ratio);
                break;
            }
            remaining -= segment;
        }
        center
    };
    assert!(rect.contains(midpoint), "the plate must cover the line under the label: {rect:?} vs {midpoint:?}");

    // The plate paints before the label text.
    let plate_index = scene.nodes.iter().position(|node| node.id.as_deref() == Some("edge-0-label-bg")).expect("plate index");
    let text_index = scene.nodes.iter().position(|node| node.id.as_deref() == Some("edge-0-label")).expect("label text");
    assert!(plate_index < text_index, "the plate must paint before the text");
}

#[test]
fn long_edge_labels_wrap_within_budget_and_truncate_with_an_ellipsis() {
    let long = (0..30).map(|index| format!("word{index}")).collect::<Vec<_>>().join(" ");
    let mut spec = DiagramSpec::new(Direction::TopDown);
    spec.nodes = vec![node("A"), node("B")];
    spec.edges = vec![edge("A", "B", Some(&long))];
    let plan = compile(&spec, &LayoutOptions::default());

    let label = plan.edges[0].label.as_deref().expect("wrapped label");
    let lines: Vec<&str> = label.split('\n').collect();
    assert!(lines.len() <= 3, "at most three lines: {lines:?}");
    assert!(lines.last().is_some_and(|line| line.ends_with('…')), "overflow truncates with an ellipsis: {lines:?}");
    for line in &lines {
        assert!(davinci_types::measure_line(line, 12.0) <= 200.0, "line exceeds the 200px budget: {line}");
    }
    let label_box = plan.edges[0].label_box.expect("label box");
    assert!(label_box.width <= 200.0 + 12.0, "the label box respects the budget plus padding: {label_box:?}");
    assert!(
        plan.diagnostics.iter().any(|diagnostic| diagnostic.message().fallback().unwrap_or_default().contains("truncated")),
        "truncation must be reported: {:?}",
        plan.diagnostics
    );
}
