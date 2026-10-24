//! Deterministic layered placement for flowchart nodes.

use std::collections::VecDeque;

use davinci_scene::{Point, Rect, Size};

const MARGIN: f64 = 24.0;
const LAYER_GAP: f64 = 72.0;
const NODE_GAP: f64 = 32.0;

/// The reading direction requested by a Mermaid flowchart orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    /// `TD` or `TB`.
    TopDown,
    /// `BT`.
    BottomUp,
    /// `LR`.
    LeftToRight,
    /// `RL`.
    RightToLeft,
}

impl Direction {
    pub(crate) fn parse(orientation: &str) -> Option<Self> {
        match orientation.trim().to_ascii_uppercase().as_str() {
            "TD" | "TB" => Some(Self::TopDown),
            "BT" => Some(Self::BottomUp),
            "LR" => Some(Self::LeftToRight),
            "RL" => Some(Self::RightToLeft),
            _ => None,
        }
    }

    fn is_horizontal(self) -> bool {
        matches!(self, Self::LeftToRight | Self::RightToLeft)
    }

    fn is_reversed(self) -> bool {
        matches!(self, Self::BottomUp | Self::RightToLeft)
    }
}

/// Result of a layered placement pass.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Layout {
    pub(crate) rects: Vec<Rect>,
    pub(crate) canvas: Size,
    /// Nodes whose layer had to be broken because the graph contains a cycle.
    pub(crate) cycle_nodes: Vec<usize>,
}

/// Place node boxes into declaration-ordered layers.
///
/// Layers come from a longest-path pass over the acyclic part of the graph.
/// Nodes still unresolved afterwards are cycle members: they receive a
/// deterministic layer from their resolved predecessors and are reported in
/// [`Layout::cycle_nodes`]. Placement is centered on the cross axis, and the
/// whole drawing is shifted into positive coordinates with a fixed margin.
pub(crate) fn layered_rects(sizes: &[Size], edges: &[(usize, usize)], direction: Direction) -> Layout {
    let count = sizes.len();
    if count == 0 {
        return Layout { rects: Vec::new(), canvas: Size::ZERO, cycle_nodes: Vec::new() };
    }

    let mut successors = vec![Vec::new(); count];
    let mut indegree = vec![0usize; count];
    for &(from, to) in edges {
        successors[from].push(to);
        indegree[to] += 1;
    }

    let mut layer = vec![0usize; count];
    let mut resolved = vec![false; count];
    let mut queue: VecDeque<usize> = (0..count).filter(|index| indegree[*index] == 0).collect();
    while let Some(node) = queue.pop_front() {
        resolved[node] = true;
        for &successor in &successors[node] {
            layer[successor] = layer[successor].max(layer[node] + 1);
            indegree[successor] -= 1;
            if indegree[successor] == 0 {
                queue.push_back(successor);
            }
        }
    }

    let mut cycle_nodes = Vec::new();
    for node in 0..count {
        if resolved[node] {
            continue;
        }
        cycle_nodes.push(node);
        let mut best = layer[node];
        for &(from, to) in edges {
            if to == node {
                best = best.max(layer[from] + 1);
            }
        }
        layer[node] = best;
        resolved[node] = true;
    }

    let max_layer = layer.iter().copied().max().unwrap_or(0);
    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); max_layer + 1];
    for (index, layer) in layer.iter().enumerate() {
        groups[*layer].push(index);
    }

    let primary_size = |index: usize| if direction.is_horizontal() { sizes[index].width } else { sizes[index].height };
    let cross_size = |index: usize| if direction.is_horizontal() { sizes[index].height } else { sizes[index].width };

    let mut primary_extents = Vec::with_capacity(groups.len());
    let mut cross_extents = Vec::with_capacity(groups.len());
    for group in &groups {
        let primary = group.iter().map(|index| primary_size(*index)).fold(0.0, f64::max);
        let cross = group.iter().map(|index| cross_size(*index)).sum::<f64>() + NODE_GAP * group.len().saturating_sub(1) as f64;
        primary_extents.push(primary);
        cross_extents.push(cross);
    }

    let mut primary_centers = vec![0.0; groups.len()];
    let mut cursor = 0.0;
    for (index, extent) in primary_extents.iter().enumerate() {
        primary_centers[index] = cursor + extent * 0.5;
        cursor += extent + LAYER_GAP;
    }
    let total_primary = cursor - LAYER_GAP;

    let mut rects = vec![Rect::default(); count];
    for (group_index, group) in groups.iter().enumerate() {
        let primary_center = if direction.is_reversed() { total_primary - primary_centers[group_index] } else { primary_centers[group_index] };
        let mut cross_cursor = -cross_extents[group_index] * 0.5;
        for index in group {
            let cross_center = cross_cursor + cross_size(*index) * 0.5;
            cross_cursor += cross_size(*index) + NODE_GAP;
            let (x, y) = if direction.is_horizontal() {
                (primary_center - sizes[*index].width * 0.5, cross_center - sizes[*index].height * 0.5)
            }
            else {
                (cross_center - sizes[*index].width * 0.5, primary_center - sizes[*index].height * 0.5)
            };
            rects[*index] = Rect::new(x, y, sizes[*index].width, sizes[*index].height);
        }
    }

    let min_x = rects.iter().map(|rect| rect.x).fold(f64::INFINITY, f64::min);
    let min_y = rects.iter().map(|rect| rect.y).fold(f64::INFINITY, f64::min);
    let max_x = rects.iter().map(|rect| rect.right()).fold(f64::NEG_INFINITY, f64::max);
    let max_y = rects.iter().map(|rect| rect.bottom()).fold(f64::NEG_INFINITY, f64::max);
    let shift = Point::new(MARGIN - min_x, MARGIN - min_y);
    for rect in &mut rects {
        rect.x += shift.x;
        rect.y += shift.y;
    }

    Layout { rects, canvas: Size::new(max_x - min_x + MARGIN * 2.0, max_y - min_y + MARGIN * 2.0), cycle_nodes }
}
