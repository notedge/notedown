//! Deterministic layered compilation from a diagram spec to a layout plan.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use davinci_scene::{Point, Rect, Size};
use davinci_types::{Diagnostic, DiagnosticSeverity, codes, diagnostic};

use crate::{
    plan::{DiagramLayoutPlan, PlacedCluster, PlacedNode, RoutedEdge},
    spec::{Constraint, DiagramSpec, Direction, EdgeStyle, Endpoint, NodeShape, Port, Side},
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

struct ResolvedNode {
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
/// iteration order, and it keeps nodes in spec declaration order. Loss,
/// approximation, and conflict records are reported through
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
/// - Unknown cluster references, cluster members that do not exist, and
///   clusters without members are reported; unusable members are ignored.
pub fn compile(spec: &DiagramSpec, options: &LayoutOptions) -> DiagramLayoutPlan {
    let mut diagnostics = Vec::new();
    let nodes = resolve_nodes(spec, &mut diagnostics);
    let index_by_id: BTreeMap<String, usize> = nodes.iter().enumerate().map(|(index, node)| (node.id.clone(), index)).collect();

    for node in &nodes {
        if matches!(node.shape, NodeShape::Subroutine) {
            diagnostics.push(warning(
                codes::APPROXIMATED,
                format!("subroutine node `{}` is rendered as a plain rectangle without its double border", node.id),
            ));
        }
    }

    let clusters = resolve_clusters(spec, &nodes, &index_by_id, &mut diagnostics);
    let edges = resolve_edges(spec, &index_by_id, &mut diagnostics);

    if nodes.is_empty() {
        return DiagramLayoutPlan { canvas: Size::ZERO, nodes: Vec::new(), edges: Vec::new(), clusters: Vec::new(), diagnostics };
    }

    let layers = assign_layers(&nodes, &edges, &spec.constraints, &index_by_id, &mut diagnostics);
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
        ports.push(build_ports(node, rects[index], &mut diagnostics));
    }

    let memberships = node_memberships(&clusters, nodes.len());
    let mut resolver =
        EndpointResolver { nodes: &nodes, rects: &rects, ports: &ports, direction: spec.direction, reported_ports: BTreeSet::new() };
    let mut routed = Vec::with_capacity(edges.len());
    for edge in &edges {
        let from = resolver.resolve(&edge.from, edge.from_index, true, &mut diagnostics);
        let to = resolver.resolve(&edge.to, edge.to_index, false, &mut diagnostics);
        let points = route(from, to, spec.direction);
        let label = edge.label.clone().filter(|label| !label.trim().is_empty());
        let label_box = label.as_ref().map(|label| estimate_label_box(label, polyline_midpoint(&points), options.font_size));
        if matches!(edge.style, EdgeStyle::DottedArrow | EdgeStyle::DottedLine) {
            diagnostics.push(warning(
                codes::APPROXIMATED,
                format!(
                    "dotted edge `{}->{}` is rendered as a solid line because the scene style has no dash surface",
                    edge.from.node, edge.to.node
                ),
            ));
        }
        routed.push(RoutedEdge { from: edge.from.clone(), to: edge.to.clone(), points, label, label_box, style: edge.style });
    }

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

    DiagramLayoutPlan { canvas, nodes: placed_nodes, edges: routed, clusters: placed_clusters, diagnostics }
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
    let mut diagnostics = Vec::new();
    if !spec.clusters.is_empty() {
        diagnostics.push(warning(codes::UNSUPPORTED, format!("sequence layout ignores {} cluster declarations", spec.clusters.len())));
    }
    if !spec.constraints.is_empty() {
        diagnostics.push(warning(codes::UNSUPPORTED, format!("sequence layout ignores {} ordering constraints", spec.constraints.len())));
    }

    let mut participants: Vec<ResolvedNode> = Vec::with_capacity(spec.nodes.len());
    let mut indices: BTreeMap<String, usize> = BTreeMap::new();
    for node in &spec.nodes {
        if node.id.is_empty() {
            diagnostics.push(warning(codes::UNSUPPORTED, "sequence participant with an empty id is ignored"));
            continue;
        }
        if indices.contains_key(&node.id) {
            diagnostics.push(warning(
                codes::UNSUPPORTED,
                format!("duplicate participant id `{}`, the first declaration is kept", node.id),
            ));
            continue;
        }
        indices.insert(node.id.clone(), participants.len());
        participants.push(ResolvedNode {
            id: node.id.clone(),
            label: node.label.clone(),
            shape: node.shape,
            ports: Vec::new(),
            cluster: None,
        });
    }

    if participants.is_empty() {
        return DiagramLayoutPlan {
            canvas: Size::ZERO,
            nodes: Vec::new(),
            edges: Vec::new(),
            clusters: Vec::new(),
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

    let mut messages: Vec<RoutedEdge> = Vec::with_capacity(spec.edges.len());
    for edge in &spec.edges {
        let (Some(&from_index), Some(&to_index)) = (indices.get(&edge.from.node), indices.get(&edge.to.node)) else {
            let missing = if indices.contains_key(&edge.from.node) { edge.to.node.as_str() } else { edge.from.node.as_str() };
            diagnostics.push(warning(
                codes::UNSUPPORTED,
                format!(
                    "sequence message `{}->{}` references unknown participant `{missing}` and is skipped",
                    edge.from.node, edge.to.node
                ),
            ));
            continue;
        };
        if from_index == to_index {
            diagnostics.push(warning(
                codes::UNSUPPORTED,
                format!("self sequence message `{}->{}` is not routed and is skipped", edge.from.node, edge.to.node),
            ));
            continue;
        }
        let line_y = first_row + row_height * (messages.len() as f64 + 0.5);
        let from = Point::new(rects[from_index].center().x, line_y);
        let to = Point::new(rects[to_index].center().x, line_y);
        let label = edge.label.clone().filter(|label| !label.trim().is_empty());
        let label_box = label
            .as_ref()
            .map(|label| estimate_label_box(label, Point::new((from.x + to.x) * 0.5, line_y - options.font_size), options.font_size));
        if matches!(edge.style, EdgeStyle::DottedArrow | EdgeStyle::DottedLine) {
            diagnostics.push(warning(
                codes::APPROXIMATED,
                format!(
                    "dotted sequence message `{}->{}` is rendered as a solid line because the scene style has no dash surface",
                    edge.from.node, edge.to.node
                ),
            ));
        }
        messages.push(RoutedEdge {
            from: edge.from.clone(),
            to: edge.to.clone(),
            points: vec![from, to],
            label,
            label_box,
            style: edge.style,
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

    DiagramLayoutPlan { canvas, nodes: placed_nodes, edges: messages, clusters: Vec::new(), diagnostics }
}

fn resolve_nodes(spec: &DiagramSpec, diagnostics: &mut Vec<Diagnostic>) -> Vec<ResolvedNode> {
    let mut nodes: Vec<ResolvedNode> = Vec::with_capacity(spec.nodes.len());
    let mut indices: BTreeMap<String, usize> = BTreeMap::new();

    for node in &spec.nodes {
        if node.id.is_empty() {
            diagnostics.push(warning(codes::UNSUPPORTED, "node with an empty id is ignored"));
            continue;
        }
        match indices.get(&node.id) {
            Some(&index) => {
                diagnostics.push(warning(
                    codes::UNSUPPORTED,
                    format!("duplicate node id `{}`, the first definition is kept and later port declarations are merged", node.id),
                ));
                nodes[index].ports.extend(node.ports.iter().cloned());
            }
            None => {
                indices.insert(node.id.clone(), nodes.len());
                nodes.push(ResolvedNode {
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
                diagnostics.push(warning(
                    codes::UNSUPPORTED,
                    format!("node `{}` references unknown cluster `{cluster}`, the reference is ignored", node.id),
                ));
            }
        }
    }

    let memberships = node_memberships(&clusters, nodes.len());
    for (index, members) in memberships.iter().enumerate() {
        if members.len() > 1 {
            diagnostics.push(warning(
                codes::UNSUPPORTED,
                format!("node `{}` belongs to multiple clusters, the first declared frame is used", nodes[index].id),
            ));
        }
    }

    clusters
}

fn resolve_edges(spec: &DiagramSpec, index_by_id: &BTreeMap<String, usize>, diagnostics: &mut Vec<Diagnostic>) -> Vec<ResolvedEdge> {
    let mut edges: Vec<ResolvedEdge> = Vec::with_capacity(spec.edges.len());

    for edge in &spec.edges {
        let from_index = index_by_id.get(&edge.from.node).copied();
        let to_index = index_by_id.get(&edge.to.node).copied();
        let (from_index, to_index) = match (from_index, to_index) {
            (Some(from), Some(to)) => (from, to),
            (None, _) => {
                diagnostics.push(warning(
                    codes::UNSUPPORTED,
                    format!("edge `{}->{}` references unknown node `{}` and is skipped", edge.from.node, edge.to.node, edge.from.node),
                ));
                continue;
            }
            (_, None) => {
                diagnostics.push(warning(
                    codes::UNSUPPORTED,
                    format!("edge `{}->{}` references unknown node `{}` and is skipped", edge.from.node, edge.to.node, edge.to.node),
                ));
                continue;
            }
        };

        if from_index == to_index {
            diagnostics
                .push(warning(codes::UNSUPPORTED, format!("self edge `{}->{}` is not routed and is skipped", edge.from.node, edge.to.node)));
            continue;
        }

        edges.push(ResolvedEdge {
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
        let ids: Vec<&str> = cycle_nodes.iter().map(|&index| nodes[index].id.as_str()).collect();
        diagnostics.push(warning(
            codes::APPROXIMATED,
            format!("diagram contains a cycle involving nodes {ids:?}. Layered placement breaks it in declaration order"),
        ));
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
    let longest = lines.iter().map(|line| line.chars().count()).max().unwrap_or(0);
    let width = (longest as f64 * options.font_size * 0.6 + options.padding_x * 2.0).max(options.min_node_width);
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

fn build_ports(node: &ResolvedNode, rect: Rect, diagnostics: &mut Vec<Diagnostic>) -> BTreeMap<String, Point> {
    let mut ports = BTreeMap::new();
    for port in &node.ports {
        if port.id.is_empty() {
            diagnostics
                .push(warning(codes::UNSUPPORTED, format!("node `{}` declares a port with an empty id, the declaration is ignored", node.id)));
            continue;
        }
        if ports.contains_key(&port.id) {
            diagnostics.push(warning(
                codes::UNSUPPORTED,
                format!("node `{}` declares duplicate port id `{}`, the first declaration is kept", node.id, port.id),
            ));
            continue;
        }
        let offset = if !port.offset.is_finite() {
            diagnostics
                .push(warning(codes::APPROXIMATED, format!("node `{}` declares a port offset that is not finite, `0.0` is used", node.id)));
            0.0
        }
        else if !(0.0..=1.0).contains(&port.offset) {
            diagnostics.push(warning(
                codes::APPROXIMATED,
                format!("port `{}` on node `{}` has an offset outside `0.0..=1.0` and is clamped", port.id, node.id),
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
    fn resolve(&mut self, endpoint: &Endpoint, node_index: usize, is_source: bool, diagnostics: &mut Vec<Diagnostic>) -> Point {
        let rect = self.rects[node_index];
        match endpoint.port.as_deref() {
            None => default_anchor(rect, self.direction, is_source),
            Some(port_id) => match self.ports[node_index].get(port_id) {
                Some(point) => *point,
                None => {
                    let node_id = &self.nodes[node_index].id;
                    if self.reported_ports.insert((node_id.clone(), port_id.to_string())) {
                        diagnostics.push(warning(
                            codes::APPROXIMATED,
                            format!(
                                "edge endpoint references unknown port `{port_id}` on node `{node_id}`, the default anchor for the layout direction is used"
                            ),
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
    let longest = lines.iter().map(|line| line.chars().count()).max().unwrap_or(0);
    let width = longest as f64 * font_size * 0.6 + 16.0;
    let height = lines.len() as f64 * font_size * 1.4 + 8.0;
    Rect::new(center.x - width * 0.5, center.y - height * 0.5, width, height)
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
