use davinci_diagram::{
    compile::{LayoutOptions, compile},
    plan::DiagramLayoutPlan,
    spec::{Cluster, Constraint, DiagramEdge, DiagramNode, DiagramSpec, Direction, EdgeStyle, Endpoint, NodeShape, Port, Side},
};
use davinci_scene::Point;

fn node(id: &str) -> DiagramNode {
    DiagramNode { id: id.to_string(), label: None, shape: NodeShape::Box, ports: Vec::new(), cluster: None }
}

fn labeled_node(id: &str, label: &str) -> DiagramNode {
    DiagramNode { label: Some(label.to_string()), ..node(id) }
}

fn edge(from: &str, to: &str) -> DiagramEdge {
    DiagramEdge {
        from: Endpoint { node: from.to_string(), port: None },
        to: Endpoint { node: to.to_string(), port: None },
        label: None,
        style: EdgeStyle::Arrow,
    }
}

fn compile_spec(spec: &DiagramSpec) -> DiagramLayoutPlan {
    compile(spec, &LayoutOptions::default())
}

fn has_message(plan: &DiagramLayoutPlan, needle: &str) -> bool {
    plan.diagnostics.iter().any(|diagnostic| diagnostic.message().fallback().unwrap_or_default().contains(needle))
}

fn two_node_plan(direction: Direction) -> DiagramLayoutPlan {
    let mut spec = DiagramSpec::new(direction);
    spec.nodes = vec![node("A"), node("B")];
    spec.edges = vec![edge("A", "B")];
    compile_spec(&spec)
}

#[test]
fn four_directions_place_layers_along_the_primary_axis() {
    let top_down = two_node_plan(Direction::TopDown);
    let a = top_down.node("A").expect("A").rect;
    let b = top_down.node("B").expect("B").rect;
    assert!(a.bottom() < b.y, "top-down should place B below A");

    let bottom_up = two_node_plan(Direction::BottomUp);
    let a = bottom_up.node("A").expect("A").rect;
    let b = bottom_up.node("B").expect("B").rect;
    assert!(a.y > b.bottom(), "bottom-up should place B above A");

    let left_to_right = two_node_plan(Direction::LeftToRight);
    let a = left_to_right.node("A").expect("A").rect;
    let b = left_to_right.node("B").expect("B").rect;
    assert!(a.right() < b.x, "left-to-right should place B to the right of A");

    let right_to_left = two_node_plan(Direction::RightToLeft);
    let a = right_to_left.node("A").expect("A").rect;
    let b = right_to_left.node("B").expect("B").rect;
    assert!(a.x > b.right(), "right-to-left should place B to the left of A");
}

#[test]
fn same_rank_constraints_merge_layers() {
    let mut spec = DiagramSpec::new(Direction::TopDown);
    spec.nodes = vec![node("A"), node("B"), node("C"), node("D")];
    spec.edges = vec![edge("A", "B"), edge("B", "C")];
    spec.constraints = vec![Constraint::SameRank(vec!["C".to_string(), "D".to_string()])];

    let plan = compile_spec(&spec);
    let c = plan.node("C").expect("C").rect.center().y;
    let d = plan.node("D").expect("D").rect.center().y;
    assert_eq!(c, d, "same-rank nodes must share one layer");
    assert!(plan.diagnostics.is_empty(), "a satisfiable same-rank constraint must not report loss");
}

#[test]
fn place_before_constraints_delay_the_after_node() {
    let mut spec = DiagramSpec::new(Direction::TopDown);
    spec.nodes = vec![node("A"), node("B")];
    spec.constraints = vec![Constraint::PlaceBefore { before: "A".to_string(), after: "B".to_string() }];

    let plan = compile_spec(&spec);
    let a = plan.node("A").expect("A").rect.center().y;
    let b = plan.node("B").expect("B").rect.center().y;
    assert!(a < b, "the after node must be placed on a strictly later layer");
    assert!(plan.diagnostics.is_empty());
}

#[test]
fn declared_ports_anchor_edges_and_routes_stay_orthogonal() {
    let mut source = node("A");
    source.ports =
        vec![Port { id: "out".to_string(), side: Side::Bottom, offset: 0.25 }, Port { id: "in".to_string(), side: Side::Top, offset: 1.0 }];
    let mut spec = DiagramSpec::new(Direction::TopDown);
    spec.nodes = vec![source, node("B")];
    spec.edges = vec![DiagramEdge {
        from: Endpoint { node: "A".to_string(), port: Some("out".to_string()) },
        to: Endpoint { node: "B".to_string(), port: None },
        label: None,
        style: EdgeStyle::Arrow,
    }];

    let plan = compile_spec(&spec);
    let a = plan.node("A").expect("A");
    let out = Point::new(a.rect.x + a.rect.width * 0.25, a.rect.bottom());
    let input = Point::new(a.rect.right(), a.rect.y);
    assert_eq!(a.ports.get("out"), Some(&out));
    assert_eq!(a.ports.get("in"), Some(&input));

    let routed = &plan.edges[0];
    assert_eq!(routed.points.first(), Some(&out));
    let b = plan.node("B").expect("B");
    assert_eq!(routed.points.last(), Some(&Point::new(b.rect.center().x, b.rect.y)));
    for pair in routed.points.windows(2) {
        assert!(pair[0].x == pair[1].x || pair[0].y == pair[1].y, "edge segment {pair:?} is not axis-aligned");
    }
}

#[test]
fn compilation_is_deterministic() {
    let mut spec = DiagramSpec::new(Direction::LeftToRight);
    let mut first = node("A");
    first.cluster = Some("group".to_string());
    let mut second = node("B");
    second.cluster = Some("group".to_string());
    spec.nodes = vec![first, second, node("C")];
    spec.edges = vec![edge("A", "B"), DiagramEdge { label: Some("maybe".to_string()), ..edge("B", "C") }];
    spec.clusters =
        vec![Cluster { id: "group".to_string(), label: Some("Group".to_string()), members: vec!["A".to_string(), "B".to_string()] }];
    spec.constraints = vec![Constraint::PlaceBefore { before: "A".to_string(), after: "C".to_string() }];

    let first_plan = compile_spec(&spec);
    let second_plan = compile_spec(&spec);
    assert_eq!(first_plan, second_plan, "compile must be deterministic");
    assert_eq!(second_plan.node("C").expect("C").label, "C");
}

#[test]
fn labels_drive_node_size_and_multiline_labels_add_height() {
    let mut spec = DiagramSpec::new(Direction::TopDown);
    spec.nodes = vec![labeled_node("A", "A very long node label"), labeled_node("B", "line one\nline two")];

    let plan = compile_spec(&spec);
    assert!(plan.node("A").expect("A").rect.width > 80.0);
    assert!(plan.node("B").expect("B").rect.height > 40.0);
}

#[test]
fn cluster_frames_expand_members_and_report_dangling_members() {
    let mut first = node("A");
    first.cluster = Some("group".to_string());
    let mut second = node("B");
    second.cluster = Some("group".to_string());
    let mut spec = DiagramSpec::new(Direction::TopDown);
    spec.nodes = vec![first, second];
    spec.edges = vec![edge("A", "B")];
    spec.clusters = vec![Cluster {
        id: "group".to_string(),
        label: Some("Group".to_string()),
        members: vec!["A".to_string(), "B".to_string(), "ghost".to_string()],
    }];

    let plan = compile_spec(&spec);
    let options = LayoutOptions::default();
    assert_eq!(plan.clusters.len(), 1);
    let cluster = &plan.clusters[0];
    for id in ["A", "B"] {
        let rect = plan.node(id).expect(id).rect;
        assert!(cluster.rect.x <= rect.x - options.padding_x);
        assert!(cluster.rect.y <= rect.y - options.padding_y);
        assert!(cluster.rect.right() >= rect.right() + options.padding_x);
        assert!(cluster.rect.bottom() >= rect.bottom() + options.padding_y);
    }
    assert_eq!(plan.node("A").expect("A").cluster.as_deref(), Some("group"));
    assert!(has_message(&plan, "ghost"), "dangling cluster members must be reported");
}

#[test]
fn dangling_edges_unknown_ports_and_duplicate_ids_are_reported() {
    let mut duplicate = node("A");
    duplicate.label = Some("Second definition".to_string());
    let mut spec = DiagramSpec::new(Direction::TopDown);
    spec.nodes = vec![node("A"), duplicate, node("B")];
    spec.edges = vec![
        edge("A", "missing"),
        DiagramEdge {
            from: Endpoint { node: "A".to_string(), port: Some("nope".to_string()) },
            to: Endpoint { node: "B".to_string(), port: None },
            label: None,
            style: EdgeStyle::Arrow,
        },
        edge("A", "A"),
    ];

    let plan = compile_spec(&spec);
    assert_eq!(plan.nodes.len(), 2, "the duplicate node definition is merged");
    assert_eq!(plan.node("A").expect("A").label, "A", "the first definition is kept");
    assert_eq!(plan.edges.len(), 1, "dangling and self edges are skipped");
    assert!(has_message(&plan, "duplicate node id"));
    assert!(has_message(&plan, "unknown node `missing`"));
    assert!(has_message(&plan, "unknown port `nope`"));
    assert!(has_message(&plan, "self edge"));

    let a = plan.node("A").expect("A").rect;
    assert_eq!(plan.edges[0].points.first(), Some(&Point::new(a.center().x, a.bottom())));
}

#[test]
fn cycles_are_placed_deterministically_and_reported() {
    let mut spec = DiagramSpec::new(Direction::TopDown);
    spec.nodes = vec![node("A"), node("B")];
    spec.edges = vec![edge("A", "B"), edge("B", "A")];

    let plan = compile_spec(&spec);
    assert_eq!(plan.nodes.len(), 2);
    assert_eq!(plan.edges.len(), 2);
    assert!(has_message(&plan, "cycle"), "a cycle must be reported");
    assert_eq!(plan, compile_spec(&spec));
}

// ---------------------------------------------------------------------------
// Edge routing around nodes
// ---------------------------------------------------------------------------

/// Test-side oracle for orthogonal segment vs rectangle interior checks. The
/// caller supplies the inflation: node obstacles use half the node stroke
/// (1.5 / 2) plus the 4px minimum gap, label obstacles use the 4px label
/// clearance.
fn oracle_segment_hits_rect(a: Point, b: Point, rect: davinci_scene::Rect, inflate: f64) -> bool {
    let (x, y, right, bottom) = (rect.x - inflate, rect.y - inflate, rect.right() + inflate, rect.bottom() + inflate);
    if (a.y - b.y).abs() < f64::EPSILON {
        let (low, high) = if a.x <= b.x { (a.x, b.x) } else { (b.x, a.x) };
        return a.y > y && a.y < bottom && high > x && low < right;
    }
    if (a.x - b.x).abs() < f64::EPSILON {
        let (low, high) = if a.y <= b.y { (a.y, b.y) } else { (b.y, a.y) };
        return a.x > x && a.x < right && high > y && low < bottom;
    }
    panic!("routes must stay orthogonal: {a:?} -> {b:?}")
}

fn oracle_segment_hits_inflated_node(a: Point, b: Point, rect: davinci_scene::Rect) -> bool {
    oracle_segment_hits_rect(a, b, rect, 4.75)
}

/// The acceptance topology: a long bypass edge whose straight track would
/// pass through an unrelated intermediate node, plus a converging join.
fn bypass_plan() -> DiagramLayoutPlan {
    let mut spec = DiagramSpec::new(Direction::TopDown);
    spec.nodes =
        vec![node("A"), node("B"), node("C"), node("D"), node("E"), node("F"), node("G"), node("H"), node("I")];
    spec.edges = vec![
        edge("A", "B"),
        DiagramEdge { label: Some("yes".to_string()), ..edge("B", "C") },
        DiagramEdge { label: Some("no".to_string()), ..edge("B", "D") },
        edge("C", "E"),
        edge("E", "F"),
        DiagramEdge { label: Some("no".to_string()), ..edge("F", "G") },
        DiagramEdge { label: Some("yes".to_string()), ..edge("F", "H") },
        edge("H", "G"),
        edge("G", "I"),
    ];
    compile_spec(&spec)
}

#[test]
fn edges_never_cross_a_non_endpoint_node() {
    let plan = bypass_plan();
    assert!(plan.diagnostics.iter().all(|diagnostic| !diagnostic.message().fallback().unwrap_or_default().contains("no channel")),
        "every edge must find a clearing channel: {:?}", plan.diagnostics);

    for routed in &plan.edges {
        let from_id = routed.from.node.as_str();
        let to_id = routed.to.node.as_str();
        for obstacle in plan.nodes.iter().filter(|placed| placed.id != from_id && placed.id != to_id) {
            for segment in routed.points.windows(2) {
                assert!(
                    !oracle_segment_hits_inflated_node(segment[0], segment[1], obstacle.rect),
                    "edge {from_id}->{to_id} crosses node {}: {:?} vs {:?}",
                    obstacle.id,
                    segment,
                    obstacle.rect
                );
            }
        }
    }

    // Foreign edge label boxes are obstacles too: no segment may cross the
    // inflated label box of another edge.
    for (index, routed) in plan.edges.iter().enumerate() {
        for (other_index, other) in plan.edges.iter().enumerate() {
            if other_index == index {
                continue;
            }
            let Some(label_box) = other.label_box else { continue };
            for segment in routed.points.windows(2) {
                assert!(
                    !oracle_segment_hits_rect(segment[0], segment[1], label_box, 4.0),
                    "edge {}->{} crosses the label of {}->{}: {:?} vs {:?}",
                    routed.from.node,
                    routed.to.node,
                    other.from.node,
                    other.to.node,
                    segment,
                    label_box
                );
            }
        }
    }
}

#[test]
fn the_bypass_edge_visibly_detours_around_the_intermediate_node() {
    let plan = bypass_plan();
    let bypass = plan.edges.iter().find(|routed| routed.from.node == "F" && routed.to.node == "G").expect("F->G edge");
    assert!(bypass.points.len() > 2, "the bypass must leave the straight track: {:?}", bypass.points);
    let h = plan.node("H").expect("H");
    let lane_x = bypass.points.iter().map(|point| point.x).fold(f64::NAN, f64::min).min(
        bypass.points.iter().map(|point| point.x).fold(f64::NAN, f64::max),
    );
    assert!(lane_x <= h.rect.x - 4.0 || lane_x >= h.rect.right() + 4.0, "the detour lane must clear H by the gap: {lane_x} vs {:?}", h.rect);
}

#[test]
fn converging_edges_share_a_track_in_front_of_the_target() {
    let plan = bypass_plan();
    let target = plan.node("G").expect("G");
    let anchor = Point::new(target.rect.center().x, target.rect.y);
    let merge_y = anchor.y - 16.0;
    for routed in plan.edges.iter().filter(|routed| routed.to.node == "G") {
        assert!(
            routed.points.iter().any(|point| (point.x - anchor.x).abs() < f64::EPSILON && (point.y - merge_y).abs() < f64::EPSILON),
            "edge {}->G must pass the shared merge point ({}, {merge_y}): {:?}",
            routed.from.node,
            anchor.x,
            routed.points
        );
    }
}

#[test]
fn routing_is_byte_deterministic() {
    assert_eq!(bypass_plan(), bypass_plan());
}

#[test]
fn an_edge_to_an_undeclared_node_is_refused_at_its_statement() {
    let mut spec = DiagramSpec::new(Direction::TopDown);
    spec.nodes = vec![node("A")];
    spec.edges = vec![edge("A", "missing")];

    let plan = compile_spec(&spec);
    assert!(has_message(&plan, "references unknown node `missing`"), "the diagnostic must name the missing node: {:?}", plan.diagnostics);
    assert!(plan.edges.is_empty(), "the dangling edge is dropped and the remaining nodes still lay out");
}

#[test]
fn an_edge_to_an_undeclared_port_keeps_its_route_on_the_default_anchor() {
    let mut spec = DiagramSpec::new(Direction::TopDown);
    spec.nodes = vec![node("A"), node("B")];
    spec.edges = vec![DiagramEdge {
        from: Endpoint { node: "A".to_string(), port: Some("ghost".to_string()) },
        to: Endpoint { node: "B".to_string(), port: None },
        label: None,
        style: EdgeStyle::Arrow,
    }];

    let plan = compile_spec(&spec);
    assert!(has_message(&plan, "unknown port `ghost`"), "the diagnostic must name the port: {:?}", plan.diagnostics);
    assert_eq!(plan.edges.len(), 1, "the edge keeps its route on the default anchor");
}
