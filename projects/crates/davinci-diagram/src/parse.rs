//! Oak Mermaid frontend: source to diagram spec and to a lowered diagram.
//!
//! This module never assembles SVG or scene nodes. It maps the typed
//! `oak-mermaid` AST onto [`crate::spec::DiagramSpec`] and leaves geometry to
//! [`crate::compile::compile`] or [`crate::compile::compile_sequence`] and
//! [`crate::render::render`] or [`crate::render::render_sequence`].

use std::collections::{BTreeMap, BTreeSet};

use davinci_scene::SceneResolver;
use oak_core::{Builder, SourceText, parser::session::ParseSession};
use oak_mermaid::{
    MermaidBuilder, MermaidLanguage,
    ast::{
        EdgeStyle as MermaidEdgeStyle, FlowElement, Flowchart, MermaidDiagram, MermaidRoot, MessageArrow,
        NodeShape as MermaidNodeShape, ParticipantKind, SequenceDiagram,
    },
};

use crate::{
    LoweredDiagram,
    compile::{LayoutOptions, compile, compile_sequence},
    error::DiagramError,
    render::{render, render_sequence},
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
/// The frontend recognises `graph`/`flowchart`, `sequenceDiagram`, and
/// `classDiagram` sections. Source without any diagram header produces an empty
/// root. This function rejects an empty AST with an explicit
/// `davinci.diagram.ast-unavailable` diagnostic instead of reporting a
/// successful parse with no semantics. Family-specific lowering rules live in
/// the functions below.
pub fn parse_mermaid(source: &str) -> Result<MermaidRoot, DiagramError> {
    let text = SourceText::new(source);
    let language = MermaidLanguage::default();
    let builder = MermaidBuilder::new(&language);
    let mut session = ParseSession::<MermaidLanguage>::default();
    let output = builder.build(&text, &[], &mut session);
    let oak_diagnostics = output.diagnostics.len();

    match output.result {
        Ok(root) if root.diagrams.is_empty() => Err(DiagramError::ast_unavailable(format!(
            "`oak-mermaid` accepted {} bytes with {oak_diagnostics} Oak parser diagnostics but produced no diagrams",
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
/// implicit `Box` node, and each `subgraph` block becomes a flat [`Cluster`].
/// Nested subgraphs are flattened: a node joins every ancestor cluster of the
/// block that declares it, so an outer frame still contains nested members.
///
/// Sequence and class diagrams lower through [`mermaid_to_scene`], which selects
/// their family-specific compile pass.
pub fn mermaid_to_spec(source: &str) -> Result<DiagramSpec, DiagramError> {
    let root = parse_mermaid(source)?;
    match single_diagram(&root)? {
        MermaidDiagram::Flowchart(flowchart) => flowchart_spec(flowchart),
        MermaidDiagram::Sequence(_) | MermaidDiagram::Class(_) => Err(DiagramError::unsupported(
            "`mermaid_to_spec` returns the flowchart graph spec, sequence and class diagrams lower through `mermaid_to_scene`",
        )),
    }
}

/// Lower Mermaid source to a layout plan and resolved display scene.
///
/// The pipeline is `spec -> compile -> render -> resolve`. Flowcharts and class
/// diagrams use layered graph layout, sequence diagrams use time-ordered layout.
/// Diagnostics come from the layout plan; rendering and resolution have no loss
/// surface because every plan construct has a deterministic scene mapping.
/// Unsupported sequence statements fail before compilation so the host keeps the
/// authored source visible instead of dropping the diagram silently.
pub fn mermaid_to_scene(source: &str) -> Result<LoweredDiagram, DiagramError> {
    let root = parse_mermaid(source)?;
    match single_diagram(&root)? {
        MermaidDiagram::Flowchart(flowchart) => lower_graph(flowchart_spec(flowchart)?),
        MermaidDiagram::Sequence(sequence) => {
            validate_sequence_source(source)?;
            let spec = sequence_spec(sequence)?;
            let plan = compile_sequence(&spec, &LayoutOptions::default());
            let semantic = render_sequence(&plan);
            let scene = semantic.resolve(&SceneResolver::default());
            let diagnostics = plan.diagnostics.clone();
            Ok(LoweredDiagram { plan, scene, diagnostics })
        }
        MermaidDiagram::Class(_) => {
            Err(DiagramError::ast_unavailable("Mermaid class diagrams are not lowered yet"))
        }
    }
}

fn lower_graph(spec: DiagramSpec) -> Result<LoweredDiagram, DiagramError> {
    let plan = compile(&spec, &LayoutOptions::default());
    let semantic = render(&plan);
    let scene = semantic.resolve(&SceneResolver::default());
    let diagnostics = plan.diagnostics.clone();
    Ok(LoweredDiagram { plan, scene, diagnostics })
}

fn flowchart_spec(flowchart: &Flowchart) -> Result<DiagramSpec, DiagramError> {
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

fn single_diagram(root: &MermaidRoot) -> Result<&MermaidDiagram, DiagramError> {
    if root.diagrams.is_empty() {
        return Err(DiagramError::ast_unavailable("Mermaid source produced no diagrams"));
    }
    if root.diagrams.len() > 1 {
        return Err(DiagramError::unsupported(format!(
            "a Mermaid root with {} diagrams is not lowered yet, exactly one diagram is supported",
            root.diagrams.len()
        )));
    }
    Ok(&root.diagrams[0])
}

/// Lower one `sequenceDiagram` AST into the graph spec used by sequence layout.
///
/// Participants become nodes in declaration order and messages become edges in
/// source order. A message endpoint that has no participant declaration becomes
/// an implicit participant, matching Mermaid. `NodeShape::Box` marks a
/// participant and `NodeShape::Round` marks an actor, so the actor distinction
/// survives without a second node model.
fn sequence_spec(sequence: &SequenceDiagram) -> Result<DiagramSpec, DiagramError> {
    let mut spec = DiagramSpec::new(Direction::TopDown);
    let mut known: BTreeSet<String> = BTreeSet::new();

    for participant in &sequence.participants {
        let id = participant.id.trim();
        if id.is_empty() {
            continue;
        }
        let label = participant.display.as_deref().map(str::trim).filter(|display| !display.is_empty()).map(str::to_string);
        spec.nodes.push(DiagramNode {
            id: id.to_string(),
            label,
            shape: match participant.kind {
                ParticipantKind::Participant => NodeShape::Box,
                ParticipantKind::Actor => NodeShape::Round,
            },
            ports: Vec::new(),
            cluster: None,
        });
        known.insert(id.to_string());
    }

    for message in &sequence.messages {
        let from = message.from.trim();
        let to = message.to.trim();
        if from.is_empty() || to.is_empty() {
            return Err(DiagramError::unsupported("sequence message has an empty participant id"));
        }
        for endpoint in [from, to] {
            if known.insert(endpoint.to_string()) {
                spec.nodes.push(DiagramNode {
                    id: endpoint.to_string(),
                    label: None,
                    shape: NodeShape::Box,
                    ports: Vec::new(),
                    cluster: None,
                });
            }
        }
        spec.edges.push(DiagramEdge {
            from: Endpoint { node: from.to_string(), port: None },
            to: Endpoint { node: to.to_string(), port: None },
            label: Some(message.text.trim().to_string()).filter(|text| !text.is_empty()),
            style: message_style(message.arrow),
        });
    }

    if spec.nodes.is_empty() {
        return Err(DiagramError::empty_diagram("sequence diagram contains no participants"));
    }
    Ok(spec)
}

fn message_style(arrow: MessageArrow) -> EdgeStyle {
    match arrow {
        MessageArrow::Solid => EdgeStyle::Line,
        MessageArrow::Dotted => EdgeStyle::DottedLine,
        MessageArrow::SolidArrow => EdgeStyle::Arrow,
        MessageArrow::DottedArrow => EdgeStyle::DottedArrow,
    }
}

/// Diagram headers that start a new section in one Mermaid source.
const DIAGRAM_HEADERS: &[&str] = &[
    "flowchart",
    "graph",
    "classDiagram",
    "stateDiagram-v2",
    "stateDiagram",
    "erDiagram",
    "gantt",
    "pie",
    "gitGraph",
    "journey",
    "mindmap",
    "requirementDiagram",
    "timeline",
    "quadrantChart",
    "xychart-beta",
    "sankey-beta",
    "block-beta",
    "C4Context",
    "zenuml",
];

/// Reject sequence statements outside the documented lowering subset.
///
/// `oak-mermaid` skips statements such as `Note`, `loop`, `alt`, `activate`, and
/// `autonumber`. Skipping them in the AST would render a silently incomplete
/// sequence, so this guard checks the authored sequence section before lowering:
/// blank lines, comments, participant/actor declarations, and message arrows are
/// accepted, and every other statement produces a structured
/// `davinci.diagram.unsupported` error so the host falls back to the authored
/// source. The guard classifies statement starts only, it does not build a
/// second AST.
fn validate_sequence_source(source: &str) -> Result<(), DiagramError> {
    let mut inside = false;
    for (index, raw) in source.lines().enumerate() {
        let line = raw.trim();
        if !inside {
            inside = line.starts_with("sequenceDiagram");
            continue;
        }
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if DIAGRAM_HEADERS.iter().any(|header| starts_with_keyword(line, header)) {
            break;
        }
        if starts_with_keyword(line, "participant") || starts_with_keyword(line, "actor") {
            continue;
        }
        if line.contains("-->>") || line.contains("->>") || line.contains("-->") || line.contains("->") {
            continue;
        }
        let keyword = line.split_whitespace().next().unwrap_or(line);
        return Err(DiagramError::unsupported(format!(
            "sequence statement `{keyword}` on line {} is not lowered, the diagram falls back to its source",
            index + 1
        )));
    }
    Ok(())
}

fn starts_with_keyword(line: &str, keyword: &str) -> bool {
    line.strip_prefix(keyword)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace) || rest.starts_with(':'))
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
