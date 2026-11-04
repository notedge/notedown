//! Deterministic layered compilation from a diagram spec to a layout plan.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ops::Range,
};

use davinci_scene::{Point, Rect, Size};
use davinci_types::{
    ByteRange, Diagnostic, DiagnosticLabel, DiagnosticLocation, DiagnosticSeverity, LabelRole, Message, NAMESPACE, SourceRef, codes,
    diagnostic, measure_line, wrap_label,
};

use crate::{
    plan::{DiagramLayoutPlan, PlacedActivation, PlacedCluster, PlacedNode, PlanLayout, RoutedEdge},
    source::SourceMap,
    spec::{Constraint, DiagramSpec, Direction, EdgeStyle, Endpoint, LayoutModel, NodeShape, Port, SequenceLayout, Side},
};

/// Tuning for deterministic layered compilation.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutOptions {
    /// Empty space reserved around the outermost geometry.
    pub margin: f64,
    /// Gap between two consecutive layers.
    pub layer_gap: f64,
    /// Gap between two nodes on the same layer.
    pub node_gap: f64,
    /// Horizontal padding around a node label, reused to expand cluster frames.
    pub padding_x: f64,
    /// Vertical padding around a node label, reused to expand cluster frames.
    pub padding_y: f64,
    /// Minimum node width.
    pub min_node_width: f64,
    /// Minimum node height.
    pub min_node_height: f64,
    /// Font size used for label measurement.
    pub font_size: f64,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            margin: 24.0,
            layer_gap: 72.0,
            node_gap: 32.0,
            padding_x: 12.0,
            padding_y: 8.0,
            min_node_width: 80.0,
            min_node_height: 40.0,
            font_size: 13.0,
        }
    }
}

/// Width of an activation bar drawn on a sequence lifeline.
const ACTIVATION_WIDTH: f64 = 8.0;

struct ResolvedNode {
    /// Index of this node in [`DiagramSpec::nodes`].
    spec_index: usize,
    id: String,
    label: Option<String>,
    shape: NodeShape,
    ports: Vec<Port>,
    cluster: Option<String>,
}

struct ResolvedCluster {
    id: String,
    label: Option<String>,
    members: Vec<usize>,
}

struct ResolvedEdge {
    /// Index of this edge in [`DiagramSpec::edges`].
    spec_index: usize,
    from: Endpoint,
    to: Endpoint,
    from_index: usize,
    to_index: usize,
    label: Option<String>,
    style: EdgeStyle,
}

/// Compile a diagram spec into a deterministic layout plan.
///
/// The pass is pure and deterministic: it never uses randomness or hash-map
/// iteration order, and it keeps nodes in spec declaration order.
///
/// The layout rule comes from [`DiagramSpec::layout`]: layered graphs use
/// [`compile_layered`], sequence diagrams use [`compile_sequence`]. Every
/// loss, approximation, and conflict record is reported through
/// [`DiagramLayoutPlan::diagnostics`].
///
/// Documented degradation rules:
///
/// - An edge whose endpoint node is missing is skipped with a diagnostic.
/// - An edge that references an unknown port keeps its route and falls back to
///   the default anchor for the layout direction, with a diagnostic.
/// - A self edge is skipped with a diagnostic.
/// - Duplicate node ids keep the first definition, merge later ports, and
///   report the duplicate.
/// - A cycle is broken in declaration order: the edges that no longer advance
///   to a later layer are named, and the layout still routes every edge.
/// - Unknown cluster references, cluster members that do not exist, and
///   clusters without members are reported; unusable members are ignored.
///
/// [`compile`] runs without authored positions. [`compile_with_source`] takes a
/// [`SourceMap`] so the same diagnostics also point at the token that caused
/// them.
pub fn compile(spec: &DiagramSpec, options: &LayoutOptions) -> DiagramLayoutPlan {
    compile_with_source(spec, options, &SourceMap::default())
}

/// Compile a diagram spec and locate diagnostics through a [`SourceMap`].
///
/// The map is optional per item: a diagnostic whose token the frontend could
/// not find is emitted without a primary label instead of guessing a range.
pub fn compile_with_source(spec: &DiagramSpec, options: &LayoutOptions, source: &SourceMap) -> DiagramLayoutPlan {
    match &spec.layout {
        LayoutModel::Layered => compile_layered_with_source(spec, options, source),
        LayoutModel::TimeOrdered(sequence) => compile_time_ordered(spec, sequence, options, source),
    }
}

/// Compile a layered graph spec without authored positions.
pub fn compile_layered(spec: &DiagramSpec, options: &LayoutOptions) -> DiagramLayoutPlan {
    compile_layered_with_source(spec, options, &SourceMap::default())
}

/// Compile a layered graph spec and locate diagnostics through a [`SourceMap`].
pub fn compile_layered_with_source(spec: &DiagramSpec, options: &LayoutOptions, source: &SourceMap) -> DiagramLayoutPlan {
    let mut diagnostics = Vec::new();
    let nodes = resolve_nodes(spec, &mut diagnostics, source);
    let index_by_id: BTreeMap<String, usize> = nodes.iter().enumerate().map(|(index, node)| (node.id.clone(), index)).collect();

    for node in &nodes {
        if matches!(node.shape, NodeShape::Subroutine) {
            diagnostics.push(located(
                codes::APPROXIMATED,
                format!("subroutine node `{}` is rendered as a plain rectangle without its double border", node.id),
                source.node(node.spec_index),
                source,
            ));
        }
    }

    let clusters = resolve_clusters(spec, &nodes, &index_by_id, &mut diagnostics, source);
    let edges = resolve_edges(spec, &index_by_id, &mut diagnostics, source);

    if nodes.is_empty() {
        return DiagramLayoutPlan {
            layout: PlanLayout::Layered,
            canvas: Size::ZERO,
            nodes: Vec::new(),
            edges: Vec::new(),
            clusters: Vec::new(),
            activations: Vec::new(),
            diagnostics,
        };
    }

    let layers = assign_layers(&nodes, &edges, &spec.constraints, &index_by_id, &mut diagnostics, source);
    let sizes: Vec<Size> = nodes.iter().map(|node| node_size(node, options)).collect();
    let mut rects = place_rects(&sizes, &layers, spec.direction, options);

    let mut cluster_rects: Vec<Option<Rect>> = Vec::with_capacity(clusters.len());
    for cluster in &clusters {
        let bounds = cluster.members.iter().map(|&member| rects[member].to_bounds()).reduce(|left, right| left.union(right));
        cluster_rects.push(bounds.map(|bounds| {
            Rect::new(
                bounds.min.x - options.padding_x,
                bounds.min.y - options.padding_y,
                bounds.width() + options.padding_x * 2.0,
                bounds.height() + options.padding_y * 2.0,
            )
        }));
    }

    let (shift_x, shift_y, canvas) = canvas_shift(&rects, &cluster_rects, options);
    for rect in &mut rects {
        rect.x += shift_x;
        rect.y += shift_y;
    }
    for rect in cluster_rects.iter_mut().flatten() {
        rect.x += shift_x;
        rect.y += shift_y;
    }

    let mut ports: Vec<BTreeMap<String, Point>> = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        ports.push(build_ports(node, rects[index], &mut diagnostics, source));
    }

    let memberships = node_memberships(&clusters, nodes.len());
    let anchors = {
        let mut resolver =
            EndpointResolver { nodes: &nodes, rects: &rects, ports: &ports, direction: spec.direction, reported_ports: BTreeSet::new() };
        edges
            .iter()
            .map(|edge| {
                let span = source.edge(edge.spec_index);
                let from = resolver.resolve(&edge.from, edge.from_index, true, &mut diagnostics, span.clone(), source);
                let to = resolver.resolve(&edge.to, edge.to_index, false, &mut diagnostics, span, source);
                (from, to)
            })
            .collect::<Vec<_>>()
    };

    // Routing stage 1: node avoidance. Every route keeps at least
    // `ROUTE_NODE_CLEARANCE` around every non-endpoint node; a route that
    // cannot clear the obstacles keeps the direct path with a diagnostic
    // instead of silently overlapping.
    let mut lanes: Vec<UsedLane> = Vec::new();
    let mut routed: Vec<RoutedEdge> = Vec::with_capacity(edges.len());
    for (edge, &(from, to)) in edges.iter().zip(anchors.iter()) {
        let obstacles = inflated_node_obstacles(&rects, edge.from_index, edge.to_index);
        let points = route_avoiding(from, to, spec.direction, &obstacles, &mut lanes).unwrap_or_else(|| {
            diagnostics.push(located(
                codes::APPROXIMATED,
                format!("edge `{}->{}` found no channel that clears every node and keeps the direct route", edge.from.node, edge.to.node),
                source.edge(edge.spec_index),
                source,
            ));
            route(from, to, spec.direction)
        });
        let label = edge.label.as_deref().and_then(|label| {
            prepare_edge_label(label, &edge.from.node, &edge.to.node, edge_label_font_size(options.font_size), &mut diagnostics)
        });
        let label_box =
            label.as_ref().map(|label| estimate_label_box(label, polyline_midpoint(&points), edge_label_font_size(options.font_size)));
        if matches!(edge.style, EdgeStyle::DottedArrow | EdgeStyle::DottedLine) {
            diagnostics.push(located(
                codes::APPROXIMATED,
                format!(
                    "dotted edge `{}->{}` is rendered as a solid line because the scene style has no dash surface",
                    edge.from.node, edge.to.node
                ),
                source.edge(edge.spec_index),
                source,
            ));
        }
        routed.push(RoutedEdge { from: edge.from.clone(), to: edge.to.clone(), points, label, label_box, style: edge.style });
    }

    // Routing stage 2: edge labels are obstacles for other edges. An edge
    // crossing a foreign label box is re-routed with that box included; its
    // own label box follows the final polyline midpoint.
    for index in 0..routed.len() {
        let foreign_labels: Vec<Rect> = routed
            .iter()
            .enumerate()
            .filter_map(|(other, edge)| (other != index).then_some(edge.label_box?).map(|rect| inflate(rect, ROUTE_LABEL_CLEARANCE)))
            .collect();
        if foreign_labels.is_empty() || !route_hits_obstacles(&routed[index].points, &foreign_labels) {
            continue;
        }
        let from = routed[index].points[0];
        let to = *routed[index].points.last().expect("non-empty route");
        let mut obstacles = inflated_node_obstacles(&rects, edges[index].from_index, edges[index].to_index);
        obstacles.extend(foreign_labels.iter().copied());
        if let Some(points) = route_avoiding(from, to, spec.direction, &obstacles, &mut lanes) {
            let label_box = routed[index]
                .label
                .as_ref()
                .map(|label| estimate_label_box(label, polyline_midpoint(&points), edge_label_font_size(options.font_size)));
            routed[index].points = points;
            routed[index].label_box = label_box;
        }
        else {
            diagnostics.push(located(
                codes::APPROXIMATED,
                format!(
                    "edge `{}->{}` crosses a foreign edge label and found no clearing channel",
                    routed[index].from.node, routed[index].to.node
                ),
                source.edge(edges[index].spec_index),
                source,
            ));
        }
    }

    // Routing stage 3: converging edges share one track in front of the
    // target. A vertical (or horizontal for sideways layouts) final segment
    // is split at `ROUTE_MERGE_STUB` before the target so parallel approaches
    // visibly join.
    merge_converging_tracks(&mut routed, spec.direction);

    let placed_nodes = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| PlacedNode {
            id: node.id.clone(),
            label: resolved_label(node),
            shape: node.shape,
            rect: rects[index],
            cluster: node
                .cluster
                .clone()
                .filter(|id| clusters.iter().any(|cluster| &cluster.id == id))
                .or_else(|| memberships[index].first().map(|&cluster_index| clusters[cluster_index].id.clone())),
            ports: ports[index].clone(),
        })
        .collect();

    let placed_clusters = clusters
        .iter()
        .enumerate()
        .filter_map(|(index, cluster)| {
            cluster_rects[index].map(|rect| PlacedCluster { id: cluster.id.clone(), label: cluster.label.clone(), rect })
        })
        .collect();

    DiagramLayoutPlan {
        layout: PlanLayout::Layered,
        canvas,
        nodes: placed_nodes,
        edges: routed,
        clusters: placed_clusters,
        activations: Vec::new(),
        diagnostics,
    }
}

/// Compile a sequence diagram into a deterministic time-ordered layout plan.
///
/// A sequence spec reuses the graph types: nodes are participants and edges are
/// messages. [`DiagramSpec::direction`] is ignored because participants always
/// advance to the right and messages advance downwards in source order. Every
/// participant header is measured from its label, every message occupies one
/// row, and the canvas covers all columns and rows plus the configured margin.
/// Lifelines are not stored in the plan: [`crate::render::render_sequence`]
/// derives them from each participant rectangle and the canvas, so the plan
/// stays a pure geometry result.
///
/// Degradation is explicit and never silent:
///
/// - A duplicate participant id keeps the first declaration with a diagnostic.
/// - A message that references an unknown participant is skipped with a
///   diagnostic.
/// - A self message is skipped with a diagnostic because the row layout has no
///   loop route.
/// - A dotted message is drawn as a solid line because the scene style has no
///   dash surface, with an approximation diagnostic.
/// - Cluster and ordering-constraint declarations are ignored with a diagnostic
///   because sequence rows come from source position.
pub fn compile_sequence(spec: &DiagramSpec, options: &LayoutOptions) -> DiagramLayoutPlan {
    compile_sequence_with_source(spec, options, &SourceMap::default())
}

/// Compile a sequence spec and locate diagnostics through a [`SourceMap`].
pub fn compile_sequence_with_source(spec: &DiagramSpec, options: &LayoutOptions, source: &SourceMap) -> DiagramLayoutPlan {
    let sequence = match &spec.layout {
        LayoutModel::TimeOrdered(sequence) => sequence.clone(),
        LayoutModel::Layered => SequenceLayout {
            participants: spec.nodes.iter().map(|node| node.id.clone()).collect(),
            messages: (0..spec.edges.len()).collect(),
            activations: Vec::new(),
        },
    };
    compile_time_ordered(spec, &sequence, options, source)
}

fn compile_time_ordered(spec: &DiagramSpec, sequence: &SequenceLayout, options: &LayoutOptions, source: &SourceMap) -> DiagramLayoutPlan {
    let mut diagnostics = Vec::new();
    if !spec.clusters.is_empty() {
        diagnostics.push(warning(codes::UNSUPPORTED, format!("sequence layout ignores {} cluster declarations", spec.clusters.len())));
    }
    if !spec.constraints.is_empty() {
        diagnostics.push(warning(codes::UNSUPPORTED, format!("sequence layout ignores {} ordering constraints", spec.constraints.len())));
    }

    let axis = declared_axis(spec, sequence, &mut diagnostics);
    let mut participants: Vec<ResolvedNode> = Vec::with_capacity(axis.len());
    let mut indices: BTreeMap<String, usize> = BTreeMap::new();
    for spec_index in axis {
        let node = &spec.nodes[spec_index];
        if node.id.is_empty() {
            diagnostics.push(located(
                codes::UNSUPPORTED,
                "sequence participant with an empty id is ignored",
                source.node(spec_index),
                source,
            ));
            continue;
        }
        if indices.contains_key(&node.id) {
            diagnostics.push(located(
                codes::UNSUPPORTED,
                format!("duplicate participant id `{}`, the first declaration is kept", node.id),
                source.node(spec_index),
                source,
            ));
            continue;
        }
        indices.insert(node.id.clone(), participants.len());
        participants.push(ResolvedNode {
            spec_index,
            id: node.id.clone(),
            label: node.label.clone(),
            shape: node.shape,
            ports: Vec::new(),
            cluster: None,
        });
    }

    if participants.is_empty() {
        return DiagramLayoutPlan {
            layout: PlanLayout::TimeOrdered,
            canvas: Size::ZERO,
            nodes: Vec::new(),
            edges: Vec::new(),
            clusters: Vec::new(),
            activations: Vec::new(),
            diagnostics,
        };
    }

    let sizes: Vec<Size> = participants.iter().map(|participant| node_size(participant, options)).collect();
    let header_height = sizes.iter().map(|size| size.height).fold(0.0, f64::max);
    let mut cursor = options.margin;
    let mut rects = Vec::with_capacity(participants.len());
    for size in &sizes {
        rects.push(Rect::new(cursor, options.margin, size.width, size.height));
        cursor += size.width + options.node_gap;
    }
    let right = cursor - options.node_gap;
    let header_bottom = options.margin + header_height;
    let row_height = options.font_size * 1.4 + options.padding_y * 2.0;
    let first_row = header_bottom + options.layer_gap;

    let message_indices = declared_messages(spec, sequence, &mut diagnostics);
    let mut messages: Vec<RoutedEdge> = Vec::with_capacity(spec.edges.len());
    let mut row_centers: Vec<f64> = Vec::with_capacity(spec.edges.len());
    for edge_index in message_indices {
        let edge = &spec.edges[edge_index];
        let (Some(&from_index), Some(&to_index)) = (indices.get(&edge.from.node), indices.get(&edge.to.node))
        else {
            let missing = if indices.contains_key(&edge.from.node) { edge.to.node.as_str() } else { edge.from.node.as_str() };
            diagnostics.push(located(
                codes::UNSUPPORTED,
                format!("sequence message `{}->{}` references unknown participant `{missing}` and is skipped", edge.from.node, edge.to.node),
                source.edge(edge_index),
                source,
            ));
            continue;
        };
        if from_index == to_index {
            diagnostics.push(located(
                codes::UNSUPPORTED,
                format!("self sequence message `{}->{}` is not routed and is skipped", edge.from.node, edge.to.node),
                source.edge(edge_index),
                source,
            ));
            continue;
        }
        let line_y = first_row + row_height * (messages.len() as f64 + 0.5);
        row_centers.push(line_y);
        let from = Point::new(rects[from_index].center().x, line_y);
        let to = Point::new(rects[to_index].center().x, line_y);
        let label = edge.label.as_deref().and_then(|label| {
            prepare_edge_label(label, &edge.from.node, &edge.to.node, edge_label_font_size(options.font_size), &mut diagnostics)
        });
        let label_box = label.as_ref().map(|label| {
            estimate_label_box(label, Point::new((from.x + to.x) * 0.5, line_y - options.font_size), edge_label_font_size(options.font_size))
        });
        if matches!(edge.style, EdgeStyle::DottedArrow | EdgeStyle::DottedLine) {
            diagnostics.push(located(
                codes::APPROXIMATED,
                format!(
                    "dotted sequence message `{}->{}` is rendered as a solid line because the scene style has no dash surface",
                    edge.from.node, edge.to.node
                ),
                source.edge(edge_index),
                source,
            ));
        }
        messages.push(RoutedEdge { from: edge.from.clone(), to: edge.to.clone(), points: vec![from, to], label, label_box, style: edge.style });
    }

    let mut activations = Vec::new();
    for activation in &sequence.activations {
        let Some(&participant_index) = indices.get(&activation.participant)
        else {
            diagnostics.push(warning(codes::UNSUPPORTED, format!("activation on unknown participant `{}` is ignored", activation.participant)));
            continue;
        };
        if activation.from_message > activation.to_message || activation.to_message >= row_centers.len() {
            diagnostics.push(warning(
                codes::UNSUPPORTED,
                format!(
                    "activation on `{}` spans messages {}..={} outside the {} routed rows and is ignored",
                    activation.participant,
                    activation.from_message,
                    activation.to_message,
                    row_centers.len()
                ),
            ));
            continue;
        }
        let center = rects[participant_index].center().x;
        let top = row_centers[activation.from_message] - row_height * 0.5;
        let bottom = row_centers[activation.to_message] + row_height * 0.5;
        activations.push(PlacedActivation {
            participant: activation.participant.clone(),
            rect: Rect::new(center - ACTIVATION_WIDTH * 0.5, top, ACTIVATION_WIDTH, bottom - top),
        });
    }

    let content_bottom = first_row + row_height * messages.len() as f64;
    let canvas = Size::new(right + options.margin, content_bottom + options.margin);
    let placed_nodes = participants
        .iter()
        .enumerate()
        .map(|(index, participant)| PlacedNode {
            id: participant.id.clone(),
            label: resolved_label(participant),
            shape: participant.shape,
            rect: rects[index],
            cluster: None,
            ports: BTreeMap::new(),
        })
        .collect();

    DiagramLayoutPlan {
        layout: PlanLayout::TimeOrdered,
        canvas,
        nodes: placed_nodes,
        edges: messages,
        clusters: Vec::new(),
        activations,
        diagnostics,
    }
}

/// Participant indices in the declared axis order.
///
/// A declaration that names an unknown id reports it and otherwise keeps every
/// node: a missing participant means the declaration is stale, and dropping a
/// whole lifeline would hide it.
///
/// The axis keeps every declaration, including a repeated one. Collapsing a
/// duplicate here would consume the second token and leave the duplicate
/// diagnostic pointing at the first declaration, so the caller reports it
/// while it owns the right spec index.
fn declared_axis(spec: &DiagramSpec, sequence: &SequenceLayout, diagnostics: &mut Vec<Diagnostic>) -> Vec<usize> {
    if sequence.participants.is_empty() {
        return (0..spec.nodes.len()).collect();
    }
    let mut ordered: Vec<usize> = Vec::with_capacity(spec.nodes.len());
    let mut consumed: BTreeSet<usize> = BTreeSet::new();
    for id in &sequence.participants {
        match (0..spec.nodes.len()).find(|&index| &spec.nodes[index].id == id && !consumed.contains(&index)) {
            Some(index) => {
                consumed.insert(index);
                ordered.push(index);
            }
            None => diagnostics.push(warning(codes::UNSUPPORTED, format!("sequence axis names unknown participant `{id}` and is ignored"))),
        }
    }
    ordered.extend((0..spec.nodes.len()).filter(|&index| !consumed.contains(&index)));
    ordered
}

/// Message indices in the declared top-to-bottom order.
fn declared_messages(spec: &DiagramSpec, sequence: &SequenceLayout, diagnostics: &mut Vec<Diagnostic>) -> Vec<usize> {
    if sequence.messages.is_empty() {
        return (0..spec.edges.len()).collect();
    }
    let mut ordered: Vec<usize> = Vec::with_capacity(spec.edges.len());
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    for &index in &sequence.messages {
        if index >= spec.edges.len() {
            diagnostics.push(warning(
                codes::UNSUPPORTED,
                format!("sequence message index {index} is outside the {} declared edges and is ignored", spec.edges.len()),
            ));
        }
        else if seen.insert(index) {
            ordered.push(index);
        }
    }
    ordered.extend((0..spec.edges.len()).filter(|index| !seen.contains(index)));
    ordered
}

fn resolve_nodes(spec: &DiagramSpec, diagnostics: &mut Vec<Diagnostic>, source: &SourceMap) -> Vec<ResolvedNode> {
    let mut nodes: Vec<ResolvedNode> = Vec::with_capacity(spec.nodes.len());
    let mut indices: BTreeMap<String, usize> = BTreeMap::new();

    for (spec_index, node) in spec.nodes.iter().enumerate() {
        if node.id.is_empty() {
            diagnostics.push(located(codes::UNSUPPORTED, "node with an empty id is ignored", source.node(spec_index), source));
            continue;
        }
        match indices.get(&node.id) {
            Some(&index) => {
                diagnostics.push(located(
                    codes::UNSUPPORTED,
                    format!("duplicate node id `{}`, the first definition is kept and later port declarations are merged", node.id),
                    source.node(spec_index),
                    source,
                ));
                nodes[index].ports.extend(node.ports.iter().cloned());
            }
            None => {
                indices.insert(node.id.clone(), nodes.len());
                nodes.push(ResolvedNode {
                    spec_index,
                    id: node.id.clone(),
                    label: node.label.clone(),
                    shape: node.shape,
                    ports: node.ports.clone(),
                    cluster: node.cluster.clone(),
                });
            }
        }
    }

    nodes
}

fn resolve_clusters(
    spec: &DiagramSpec,
    nodes: &[ResolvedNode],
    index_by_id: &BTreeMap<String, usize>,
    diagnostics: &mut Vec<Diagnostic>,
    source: &SourceMap,
) -> Vec<ResolvedCluster> {
    let mut clusters: Vec<ResolvedCluster> = Vec::with_capacity(spec.clusters.len());
    let mut declared: BTreeSet<String> = BTreeSet::new();

    for cluster in &spec.clusters {
        if !declared.insert(cluster.id.clone()) {
            diagnostics.push(warning(codes::UNSUPPORTED, format!("duplicate cluster id `{}`, later definitions are ignored", cluster.id)));
            continue;
        }

        let mut member_flags = vec![false; nodes.len()];
        for member in &cluster.members {
            match index_by_id.get(member) {
                Some(&index) => member_flags[index] = true,
                None => diagnostics.push(warning(
                    codes::UNSUPPORTED,
                    format!("cluster `{}` references unknown node `{member}`, the reference is ignored", cluster.id),
                )),
            }
        }
        for (index, node) in nodes.iter().enumerate() {
            if node.cluster.as_deref() == Some(cluster.id.as_str()) {
                member_flags[index] = true;
            }
        }

        let members: Vec<usize> = member_flags.iter().enumerate().filter_map(|(index, &present)| present.then_some(index)).collect();
        if members.is_empty() {
            diagnostics.push(warning(codes::UNSUPPORTED, format!("cluster `{}` has no members and produces no frame", cluster.id)));
            continue;
        }
        clusters.push(ResolvedCluster { id: cluster.id.clone(), label: cluster.label.clone(), members });
    }

    for node in nodes {
        if let Some(cluster) = &node.cluster {
            if !declared.contains(cluster) {
                diagnostics.push(located(
                    codes::UNSUPPORTED,
                    format!("node `{}` references unknown cluster `{cluster}`, the reference is ignored", node.id),
                    source.node(node.spec_index),
                    source,
                ));
            }
        }
    }

    let memberships = node_memberships(&clusters, nodes.len());
    for (index, members) in memberships.iter().enumerate() {
        if members.len() > 1 {
            diagnostics.push(located(
                codes::UNSUPPORTED,
                format!("node `{}` belongs to multiple clusters, the first declared frame is used", nodes[index].id),
                source.node(nodes[index].spec_index),
                source,
            ));
        }
    }

    clusters
}

fn resolve_edges(
    spec: &DiagramSpec,
    index_by_id: &BTreeMap<String, usize>,
    diagnostics: &mut Vec<Diagnostic>,
    source: &SourceMap,
) -> Vec<ResolvedEdge> {
    let mut edges: Vec<ResolvedEdge> = Vec::with_capacity(spec.edges.len());

    for (spec_index, edge) in spec.edges.iter().enumerate() {
        let span = source.edge(spec_index);
        let from_index = index_by_id.get(&edge.from.node).copied();
        let to_index = index_by_id.get(&edge.to.node).copied();
        let (from_index, to_index) = match (from_index, to_index) {
            (Some(from), Some(to)) => (from, to),
            (None, _) => {
                diagnostics.push(located(
                    codes::UNSUPPORTED,
                    format!("edge `{}->{}` references unknown node `{}` and is skipped", edge.from.node, edge.to.node, edge.from.node),
                    span,
                    source,
                ));
                continue;
            }
            (_, None) => {
                diagnostics.push(located(
                    codes::UNSUPPORTED,
                    format!("edge `{}->{}` references unknown node `{}` and is skipped", edge.from.node, edge.to.node, edge.to.node),
                    span,
                    source,
                ));
                continue;
            }
        };

        if from_index == to_index {
            diagnostics.push(located(
                codes::UNSUPPORTED,
                format!("self edge `{}->{}` is not routed and is skipped", edge.from.node, edge.to.node),
                span,
                source,
            ));
            continue;
        }

        edges.push(ResolvedEdge {
            spec_index,
            from: edge.from.clone(),
            to: edge.to.clone(),
            from_index,
            to_index,
            label: edge.label.clone(),
            style: edge.style,
        });
    }

    edges
}

fn assign_layers(
    nodes: &[ResolvedNode],
    edges: &[ResolvedEdge],
    constraints: &[Constraint],
    index_by_id: &BTreeMap<String, usize>,
    diagnostics: &mut Vec<Diagnostic>,
    source: &SourceMap,
) -> Vec<usize> {
    let count = nodes.len();
    let mut successors = vec![Vec::new(); count];
    let mut indegree = vec![0usize; count];
    for edge in edges {
        successors[edge.from_index].push(edge.to_index);
        indegree[edge.to_index] += 1;
    }

    let mut layer = vec![0usize; count];
    let mut resolved = vec![false; count];
    let mut order = Vec::with_capacity(count);
    let mut queue: VecDeque<usize> = (0..count).filter(|index| indegree[*index] == 0).collect();
    while let Some(node) = queue.pop_front() {
        resolved[node] = true;
        order.push(node);
        for &successor in &successors[node] {
            layer[successor] = layer[successor].max(layer[node] + 1);
            indegree[successor] -= 1;
            if indegree[successor] == 0 {
                queue.push_back(successor);
            }
        }
    }

    let mut cycle = vec![false; count];
    let mut cycle_nodes = Vec::new();
    for node in 0..count {
        if resolved[node] {
            continue;
        }
        cycle[node] = true;
        cycle_nodes.push(node);
        let mut best = layer[node];
        for edge in edges {
            if edge.to_index == node {
                best = best.max(layer[edge.from_index] + 1);
            }
        }
        layer[node] = best;
        resolved[node] = true;
    }

    if !cycle_nodes.is_empty() {
        // After the repair pass, an edge whose source already sits on the
        // target layer or later is the one the layered model cannot honour.
        // Naming those edges turns "there is a cycle somewhere" into a
        // locatable statement.
        let reversed: Vec<&ResolvedEdge> = edges.iter().filter(|edge| layer[edge.from_index] >= layer[edge.to_index]).collect();
        let message = if reversed.is_empty() {
            let ids: Vec<&str> = cycle_nodes.iter().map(|&index| nodes[index].id.as_str()).collect();
            format!("diagram contains a cycle involving nodes {ids:?}. Layered placement breaks it in declaration order")
        }
        else {
            let pairs: Vec<String> = reversed.iter().map(|edge| format!("{}->{}", edge.from.node, edge.to.node)).collect();
            format!(
                "diagram contains a cycle closed by edge {pairs:?}, which no longer advances to a later layer and is drawn in declaration order"
            )
        };
        let span = reversed.iter().find_map(|edge| source.edge(edge.spec_index));
        diagnostics.push(located(codes::APPROXIMATED, message, span, source));
    }

    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut precedences: Vec<(usize, usize)> = Vec::new();
    for constraint in constraints {
        match constraint {
            Constraint::SameRank(ids) => {
                let mut group = Vec::new();
                for id in ids {
                    match index_by_id.get(id) {
                        Some(&index) => {
                            if !group.contains(&index) {
                                group.push(index);
                            }
                        }
                        None => diagnostics.push(warning(
                            codes::UNSUPPORTED,
                            format!("same-rank constraint references unknown node `{id}`, the reference is ignored"),
                        )),
                    }
                }
                if group.is_empty() {
                    diagnostics.push(warning(codes::UNSUPPORTED, "same-rank constraint references no known nodes and is ignored"));
                }
                else {
                    groups.push(group);
                }
            }
            Constraint::PlaceBefore { before, after } => match (index_by_id.get(before), index_by_id.get(after)) {
                (Some(&before_index), Some(&after_index)) => {
                    if before_index == after_index {
                        diagnostics.push(warning(
                            codes::UNSUPPORTED,
                            format!("place-before constraint relates node `{before}` to itself and is ignored"),
                        ));
                    }
                    else {
                        precedences.push((before_index, after_index));
                    }
                }
                (None, _) => diagnostics.push(warning(
                    codes::UNSUPPORTED,
                    format!("place-before constraint references unknown node `{before}`, the constraint is ignored"),
                )),
                (_, None) => diagnostics.push(warning(
                    codes::UNSUPPORTED,
                    format!("place-before constraint references unknown node `{after}`, the constraint is ignored"),
                )),
            },
        }
    }

    apply_same_rank(&mut layer, &groups);

    if !precedences.is_empty() {
        let mut blocked = vec![false; precedences.len()];
        let passes = precedences.len() + 1;
        for pass in 0..passes {
            let mut changed = false;
            for (index, &(before, after)) in precedences.iter().enumerate() {
                if blocked[index] || layer[after] > layer[before] {
                    continue;
                }
                if reachable(&successors, after, before) {
                    blocked[index] = true;
                    diagnostics.push(warning(
                        codes::UNSUPPORTED,
                        format!(
                            "place-before constraint between `{}` and `{}` conflicts with edge order and is ignored",
                            nodes[before].id, nodes[after].id
                        ),
                    ));
                    continue;
                }
                layer[after] = layer[before] + 1;
                changed = true;
            }
            if !changed {
                break;
            }
            if pass + 1 == passes {
                diagnostics.push(warning(
                    codes::APPROXIMATED,
                    "layout ordering constraints do not converge and were applied for a bounded number of passes",
                ));
            }
        }
    }

    for &node in &order {
        for &successor in &successors[node] {
            if cycle[successor] {
                continue;
            }
            layer[successor] = layer[successor].max(layer[node] + 1);
        }
    }

    apply_same_rank(&mut layer, &groups);

    layer
}

fn apply_same_rank(layer: &mut [usize], groups: &[Vec<usize>]) {
    for group in groups {
        let max = group.iter().map(|&index| layer[index]).max().unwrap_or(0);
        for &index in group {
            layer[index] = max;
        }
    }
}

fn reachable(successors: &[Vec<usize>], from: usize, to: usize) -> bool {
    let mut visited = vec![false; successors.len()];
    let mut stack = vec![from];
    visited[from] = true;
    while let Some(node) = stack.pop() {
        for &next in &successors[node] {
            if next == to {
                return true;
            }
            if !visited[next] {
                visited[next] = true;
                stack.push(next);
            }
        }
    }
    false
}

fn node_size(node: &ResolvedNode, options: &LayoutOptions) -> Size {
    label_size(&resolved_label(node), node.shape, options)
}

fn label_size(label: &str, shape: NodeShape, options: &LayoutOptions) -> Size {
    let lines: Vec<&str> = label.split('\n').collect();
    let widest = lines.iter().map(|line| measure_line(line, options.font_size)).fold(0.0, f64::max);
    let width = (widest + options.padding_x * 2.0).max(options.min_node_width);
    let height = (lines.len() as f64 * options.font_size * 1.4 + options.padding_y * 2.0).max(options.min_node_height);
    if matches!(shape, NodeShape::Circle) {
        let side = width.max(height);
        Size::new(side, side)
    }
    else {
        Size::new(width, height)
    }
}

fn resolved_label(node: &ResolvedNode) -> String {
    node.label.as_ref().filter(|label| !label.trim().is_empty()).cloned().unwrap_or_else(|| node.id.clone())
}

fn place_rects(sizes: &[Size], layers: &[usize], direction: Direction, options: &LayoutOptions) -> Vec<Rect> {
    let layer_count = layers.iter().copied().max().map(|max| max + 1).unwrap_or(0);
    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); layer_count];
    for (index, &layer) in layers.iter().enumerate() {
        groups[layer].push(index);
    }

    let horizontal = matches!(direction, Direction::LeftToRight | Direction::RightToLeft);
    let reversed = matches!(direction, Direction::BottomUp | Direction::RightToLeft);
    let primary = |index: usize| if horizontal { sizes[index].width } else { sizes[index].height };
    let cross = |index: usize| if horizontal { sizes[index].height } else { sizes[index].width };

    let mut primary_extents = Vec::with_capacity(groups.len());
    let mut cross_extents = Vec::with_capacity(groups.len());
    for group in &groups {
        let primary_extent = group.iter().map(|&index| primary(index)).fold(0.0, f64::max);
        let cross_extent = group.iter().map(|&index| cross(index)).sum::<f64>() + options.node_gap * group.len().saturating_sub(1) as f64;
        primary_extents.push(primary_extent);
        cross_extents.push(cross_extent);
    }

    let mut primary_centers = vec![0.0; groups.len()];
    let mut cursor = 0.0;
    for (index, extent) in primary_extents.iter().enumerate() {
        primary_centers[index] = cursor + extent * 0.5;
        cursor += extent + options.layer_gap;
    }
    let total_primary = cursor - options.layer_gap;

    let mut rects = vec![Rect::default(); sizes.len()];
    for (group_index, group) in groups.iter().enumerate() {
        let primary_center = if reversed { total_primary - primary_centers[group_index] } else { primary_centers[group_index] };
        let mut cross_cursor = -cross_extents[group_index] * 0.5;
        for &index in group {
            let cross_center = cross_cursor + cross(index) * 0.5;
            cross_cursor += cross(index) + options.node_gap;
            let (x, y) = if horizontal {
                (primary_center - sizes[index].width * 0.5, cross_center - sizes[index].height * 0.5)
            }
            else {
                (cross_center - sizes[index].width * 0.5, primary_center - sizes[index].height * 0.5)
            };
            rects[index] = Rect::new(x, y, sizes[index].width, sizes[index].height);
        }
    }

    rects
}

fn canvas_shift(rects: &[Rect], cluster_rects: &[Option<Rect>], options: &LayoutOptions) -> (f64, f64, Size) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for rect in rects {
        min_x = min_x.min(rect.x);
        min_y = min_y.min(rect.y);
        max_x = max_x.max(rect.right());
        max_y = max_y.max(rect.bottom());
    }
    for rect in cluster_rects.iter().flatten() {
        min_x = min_x.min(rect.x);
        min_y = min_y.min(rect.y);
        max_x = max_x.max(rect.right());
        max_y = max_y.max(rect.bottom());
    }

    if !min_x.is_finite() || !min_y.is_finite() {
        return (0.0, 0.0, Size::ZERO);
    }

    let shift_x = options.margin - min_x;
    let shift_y = options.margin - min_y;
    let canvas = Size::new(max_x - min_x + options.margin * 2.0, max_y - min_y + options.margin * 2.0);
    (shift_x, shift_y, canvas)
}

fn build_ports(node: &ResolvedNode, rect: Rect, diagnostics: &mut Vec<Diagnostic>, source: &SourceMap) -> BTreeMap<String, Point> {
    let mut ports = BTreeMap::new();
    for port in &node.ports {
        if port.id.is_empty() {
            diagnostics.push(located(
                codes::UNSUPPORTED,
                format!("node `{}` declares a port with an empty id, the declaration is ignored", node.id),
                source.node(node.spec_index),
                source,
            ));
            continue;
        }
        if ports.contains_key(&port.id) {
            diagnostics.push(located(
                codes::UNSUPPORTED,
                format!("node `{}` declares duplicate port id `{}`, the first declaration is kept", node.id, port.id),
                source.node(node.spec_index),
                source,
            ));
            continue;
        }
        let offset = if !port.offset.is_finite() {
            diagnostics.push(located(
                codes::APPROXIMATED,
                format!("node `{}` declares a port offset that is not finite, `0.0` is used", node.id),
                source.node(node.spec_index),
                source,
            ));
            0.0
        }
        else if !(0.0..=1.0).contains(&port.offset) {
            diagnostics.push(located(
                codes::APPROXIMATED,
                format!("port `{}` on node `{}` has an offset outside `0.0..=1.0` and is clamped", port.id, node.id),
                source.node(node.spec_index),
                source,
            ));
            port.offset.clamp(0.0, 1.0)
        }
        else {
            port.offset
        };
        ports.insert(port.id.clone(), port_anchor(rect, port.side, offset));
    }
    ports
}

fn port_anchor(rect: Rect, side: Side, offset: f64) -> Point {
    match side {
        Side::Top => Point::new(rect.x + rect.width * offset, rect.y),
        Side::Right => Point::new(rect.right(), rect.y + rect.height * offset),
        Side::Bottom => Point::new(rect.x + rect.width * offset, rect.bottom()),
        Side::Left => Point::new(rect.x, rect.y + rect.height * offset),
    }
}

fn default_anchor(rect: Rect, direction: Direction, is_source: bool) -> Point {
    let center = rect.center();
    match (direction, is_source) {
        (Direction::TopDown, true) => Point::new(center.x, rect.bottom()),
        (Direction::TopDown, false) => Point::new(center.x, rect.y),
        (Direction::BottomUp, true) => Point::new(center.x, rect.y),
        (Direction::BottomUp, false) => Point::new(center.x, rect.bottom()),
        (Direction::LeftToRight, true) => Point::new(rect.right(), center.y),
        (Direction::LeftToRight, false) => Point::new(rect.x, center.y),
        (Direction::RightToLeft, true) => Point::new(rect.x, center.y),
        (Direction::RightToLeft, false) => Point::new(rect.right(), center.y),
    }
}

struct EndpointResolver<'a> {
    nodes: &'a [ResolvedNode],
    rects: &'a [Rect],
    ports: &'a [BTreeMap<String, Point>],
    direction: Direction,
    reported_ports: BTreeSet<(String, String)>,
}

impl EndpointResolver<'_> {
    fn resolve(
        &mut self,
        endpoint: &Endpoint,
        node_index: usize,
        is_source: bool,
        diagnostics: &mut Vec<Diagnostic>,
        span: Option<Range<usize>>,
        source: &SourceMap,
    ) -> Point {
        let rect = self.rects[node_index];
        match endpoint.port.as_deref() {
            None => default_anchor(rect, self.direction, is_source),
            Some(port_id) => match self.ports[node_index].get(port_id) {
                Some(point) => *point,
                None => {
                    let node_id = &self.nodes[node_index].id;
                    if self.reported_ports.insert((node_id.clone(), port_id.to_string())) {
                        diagnostics.push(located(
                            codes::APPROXIMATED,
                            format!(
                                "edge endpoint references unknown port `{port_id}` on node `{node_id}`, the default anchor for the layout direction is used"
                            ),
                            span,
                            source,
                        ));
                    }
                    default_anchor(rect, self.direction, is_source)
                }
            },
        }
    }
}

fn route(from: Point, to: Point, direction: Direction) -> Vec<Point> {
    let points = match direction {
        Direction::LeftToRight | Direction::RightToLeft => {
            let middle_x = (from.x + to.x) * 0.5;
            vec![from, Point::new(middle_x, from.y), Point::new(middle_x, to.y), to]
        }
        Direction::TopDown | Direction::BottomUp => {
            let middle_y = (from.y + to.y) * 0.5;
            vec![from, Point::new(from.x, middle_y), Point::new(to.x, middle_y), to]
        }
    };
    dedupe_points(points)
}

/// Inflation of a node rectangle when it acts as a routing obstacle: half the
/// node stroke width plus the minimum edge-to-node gap.
const ROUTE_NODE_CLEARANCE: f64 = 4.75;
/// Inflation of a foreign edge label box when it acts as a routing obstacle.
const ROUTE_LABEL_CLEARANCE: f64 = 4.0;
/// Extra lateral clearance of a candidate lane from an inflated obstacle.
const ROUTE_LANE_CLEARANCE: f64 = 6.0;
/// Length of the straight stub leaving the source anchor on a detour.
const ROUTE_STUB: f64 = 12.0;
/// Distance in front of the target anchor where a detour rejoins and
/// converging edges share one track.
const ROUTE_MERGE_STUB: f64 = 16.0;
/// Minimum distance between two parallel detour lanes with overlapping spans.
const ROUTE_CHANNEL_SPACING: f64 = 12.0;

/// A detour lane already claimed by an earlier edge: the lane coordinate
/// (x for vertical lanes, y for horizontal ones) plus its span on the
/// routing axis.
#[derive(Clone, Copy, Debug)]
struct UsedLane {
    coordinate: f64,
    low: f64,
    high: f64,
}

fn inflate(rect: Rect, amount: f64) -> Rect {
    Rect::new(rect.x - amount, rect.y - amount, rect.width + amount * 2.0, rect.height + amount * 2.0)
}

fn inflated_node_obstacles(rects: &[Rect], from_index: usize, to_index: usize) -> Vec<Rect> {
    rects
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != from_index && *index != to_index)
        .map(|(_, rect)| inflate(*rect, ROUTE_NODE_CLEARANCE))
        .collect()
}

/// Orthogonal segment vs rectangle interior. Degenerate segments count as a
/// point test. Non-orthogonal segments never occur in our routes; they are
/// treated as hitting to stay conservative.
fn segment_hits_rect(a: Point, b: Point, rect: &Rect) -> bool {
    let dx = (a.x - b.x).abs();
    let dy = (a.y - b.y).abs();
    if dx < f64::EPSILON && dy < f64::EPSILON {
        return a.x > rect.x && a.x < rect.right() && a.y > rect.y && a.y < rect.bottom();
    }
    if dy < f64::EPSILON {
        if a.y <= rect.y || a.y >= rect.bottom() {
            return false;
        }
        let (low, high) = if a.x <= b.x { (a.x, b.x) } else { (b.x, a.x) };
        return high > rect.x && low < rect.right();
    }
    if dx < f64::EPSILON {
        if a.x <= rect.x || a.x >= rect.right() {
            return false;
        }
        let (low, high) = if a.y <= b.y { (a.y, b.y) } else { (b.y, a.y) };
        return high > rect.y && low < rect.bottom();
    }
    true
}

fn route_hits_obstacles(points: &[Point], obstacles: &[Rect]) -> bool {
    points.windows(2).any(|segment| obstacles.iter().any(|rect| segment_hits_rect(segment[0], segment[1], rect)))
}

/// Route an edge around the given obstacles. Returns the direct orthogonal
/// route when it is already clean, otherwise the nearest channel lane whose
/// detour clears every obstacle, or `None` when no candidate lane works.
///
/// Lanes claimed by earlier edges are spaced at least
/// [`ROUTE_CHANNEL_SPACING`] apart so parallel detours never overlap.
fn route_avoiding(from: Point, to: Point, direction: Direction, obstacles: &[Rect], lanes: &mut Vec<UsedLane>) -> Option<Vec<Point>> {
    let direct = route(from, to, direction);
    if !route_hits_obstacles(&direct, obstacles) {
        return Some(direct);
    }
    let vertical = matches!(direction, Direction::TopDown | Direction::BottomUp);
    let (start, end, start_cross, end_cross) = if vertical { (from.y, to.y, from.x, to.x) } else { (from.x, to.x, from.y, to.y) };
    let sign = if end >= start { 1.0 } else { -1.0 };
    let (span_low, span_high) = if start <= end { (start, end) } else { (end, start) };

    // Candidate lanes come from the lateral edges of every obstacle that the
    // direct route crosses, extended by the lane clearance.
    let mut candidates: Vec<f64> = obstacles
        .iter()
        .flat_map(|rect| {
            let (rect_low, rect_high, rect_cross_low, rect_high_cross) =
                if vertical { (rect.y, rect.bottom(), rect.x, rect.right()) } else { (rect.x, rect.right(), rect.y, rect.bottom()) };
            if rect_high > span_low && rect_low < span_high {
                vec![rect_cross_low - ROUTE_LANE_CLEARANCE, rect_high_cross + ROUTE_LANE_CLEARANCE]
            }
            else {
                Vec::new()
            }
        })
        .collect();
    let reference = (start_cross + end_cross) * 0.5;
    candidates.sort_by(|left, right| {
        (left - reference).abs().partial_cmp(&(right - reference).abs()).unwrap_or(std::cmp::Ordering::Equal).then(left.total_cmp(right))
    });
    candidates.dedup();

    for candidate in candidates {
        // Parallel detours with overlapping spans keep the channel spacing.
        let mut lane = candidate;
        loop {
            let conflict =
                lanes.iter().any(|used| used.high > span_low && used.low < span_high && (used.coordinate - lane).abs() < ROUTE_CHANNEL_SPACING);
            if !conflict {
                break;
            }
            let direction_away = if lane >= reference { 1.0 } else { -1.0 };
            lane += direction_away * ROUTE_CHANNEL_SPACING;
        }
        let stub = start + sign * ROUTE_STUB;
        let rejoin = end - sign * ROUTE_MERGE_STUB;
        let (stub, rejoin) = if sign * (rejoin - stub) <= 0.0 {
            let middle = (start + end) * 0.5;
            (middle, middle)
        }
        else {
            (stub, rejoin)
        };
        let points = if vertical {
            vec![from, Point::new(start_cross, stub), Point::new(lane, stub), Point::new(lane, rejoin), Point::new(end_cross, rejoin), to]
        }
        else {
            vec![from, Point::new(stub, start_cross), Point::new(stub, lane), Point::new(rejoin, lane), Point::new(rejoin, end_cross), to]
        };
        let points = dedupe_points(points);
        if !route_hits_obstacles(&points, obstacles) {
            lanes.push(UsedLane { coordinate: lane, low: span_low, high: span_high });
            return Some(points);
        }
    }
    None
}

/// Split the final approach of converging edges at [`ROUTE_MERGE_STUB`] in
/// front of the target so parallel tracks share one visible join segment.
fn merge_converging_tracks(routed: &mut [RoutedEdge], direction: Direction) {
    let vertical = matches!(direction, Direction::TopDown | Direction::BottomUp);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for edge in routed.iter() {
        *counts.entry(edge.to.node.clone()).or_default() += 1;
    }
    for edge in routed.iter_mut() {
        if counts.get(edge.to.node.as_str()).copied().unwrap_or(0) < 2 || edge.points.len() < 2 {
            continue;
        }
        let to = *edge.points.last().expect("non-empty route");
        let (anchor, cross) = if vertical { (to.y, to.x) } else { (to.x, to.y) };
        let previous = edge.points[edge.points.len() - 2];
        let previous_along = if vertical { previous.y } else { previous.x };
        let sign = if anchor >= previous_along { 1.0 } else { -1.0 };
        let merge = anchor - sign * ROUTE_MERGE_STUB;
        // Only split a straight final approach that spans the merge point.
        let final_is_axial = if vertical { (previous.x - cross).abs() < f64::EPSILON } else { (previous.y - cross).abs() < f64::EPSILON };
        let spans_merge = sign * (anchor - previous_along) > ROUTE_MERGE_STUB * 2.0;
        if final_is_axial && spans_merge {
            let point = if vertical { Point::new(cross, merge) } else { Point::new(merge, cross) };
            let last = edge.points.len() - 1;
            edge.points.insert(last, point);
        }
    }
}

fn dedupe_points(points: Vec<Point>) -> Vec<Point> {
    let mut deduped = Vec::with_capacity(points.len());
    for point in points {
        if deduped.last().map(|previous| *previous != point).unwrap_or(true) {
            deduped.push(point);
        }
    }
    deduped
}

fn polyline_midpoint(points: &[Point]) -> Point {
    let length: f64 = points.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
    if length <= 0.0 {
        return points.first().copied().unwrap_or(Point::ZERO);
    }
    let mut remaining = length * 0.5;
    for pair in points.windows(2) {
        let segment = pair[0].distance(pair[1]);
        if remaining <= segment && segment > 0.0 {
            let ratio = remaining / segment;
            return Point::new(pair[0].x + (pair[1].x - pair[0].x) * ratio, pair[0].y + (pair[1].y - pair[0].y) * ratio);
        }
        remaining -= segment;
    }
    points.last().copied().unwrap_or(Point::ZERO)
}

fn estimate_label_box(label: &str, center: Point, font_size: f64) -> Rect {
    let lines: Vec<&str> = label.split('\n').collect();
    let widest = lines.iter().map(|line| measure_line(line, font_size)).fold(0.0, f64::max);
    let width = widest + LABEL_PAD_X * 2.0;
    let height = lines.len() as f64 * font_size * 1.4 + LABEL_PAD_Y * 2.0;
    Rect::new(center.x - width * 0.5, center.y - height * 0.5, width, height)
}

/// Horizontal padding inside an edge label backing plate.
const LABEL_PAD_X: f64 = 6.0;
/// Vertical padding inside an edge label backing plate.
const LABEL_PAD_Y: f64 = 2.0;
/// Maximum line width of an edge label before greedy wrapping.
const LABEL_MAX_WIDTH: f64 = 200.0;
/// Maximum number of lines of an edge label before ellipsis truncation.
const LABEL_MAX_LINES: usize = 3;

/// The font size edge labels render at, derived from the node label size.
pub(crate) fn edge_label_font_size(font_size: f64) -> f64 {
    (font_size * 12.0 / 13.0).max(1.0)
}

/// Wrap and truncate an authored edge label against the font metrics table,
/// reporting a diagnostic when content was truncated.
fn prepare_edge_label(label: &str, from: &str, to: &str, font_size: f64, diagnostics: &mut Vec<Diagnostic>) -> Option<String> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lines = wrap_label(trimmed, font_size, LABEL_MAX_WIDTH, LABEL_MAX_LINES);
    if lines.last().is_some_and(|last| last.ends_with('…')) {
        diagnostics.push(warning(
            codes::APPROXIMATED,
            format!("edge label `{trimmed}` on `{from}->{to}` exceeds {LABEL_MAX_LINES} lines or {LABEL_MAX_WIDTH}px and is truncated"),
        ));
    }
    Some(lines.join("\n"))
}

fn node_memberships(clusters: &[ResolvedCluster], node_count: usize) -> Vec<Vec<usize>> {
    let mut memberships = vec![Vec::new(); node_count];
    for (cluster_index, cluster) in clusters.iter().enumerate() {
        for &member in &cluster.members {
            memberships[member].push(cluster_index);
        }
    }
    memberships
}

fn warning(code: &'static str, message: impl Into<String>) -> Diagnostic {
    diagnostic("diagram", code, DiagnosticSeverity::Warning, message)
}

/// Emit a warning and, when the frontend located the token, attach a primary
/// label covering that byte range.
///
/// The label names the frontend family from [`SourceMap::family`], so a DOT
/// range is attributed to `dot` and a PlantUML range to `plantuml`, matching
/// [`crate::DiagramError::labelled`].
fn located(code: &'static str, message: impl Into<String>, span: Option<Range<usize>>, source: &SourceMap) -> Diagnostic {
    let message = message.into();
    let record = diagnostic("diagram", code, DiagnosticSeverity::Warning, message.clone());
    let Some(span) = span
    else {
        return record;
    };
    let (Ok(source_ref), Ok(range)) = (SourceRef::new(NAMESPACE, source.family()), ByteRange::new(span.start as u64, span.end as u64))
    else {
        return record;
    };
    record.with_primary(DiagnosticLabel::new(
        DiagnosticLocation::Text { source: source_ref, range },
        Message::new(code).with_fallback(message),
        LabelRole::Primary,
    ))
}
