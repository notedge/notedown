//! Oak Mermaid frontend: source to diagram spec and to a lowered diagram.
//!
//! This module never assembles SVG or scene nodes. It maps the typed
//! `oak-mermaid` AST onto [`crate::spec::DiagramSpec`] and leaves geometry to
//! [`crate::compile::compile`] and [`crate::render::render`].

use std::collections::BTreeMap;

use oak_core::{Builder, SourceText, parser::session::ParseSession};
use oak_mermaid::{
    MermaidBuilder, MermaidLanguage,
    ast::{EdgeStyle as MermaidEdgeStyle, FlowElement, Flowchart, MermaidDiagram, MermaidRoot, NodeShape as MermaidNodeShape},
};

use crate::{
    LoweredDiagram,
    compile::{LayoutOptions, compile},
    error::DiagramError,
    render::render,
    spec::{Cluster, DiagramEdge, DiagramNode, DiagramSpec, Direction, EdgeStyle, Endpoint, NodeShape},
};

struct NodeSeed {
    id: String,
    label: Option<String>,
    shape: Option<NodeShape>,
    cluster: Option<String>,
}

/// Accumulates nodes, edges, and clusters from nested flowchart elements.
#[derive(Default)]
struct Collector {
    nodes: Vec<NodeSeed>,
    indices: BTreeMap<String, usize>,
    edges: Vec<DiagramEdge>,
    clusters: Vec<Cluster>,
}

impl Collector {
    /// Walk one nesting level. `ancestors` holds the enclosing cluster ids,
    /// outermost first.
    fn collect(&mut self, elements: &[FlowElement], ancestors: &[String]) -> Result<(), DiagramError> {
        for element in elements {
            match element {
                FlowElement::Node { id, text, shape } => {
                    let id = require_id(id)?;
                    let label = text.as_deref().map(str::trim).filter(|text| !text.is_empty()).map(str::to_string);
                    self.upsert_node(&id, label, Some(convert_shape(*shape)), ancestors);
                }
                FlowElement::Edge { from, to, text, style } => {
                    let from = require_id(from)?;
                    let to = require_id(to)?;
                    self.upsert_node(&from, None, None, ancestors);
                    self.upsert_node(&to, None, None, ancestors);
                    let label = text.as_deref().map(str::trim).filter(|text| !text.is_empty()).map(str::to_string);
                    self.edges.push(DiagramEdge {
                        from: Endpoint { node: from, port: None },
                        to: Endpoint { node: to, port: None },
                        label,
                        style: convert_edge_style(*style),
                    });
                }
                FlowElement::Subgraph(subgraph) => {
                    let id = self.register_cluster(subgraph.id.as_deref(), &subgraph.title);
                    let mut nested = ancestors.to_vec();
                    nested.push(id);
                    self.collect(&subgraph.elements, &nested)?;
                }
            }
        }
        Ok(())
    }

    fn upsert_node(&mut self, id: &str, label: Option<String>, shape: Option<NodeShape>, ancestors: &[String]) {
        match self.indices.get(id) {
            Some(&index) => {
                let node = &mut self.nodes[index];
                if node.label.is_none() {
                    node.label = label;
                }
                if node.shape.is_none() {
                    node.shape = shape;
                }
            }
            None => {
                self.indices.insert(id.to_string(), self.nodes.len());
                self.nodes.push(NodeSeed { id: id.to_string(), label, shape, cluster: ancestors.last().cloned() });
            }
        }

        for ancestor in ancestors {
            let Some(cluster) = self.clusters.iter_mut().find(|cluster| &cluster.id == ancestor)
            else {
                continue;
            };
            if !cluster.members.iter().any(|member| member == id) {
                cluster.members.push(id.to_string());
            }
        }
    }

    fn register_cluster(&mut self, explicit: Option<&str>, title: &str) -> String {
        let base = explicit
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("cluster-{}", self.clusters.len() + 1));
        let mut id = base.clone();
        let mut suffix = 1;
        while self.clusters.iter().any(|cluster| cluster.id == id) {
            id = format!("{base}-{suffix}");
            suffix += 1;
        }
        let label = title.trim();
        self.clusters.push(Cluster { id: id.clone(), label: (!label.is_empty()).then(|| label.to_string()), members: Vec::new() });
        id
    }

    fn into_parts(self) -> (Vec<DiagramNode>, Vec<DiagramEdge>, Vec<Cluster>) {
        let nodes = self
            .nodes
            .into_iter()
            .map(|node| DiagramNode {
                id: node.id,
                label: node.label,
                shape: node.shape.unwrap_or(NodeShape::Box),
                ports: Vec::new(),
                cluster: node.cluster,
            })
            .collect();
        (nodes, self.edges, self.clusters)
    }
}

/// Parse Mermaid source through the `oak-mermaid` frontend.
///
/// The frontend lowers `graph`/`flowchart` sections into `MermaidRoot`.
/// Diagram families outside that subset, or source without any diagram header,
/// produce an empty root. This function rejects an empty AST with an explicit
/// `davinci.diagram.ast-unavailable` diagnostic instead of reporting a
/// successful parse with no semantics.
pub fn parse_mermaid(source: &str) -> Result<MermaidRoot, DiagramError> {
    let text = SourceText::new(source);
    let language = MermaidLanguage::default();
    let builder = MermaidBuilder::new(&language);
    let mut session = ParseSession::<MermaidLanguage>::default();
    let output = builder.build(&text, &[], &mut session);
    let oak_diagnostics = output.diagnostics.len();

    match output.result {
        Ok(root) if root.diagrams.is_empty() => Err(DiagramError::ast_unavailable(format!(
            "`oak-mermaid` accepted {} bytes with {oak_diagnostics} Oak parser diagnostics but produced no diagrams, only graph/flowchart sections are lowered",
            source.len()
        ))),
        Ok(root) if root.diagrams.iter().any(|diagram| !matches!(diagram, MermaidDiagram::Flowchart(_))) => {
            Err(DiagramError::ast_unavailable(format!(
                "`oak-mermaid` accepted {} bytes with {oak_diagnostics} Oak parser diagnostics but produced non-flowchart diagrams, only graph/flowchart sections are lowered",
                source.len()
            )))
        }
        Ok(root) => Ok(root),
        Err(error) => Err(DiagramError::parse_failed(format!("`oak-mermaid` failed to build an AST: {error}"))),
    }
}

/// Lower Mermaid source into a source-independent diagram spec.
///
/// Only a single `graph`/`flowchart` diagram is accepted. Node ids keep the
/// first seen label, an edge endpoint that has no node definition becomes an
/// implicit `Box` node, and each `subgraph` block becomes a flat [`Cluster`].
/// Nested subgraphs are flattened: a node joins every ancestor cluster of the
/// block that declares it, so an outer frame still contains nested members.
pub fn mermaid_to_spec(source: &str) -> Result<DiagramSpec, DiagramError> {
    let root = parse_mermaid(source)?;
    let flowchart = single_flowchart(&root)?;
    let direction = direction(&flowchart.orientation)
        .ok_or_else(|| DiagramError::unsupported(format!("unsupported flowchart orientation `{}`", flowchart.orientation)))?;
    let mut collector = Collector::default();
    collector.collect(&flowchart.elements, &[])?;

    if collector.nodes.is_empty() {
        return Err(DiagramError::empty_diagram("flowchart contains no nodes or edges"));
    }

    let mut spec = DiagramSpec::new(direction);
    let (nodes, edges, clusters) = collector.into_parts();
    spec.nodes = nodes;
    spec.edges = edges;
    spec.clusters = clusters;
    Ok(spec)
}

/// Lower Mermaid source to a layout plan and display scene.
///
/// The pipeline is `spec -> compile -> render`. Diagnostics come from the
/// layout plan; rendering itself has no loss surface because every plan
/// construct has a deterministic scene mapping.
pub fn mermaid_to_scene(source: &str) -> Result<LoweredDiagram, DiagramError> {
    let spec = mermaid_to_spec(source)?;
    let plan = compile(&spec, &LayoutOptions::default());
    let scene = render(&plan);
    let diagnostics = plan.diagnostics.clone();
    Ok(LoweredDiagram { plan, scene, diagnostics })
}

fn single_flowchart(root: &MermaidRoot) -> Result<&Flowchart, DiagramError> {
    if root.diagrams.is_empty() {
        return Err(DiagramError::ast_unavailable("Mermaid source produced no diagrams, only graph/flowchart sections are lowered"));
    }
    if root.diagrams.len() > 1 {
        return Err(DiagramError::unsupported(format!(
            "a Mermaid root with {} diagrams is not lowered yet, exactly one diagram is supported",
            root.diagrams.len()
        )));
    }
    match &root.diagrams[0] {
        MermaidDiagram::Flowchart(flowchart) => Ok(flowchart),
        MermaidDiagram::Sequence(_) => {
            Err(DiagramError::ast_unavailable("Mermaid sequence diagrams are not lowered, only graph/flowchart sections are supported"))
        }
        MermaidDiagram::Class(_) => {
            Err(DiagramError::ast_unavailable("Mermaid class diagrams are not lowered, only graph/flowchart sections are supported"))
        }
    }
}

fn require_id(id: &str) -> Result<String, DiagramError> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err(DiagramError::unsupported("flowchart contains a node or edge endpoint with an empty id"));
    }
    Ok(trimmed.to_string())
}

fn direction(orientation: &str) -> Option<Direction> {
    match orientation.trim().to_ascii_uppercase().as_str() {
        "TD" | "TB" => Some(Direction::TopDown),
        "BT" => Some(Direction::BottomUp),
        "LR" => Some(Direction::LeftToRight),
        "RL" => Some(Direction::RightToLeft),
        _ => None,
    }
}

fn convert_shape(shape: MermaidNodeShape) -> NodeShape {
    match shape {
        MermaidNodeShape::Box => NodeShape::Box,
        MermaidNodeShape::Round => NodeShape::Round,
        MermaidNodeShape::Stadium => NodeShape::Stadium,
        MermaidNodeShape::Subroutine => NodeShape::Subroutine,
        MermaidNodeShape::Cylinder => NodeShape::Cylinder,
        MermaidNodeShape::Circle => NodeShape::Circle,
        MermaidNodeShape::Rhombus => NodeShape::Rhombus,
    }
}

fn convert_edge_style(style: MermaidEdgeStyle) -> EdgeStyle {
    match style {
        MermaidEdgeStyle::Line => EdgeStyle::Line,
        MermaidEdgeStyle::Arrow => EdgeStyle::Arrow,
        MermaidEdgeStyle::DottedArrow => EdgeStyle::DottedArrow,
        MermaidEdgeStyle::ThickArrow => EdgeStyle::ThickArrow,
    }
}
