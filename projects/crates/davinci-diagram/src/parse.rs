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
    spec::{DiagramEdge, DiagramNode, DiagramSpec, Direction, EdgeStyle, Endpoint, NodeShape},
};

struct NodeSeed {
    id: String,
    label: Option<String>,
    shape: Option<NodeShape>,
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
        Ok(root) => Ok(root),
        Err(error) => Err(DiagramError::parse_failed(format!("`oak-mermaid` failed to build an AST: {error}"))),
    }
}

/// Lower Mermaid source into a source-independent diagram spec.
///
/// Only a single `graph`/`flowchart` diagram is accepted. Node ids keep the
/// first seen label, an edge endpoint that has no node definition becomes an
/// implicit `Box` node, and subgraphs are absent from the pinned Oak AST, so
/// they cannot be represented in a spec.
pub fn mermaid_to_spec(source: &str) -> Result<DiagramSpec, DiagramError> {
    let root = parse_mermaid(source)?;
    let flowchart = single_flowchart(&root)?;
    let direction = direction(&flowchart.orientation)
        .ok_or_else(|| DiagramError::unsupported(format!("unsupported flowchart orientation `{}`", flowchart.orientation)))?;
    let (nodes, edges) = collect_flowchart(flowchart)?;

    if nodes.is_empty() {
        return Err(DiagramError::empty_diagram("flowchart contains no nodes or edges"));
    }

    let mut spec = DiagramSpec::new(direction);
    spec.nodes = nodes;
    spec.edges = edges;
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
        MermaidDiagram::ClassDiagram(_) => {
            Err(DiagramError::ast_unavailable("Mermaid class diagrams are not lowered, only graph/flowchart sections are supported"))
        }
    }
}

fn collect_flowchart(flowchart: &Flowchart) -> Result<(Vec<DiagramNode>, Vec<DiagramEdge>), DiagramError> {
    let mut nodes: Vec<NodeSeed> = Vec::new();
    let mut indices: BTreeMap<String, usize> = BTreeMap::new();
    let mut edges: Vec<DiagramEdge> = Vec::new();

    for element in &flowchart.elements {
        match element {
            FlowElement::Node { id, text, shape } => {
                let id = require_id(id)?;
                let label = text.as_deref().map(str::trim).filter(|text| !text.is_empty()).map(str::to_string);
                match indices.get(&id) {
                    Some(&index) => {
                        let node = &mut nodes[index];
                        if node.label.is_none() {
                            node.label = label;
                        }
                        if node.shape.is_none() {
                            node.shape = Some(convert_shape(*shape));
                        }
                    }
                    None => {
                        indices.insert(id.clone(), nodes.len());
                        nodes.push(NodeSeed { id, label, shape: Some(convert_shape(*shape)) });
                    }
                }
            }
            FlowElement::Edge { from, to, text, style } => {
                let from = require_id(from)?;
                let to = require_id(to)?;
                ensure_node(&mut nodes, &mut indices, &from);
                ensure_node(&mut nodes, &mut indices, &to);
                let label = text.as_deref().map(str::trim).filter(|text| !text.is_empty()).map(str::to_string);
                edges.push(DiagramEdge {
                    from: Endpoint { node: from, port: None },
                    to: Endpoint { node: to, port: None },
                    label,
                    style: convert_edge_style(*style),
                });
            }
        }
    }

    let nodes = nodes
        .into_iter()
        .map(|node| DiagramNode {
            id: node.id,
            label: node.label,
            shape: node.shape.unwrap_or(NodeShape::Box),
            ports: Vec::new(),
            cluster: None,
        })
        .collect();

    Ok((nodes, edges))
}

fn require_id(id: &str) -> Result<String, DiagramError> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err(DiagramError::unsupported("flowchart contains a node or edge endpoint with an empty id"));
    }
    Ok(trimmed.to_string())
}

fn ensure_node(nodes: &mut Vec<NodeSeed>, indices: &mut BTreeMap<String, usize>, id: &str) {
    if indices.contains_key(id) {
        return;
    }
    indices.insert(id.to_string(), nodes.len());
    nodes.push(NodeSeed { id: id.to_string(), label: None, shape: None });
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
