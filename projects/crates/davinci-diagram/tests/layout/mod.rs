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
