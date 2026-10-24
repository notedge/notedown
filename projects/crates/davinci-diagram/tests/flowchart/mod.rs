use davinci_diagram::{DiagramError, LoweredDiagram, lower_flowchart};
use davinci_scene::{Bounds, NodeKind, Scene, SceneNode, codes};
use oak_mermaid::ast::{EdgeStyle, FlowElement, Flowchart, NodeShape};

fn node(id: &str, text: Option<&str>, shape: NodeShape) -> FlowElement {
    FlowElement::Node { id: id.to_string(), text: text.map(str::to_string), shape }
}

fn edge(from: &str, to: &str, text: Option<&str>, style: EdgeStyle) -> FlowElement {
    FlowElement::Edge { from: from.to_string(), to: to.to_string(), text: text.map(str::to_string), style }
}

fn chart(orientation: &str, elements: Vec<FlowElement>) -> Flowchart {
    Flowchart { orientation: orientation.to_string(), elements }
}

fn find_node<'a>(scene: &'a Scene, id: &str) -> &'a SceneNode {
    scene.nodes.iter().find(|node| node.id.as_deref() == Some(id)).unwrap_or_else(|| panic!("missing node `{id}`"))
}

fn bounds_of(scene: &Scene, id: &str) -> Bounds {
    find_node(scene, id).bounds().unwrap_or_else(|| panic!("node `{id}` has no bounds"))
}

fn texts(lowered: &LoweredDiagram) -> Vec<String> {
    lowered
        .scene
        .nodes
        .iter()
        .filter_map(|node| match &node.kind {
            NodeKind::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn top_down_places_target_below_source() {
    let chart = chart(
        "TD",
        vec![
            node("A", Some("Start"), NodeShape::Box),
            node("B", Some("Stop"), NodeShape::Round),
            edge("A", "B", Some("next"), EdgeStyle::Arrow),
        ],
    );

    let lowered = lower_flowchart(&chart).expect("flowchart lowers");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);

    let start = bounds_of(&lowered.scene, "node-0");
    let stop = bounds_of(&lowered.scene, "node-1");
    assert!(start.max.y < stop.min.y, "start {start:?} stop {stop:?}");
    assert!((start.center().x - stop.center().x).abs() < 1e-9);

    let svg = lowered.to_svg().expect("scene serializes");
    assert!(svg.contains(">Start</text>"), "{svg}");
    assert!(svg.contains(">Stop</text>"), "{svg}");
    assert!(svg.contains(">next</text>"), "{svg}");
    assert!(svg.contains("id=\"edge-0\""), "{svg}");
    assert!(svg.contains("id=\"edge-0-arrow\""), "{svg}");
}

#[test]
fn direction_controls_layout_axis() {
    let elements = vec![node("A", None, NodeShape::Box), node("B", None, NodeShape::Box), edge("A", "B", None, EdgeStyle::Arrow)];

    let left_to_right = lower_flowchart(&chart("LR", elements.clone())).expect("lr lowers");
    assert!(bounds_of(&left_to_right.scene, "node-0").max.x < bounds_of(&left_to_right.scene, "node-1").min.x);

    let right_to_left = lower_flowchart(&chart("RL", elements.clone())).expect("rl lowers");
    assert!(bounds_of(&right_to_left.scene, "node-0").min.x > bounds_of(&right_to_left.scene, "node-1").max.x);

    let bottom_up = lower_flowchart(&chart("BT", elements)).expect("bt lowers");
    assert!(bounds_of(&bottom_up.scene, "node-0").min.y > bounds_of(&bottom_up.scene, "node-1").max.y);
}

#[test]
fn plain_edges_have_no_arrow_marker() {
    let chart = chart("TD", vec![node("A", None, NodeShape::Box), node("B", None, NodeShape::Box), edge("A", "B", None, EdgeStyle::Line)]);
    let lowered = lower_flowchart(&chart).expect("line edge lowers");
    let svg = lowered.to_svg().expect("scene serializes");
    assert!(svg.contains("id=\"edge-0\""), "{svg}");
    assert!(!svg.contains("edge-0-arrow"), "{svg}");
}

#[test]
fn dotted_edges_are_recorded_as_approximated() {
    let chart =
        chart("TD", vec![node("A", None, NodeShape::Box), node("B", None, NodeShape::Box), edge("A", "B", None, EdgeStyle::DottedArrow)]);
    let lowered = lower_flowchart(&chart).expect("dotted edge lowers");
    assert_eq!(lowered.diagnostics.len(), 1);
    assert_eq!(lowered.diagnostics[0].code().as_str(), codes::APPROXIMATED);
    assert!(lowered.diagnostics[0].message().fallback().expect("fallback").contains("dotted edge"));
}

#[test]
fn node_shapes_map_to_scene_primitives_and_record_approximations() {
    let chart = chart(
        "TD",
        vec![
            node("box", None, NodeShape::Box),
            node("round", None, NodeShape::Round),
            node("stadium", None, NodeShape::Stadium),
            node("subroutine", None, NodeShape::Subroutine),
            node("cylinder", None, NodeShape::Cylinder),
            node("circle", None, NodeShape::Circle),
            node("rhombus", None, NodeShape::Rhombus),
        ],
    );
    let lowered = lower_flowchart(&chart).expect("all shapes lower");

    assert!(matches!(find_node(&lowered.scene, "node-0").kind, NodeKind::Rect { .. }));
    assert!(matches!(find_node(&lowered.scene, "node-1").kind, NodeKind::RoundedRect { .. }));
    assert!(matches!(find_node(&lowered.scene, "node-2").kind, NodeKind::RoundedRect { radius, .. } if radius == 20.0));
    assert!(matches!(find_node(&lowered.scene, "node-3").kind, NodeKind::Rect { .. }));
    assert!(matches!(find_node(&lowered.scene, "node-4").kind, NodeKind::RoundedRect { .. }));
    assert!(matches!(find_node(&lowered.scene, "node-5").kind, NodeKind::Ellipse { .. }));
    assert!(matches!(find_node(&lowered.scene, "node-6").kind, NodeKind::Path { .. }));

    assert_eq!(lowered.diagnostics.len(), 3);
    assert!(lowered.diagnostics.iter().all(|record| record.code().as_str() == codes::APPROXIMATED));
    assert!(lowered.diagnostics[0].message().fallback().expect("fallback").contains("subroutine"));
    assert!(lowered.diagnostics[1].message().fallback().expect("fallback").contains("cylinder"));
    assert!(lowered.diagnostics[2].message().fallback().expect("fallback").contains("circle"));
}

#[test]
fn cycle_is_broken_with_a_warning_not_silence() {
    let chart = chart(
        "TD",
        vec![
            node("A", None, NodeShape::Box),
            node("B", None, NodeShape::Box),
            edge("A", "B", None, EdgeStyle::Arrow),
            edge("B", "A", None, EdgeStyle::Arrow),
        ],
    );
    let lowered = lower_flowchart(&chart).expect("cycle still lowers");
    assert_eq!(lowered.diagnostics.len(), 1);
    assert_eq!(lowered.diagnostics[0].code().as_str(), codes::APPROXIMATED);
    assert!(lowered.diagnostics[0].message().fallback().expect("fallback").contains("cycle"));
    assert_eq!(texts(&lowered), vec!["A".to_string(), "B".to_string()]);
}

#[test]
fn implicit_nodes_are_created_for_edge_endpoints() {
    let chart = chart("TD", vec![edge("left", "right", None, EdgeStyle::Arrow)]);
    let lowered = lower_flowchart(&chart).expect("implicit nodes lower");
    assert_eq!(texts(&lowered), vec!["left".to_string(), "right".to_string()]);
    assert!(lowered.scene.nodes.iter().all(|node| !matches!(node.kind, NodeKind::Group { .. })));
}

#[test]
fn labels_are_escaped_in_svg() {
    let chart = chart("TD", vec![node("A", Some("A & B"), NodeShape::Box)]);
    let lowered = lower_flowchart(&chart).expect("label lowers");
    let svg = lowered.to_svg().expect("scene serializes");
    assert!(svg.contains(">A &amp; B</text>"), "{svg}");
    assert!(!svg.contains(">A & B</text>"), "{svg}");
}

#[test]
fn lowering_is_deterministic() {
    let chart = chart(
        "LR",
        vec![
            node("A", Some("Alpha"), NodeShape::Box),
            node("B", Some("Beta"), NodeShape::Rhombus),
            edge("A", "B", Some("go"), EdgeStyle::Arrow),
        ],
    );
    let first = lower_flowchart(&chart).expect("first");
    let second = lower_flowchart(&chart).expect("second");
    assert_eq!(first.scene, second.scene);
    assert_eq!(first.diagnostics, second.diagnostics);
    assert_eq!(first.to_svg().expect("svg"), second.to_svg().expect("svg"));
}

#[test]
fn unsupported_orientation_and_empty_inputs_fail_explicitly() {
    let orientation = lower_flowchart(&chart("XX", vec![node("A", None, NodeShape::Box)])).expect_err("orientation");
    assert_eq!(orientation.code(), codes::UNSUPPORTED);
    assert!(orientation.message().contains("XX"));

    let empty = lower_flowchart(&chart("TD", Vec::new())).expect_err("empty");
    assert_eq!(empty.code(), codes::EMPTY_DIAGRAM);
}

#[test]
fn self_edge_and_empty_ids_fail_explicitly() {
    let self_edge =
        lower_flowchart(&chart("TD", vec![node("A", None, NodeShape::Box), edge("A", "A", None, EdgeStyle::Arrow)])).expect_err("self edge");
    assert_eq!(self_edge.code(), codes::UNSUPPORTED);
    assert!(self_edge.message().contains("self edge"));

    let empty_id = lower_flowchart(&chart("TD", vec![node(" ", None, NodeShape::Box)])).expect_err("empty id");
    assert_eq!(empty_id.code(), codes::UNSUPPORTED);
    assert!(empty_id.message().contains("empty id"));
}

#[test]
fn diagram_error_exposes_structured_diagnostic() {
    let error: DiagramError = lower_flowchart(&chart("XX", Vec::new())).expect_err("unsupported orientation");
    assert_eq!(error.diagnostic().origin().namespace(), "davinci");
    assert_eq!(error.diagnostic().origin().component(), "diagram");
    assert_eq!(error.diagnostic().code().as_str(), codes::UNSUPPORTED);
}
