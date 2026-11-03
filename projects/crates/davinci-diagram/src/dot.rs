//! Graphviz DOT frontend.
//!
//! This module maps the typed `oak-dot` AST onto [`DiagramSpec`]. It never
//! builds scene nodes or SVG. Lowering accepts the documented whitelist only:
//! `digraph` / `graph`, `rankdir` with its four values, one level of
//! `subgraph cluster_*`, edge labels, and compass ports. Everything outside
//! that set fails with a [`DiagramError`] carrying the offending byte range.

use std::ops::Range;

use davinci_types::{
    ByteRange, Diagnostic, DiagnosticLabel, DiagnosticLocation, DiagnosticSeverity, LabelRole, Message, NAMESPACE, SourceRef, codes, diagnostic,
};
use oak_core::{Builder, SourceText, parser::session::ParseSession};
use oak_dot::{
    DotBuilder, DotLanguage,
    ast::{Attribute, AttributeTarget, Compass, DotRoot, EdgeOp, EdgeOperand, GraphType, NodeId, NodeStatement, Statement, SubgraphStatement},
};

use crate::{
    LoweredDiagram,
    compile::{LayoutOptions, compile},
    error::DiagramError,
    render::render,
    spec::{Cluster, Constraint, DiagramEdge, DiagramNode, DiagramSpec, Direction, EdgeStyle, Endpoint, NodeShape, Port, Side},
};

/// Source identity used by approximation labels.
const SOURCE: &str = "dot";

/// Compass points accepted as ports, longest token first.
const COMPASSES: &[(&str, Compass)] = &[
    ("ne", Compass::NE),
    ("nw", Compass::NW),
    ("se", Compass::SE),
    ("sw", Compass::SW),
    ("n", Compass::N),
    ("e", Compass::E),
    ("s", Compass::S),
    ("w", Compass::W),
    ("c", Compass::C),
];

/// Attribute keys that may appear on nodes and edges.
const ALLOWED_ELEMENT_KEYS: &[&str] = &["label", "shape", "style", "fillcolor", "color", "fontcolor", "labelfontcolor"];

/// Attribute keys that may appear at graph level.
const ALLOWED_GRAPH_KEYS: &[&str] = &["rankdir", "label"];

/// Style tokens accepted by the subset.
const ALLOWED_STYLE_TOKENS: &[&str] = &["filled", "solid", "bold", "dashed", "dotted"];

/// Attribute keys that name layout engines, routing, or interaction surfaces
/// this frontend does not implement.
const FORBIDDEN_KEYS: &[&str] = &[
    "pos",
    "splines",
    "layout",
    "engine",
    "dpi",
    "ranksep",
    "nodesep",
    "constraint",
    "weight",
    "dir",
    "arrowhead",
    "headlabel",
    "taillabel",
    "xlabel",
    "URL",
    "href",
    "tooltip",
    "target",
];

/// X11 color names accepted in addition to hexadecimal colors.
const NAMED_COLORS: &[&str] = &[
    "aqua", "black", "blue", "brown", "cyan", "fuchsia", "gray", "green", "grey", "lime", "magenta", "maroon", "navy", "olive", "orange",
    "pink", "purple", "red", "silver", "teal", "white", "yellow",
];

/// Parse DOT source through the `oak-dot` frontend.
pub fn parse_dot(source: &str) -> Result<DotRoot, DiagramError> {
    reject_html_labels(source)?;
    let text = SourceText::new(source);
    let language = DotLanguage::default();
    let builder = DotBuilder::new(&language);
    let mut session = ParseSession::<DotLanguage>::default();
    let output = builder.build(&text, &[], &mut session);
    let oak_diagnostics = output.diagnostics.len();

    match output.result {
        Ok(root) if root.graphs.is_empty() => Err(DiagramError::ast_unavailable(format!(
            "`oak-dot` accepted {} bytes with {oak_diagnostics} Oak parser diagnostics but produced no graph",
            source.len()
        ))),
        Ok(root) => Ok(root),
        Err(error) => Err(DiagramError::parse_failed(format!("`oak-dot` failed to build an AST: {error}"))),
    }
}

/// Lower DOT source into a source-independent diagram spec.
pub fn dot_to_spec(source: &str) -> Result<DiagramSpec, DiagramError> {
    let root = parse_dot(source)?;
    dot_root_to_spec(&root).map(|(spec, _)| spec)
}

/// Lower DOT source to a layout plan and resolved display scene.
pub fn dot_to_scene(source: &str) -> Result<LoweredDiagram, DiagramError> {
    let root = parse_dot(source)?;
    let (spec, mut diagnostics) = dot_root_to_spec(&root)?;
    let plan = compile(&spec, &LayoutOptions::default());
    diagnostics.extend(plan.diagnostics.iter().cloned());
    let semantic = render(&plan);
    let scene = semantic.resolve(&davinci_scene::SceneResolver::default());
    Ok(LoweredDiagram { plan, scene, diagnostics })
}

/// Reject HTML-like labels before the parser sees them.
///
/// `oak-dot` keeps DOT tokens generic, so `<b>x</b>` reaches the AST as
/// ordinary identifiers and loses the span of the whole construct. Scanning
/// outside strings and comments preserves the diagnostic range.
fn reject_html_labels(source: &str) -> Result<(), DiagramError> {
    let bytes = source.as_bytes();
    let mut index = 0;
    let mut in_string = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while index < bytes.len() {
        match bytes[index] {
            b'"' if !in_line_comment && !in_block_comment => {
                in_string = !in_string;
                index += 1;
            }
            b'\\' if in_string => index += 2,
            b'/' if !in_string && !in_block_comment && bytes.get(index + 1) == Some(&b'/') => {
                in_line_comment = true;
                index += 2;
            }
            b'\n' if in_line_comment => {
                in_line_comment = false;
                index += 1;
            }
            b'/' if !in_string && bytes.get(index + 1) == Some(&b'*') => {
                in_block_comment = true;
                index += 2;
            }
            b'*' if in_block_comment && bytes.get(index + 1) == Some(&b'/') => {
                in_block_comment = false;
                index += 2;
            }
            b'<' if !in_string && !in_line_comment && !in_block_comment => {
                return Err(unsupported_at("HTML-like labels are outside the DOT subset", index..html_label_end(bytes, index)));
            }
            _ => index += 1,
        }
    }
    Ok(())
}

/// End offset of an HTML label, taken at the last `>` before the attribute
/// list, a statement separator, or a line break. Nested tags such as
/// `<b>x</b>` therefore stay inside one diagnostic range.
fn html_label_end(bytes: &[u8], start: usize) -> usize {
    let mut end = start + 1;
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'>' => {
                end = index + 1;
                index += 1;
            }
            b']' | b';' | b'\n' | b',' => break,
            _ => index += 1,
        }
    }
    end
}

fn span_of(range: core::range::Range<usize>) -> Range<usize> {
    range.start..range.end
}

fn unsupported_at(message: impl Into<String>, span: Range<usize>) -> DiagramError {
    DiagramError::new(codes::UNSUPPORTED, message).with_span(span)
}

/// Build a warning that points at the offending byte range.
fn approximated_at(message: impl Into<String>, span: Range<usize>) -> Diagnostic {
    let message = message.into();
    let record = diagnostic("diagram", codes::APPROXIMATED, DiagnosticSeverity::Warning, message.clone());
    let Ok(source) = SourceRef::new(NAMESPACE, SOURCE)
    else {
        return record;
    };
    let Ok(range) = ByteRange::new(span.start as u64, span.end as u64)
    else {
        return record;
    };
    record.with_primary(DiagnosticLabel::new(
        DiagnosticLocation::Text { source, range },
        Message::new(codes::APPROXIMATED).with_fallback(message),
        LabelRole::Primary,
    ))
}

/// Per-graph defaults declared by `node [...]`, `edge [...]`, and `graph
/// [...]`.
///
/// Defaults apply at creation time, exactly as DOT does: a node keeps the
/// defaults that were active when the graph first mentioned it.
#[derive(Default)]
struct Defaults {
    node_shape: Option<NodeShape>,
    node_label: Option<String>,
    edge_style: Option<EdgeStyle>,
    edge_label: Option<String>,
}

/// Nodes, edges, clusters, and constraints accumulate here.
#[derive(Default)]
struct Collector {
    nodes: Vec<DiagramNode>,
    indices: std::collections::BTreeMap<String, usize>,
    edges: Vec<DiagramEdge>,
    clusters: Vec<Cluster>,
    constraints: Vec<Constraint>,
}

impl Collector {
    fn node_index(&mut self, id: &str) -> usize {
        match self.indices.get(id) {
            Some(&index) => index,
            None => {
                self.indices.insert(id.to_string(), self.nodes.len());
                self.nodes.push(DiagramNode { id: id.to_string(), label: None, shape: NodeShape::Box, ports: Vec::new(), cluster: None });
                self.nodes.len() - 1
            }
        }
    }

    fn set_label(&mut self, id: &str, label: &str) {
        let index = self.node_index(id);
        self.nodes[index].label = Some(fold_label(label));
    }

    fn set_shape(&mut self, id: &str, shape: NodeShape) {
        let index = self.node_index(id);
        self.nodes[index].shape = shape;
    }

    fn add_port(&mut self, id: &str, compass: Compass, span: Range<usize>, diagnostics: &mut Vec<Diagnostic>) -> String {
        let (name, side, offset) = port_geometry(compass, span, diagnostics);
        let index = self.node_index(id);
        if self.nodes[index].ports.iter().all(|port| port.id != name) {
            self.nodes[index].ports.push(Port { id: name.to_string(), side, offset });
        }
        name.to_string()
    }
}

/// Create the node if needed and apply the defaults active at this point.
fn ensure_node(collector: &mut Collector, defaults: &Defaults, id: &str) {
    let existed = collector.indices.contains_key(id);
    let index = collector.node_index(id);
    if existed {
        return;
    }
    if let Some(label) = &defaults.node_label {
        collector.nodes[index].label = Some(label.clone());
    }
    if let Some(shape) = defaults.node_shape {
        collector.nodes[index].shape = shape;
    }
}

/// Translate a compass point into a boundary anchor.
///
/// A scene port sits on one side of the node rectangle, so the four corner
/// directions and the centre direction are anchored on the nearest side and
/// reported as approximations instead of pretending to be exact corners.
fn port_geometry(compass: Compass, span: Range<usize>, diagnostics: &mut Vec<Diagnostic>) -> (&'static str, Side, f64) {
    match compass {
        Compass::N => ("n", Side::Top, 0.5),
        Compass::S => ("s", Side::Bottom, 0.5),
        Compass::E => ("e", Side::Right, 0.5),
        Compass::W => ("w", Side::Left, 0.5),
        Compass::NE => {
            diagnostics.push(approximated_at("corner compass `ne` is anchored on the right edge near the top corner", span));
            ("ne", Side::Right, 0.0)
        }
        Compass::NW => {
            diagnostics.push(approximated_at("corner compass `nw` is anchored on the left edge near the top corner", span));
            ("nw", Side::Left, 0.0)
        }
        Compass::SE => {
            diagnostics.push(approximated_at("corner compass `se` is anchored on the right edge near the bottom corner", span));
            ("se", Side::Right, 1.0)
        }
        Compass::SW => {
            diagnostics.push(approximated_at("corner compass `sw` is anchored on the left edge near the bottom corner", span));
            ("sw", Side::Left, 1.0)
        }
        Compass::C => {
            diagnostics.push(approximated_at("center compass `c` has no boundary anchor and uses the default side midpoint", span));
            ("c", Side::Top, 0.5)
        }
    }
}

fn fold_label(label: &str) -> String {
    label.replace("\\n", "\n")
}

fn compass_of(name: &str) -> Option<Compass> {
    let normalized = name.trim().to_ascii_lowercase();
    COMPASSES.iter().find(|(token, _)| *token == normalized).map(|(_, compass)| compass.clone())
}

fn dot_root_to_spec(root: &DotRoot) -> Result<(DiagramSpec, Vec<Diagnostic>), DiagramError> {
    if root.graphs.len() > 1 {
        let second = span_of(root.graphs[1].span);
        return Err(unsupported_at(format!("a DOT document may contain exactly one top-level graph, found {}", root.graphs.len()), second));
    }
    let Some(graph) = root.graphs.first()
    else {
        return Err(DiagramError::empty_diagram("the DOT document declares no graph"));
    };
    if graph.strict {
        return Err(unsupported_at("`strict` graphs change multi-edge semantics that the DOT subset does not model", span_of(graph.span)));
    }

    let mut collector = Collector::default();
    let mut diagnostics = Vec::new();
    let mut defaults = Defaults::default();
    let mut direction = Direction::TopDown;

    for statement in &graph.statements {
        match statement {
            Statement::Attribute(attribute) => {
                if let AttributeTarget::Graph = attribute.target {
                    direction = graph_attributes(&attribute.attributes, direction, &mut diagnostics)?;
                }
            }
            Statement::Assignment(assignment) if assignment.id == "rankdir" => {
                direction = rankdir(&assignment.value, span_of(assignment.span))?;
            }
            Statement::Assignment(assignment) if assignment.id == "label" => {
                diagnostics.push(approximated_at(
                    "a graph-level label has no place in the scene surface and is not rendered",
                    span_of(assignment.span),
                ));
            }
            Statement::Assignment(assignment) => {
                return Err(unsupported_at(
                    format!("graph assignment `{}` is outside the DOT subset", assignment.id),
                    span_of(assignment.span),
                ));
            }
            _ => {}
        }
    }

    for statement in &graph.statements {
        lower_statement(statement, graph.graph_type.clone(), &mut collector, &mut defaults, &mut diagnostics)?;
    }

    if collector.nodes.is_empty() {
        return Err(DiagramError::empty_diagram("the DOT graph declares no nodes"));
    }

    let mut spec = DiagramSpec::new(direction);
    spec.nodes = collector.nodes;
    spec.edges = collector.edges;
    spec.clusters = collector.clusters;
    spec.constraints = collector.constraints;
    Ok((spec, diagnostics))
}

fn rankdir(value: &str, span: Range<usize>) -> Result<Direction, DiagramError> {
    match value.trim().to_ascii_uppercase().as_str() {
        "TB" => Ok(Direction::TopDown),
        "BT" => Ok(Direction::BottomUp),
        "LR" => Ok(Direction::LeftToRight),
        "RL" => Ok(Direction::RightToLeft),
        other => Err(unsupported_at(format!("`rankdir={other}` is not one of `TB`, `BT`, `LR`, `RL`"), span)),
    }
}

fn graph_attributes(attributes: &[Attribute], direction: Direction, diagnostics: &mut Vec<Diagnostic>) -> Result<Direction, DiagramError> {
    let mut direction = direction;
    for attribute in attributes {
        match attribute.name.to_ascii_lowercase().as_str() {
            "rankdir" => {
                let value = attribute.value.as_deref().unwrap_or_default();
                direction = rankdir(value, span_of(attribute.span))?;
            }
            "label" => diagnostics
                .push(approximated_at("a graph-level label has no place in the scene surface and is not rendered", span_of(attribute.span))),
            other => return Err(unsupported_at(format!("graph attribute `{other}` is outside the DOT subset"), span_of(attribute.span))),
        }
    }
    Ok(direction)
}

fn check_attribute(attribute: &Attribute, diagnostics: &mut Vec<Diagnostic>) -> Result<(), DiagramError> {
    let key = attribute.name.to_ascii_lowercase();
    let span = span_of(attribute.span);
    if FORBIDDEN_KEYS.iter().any(|forbidden| *forbidden == key) {
        return Err(unsupported_at(format!("attribute `{key}` is outside the DOT subset"), span));
    }
    if !ALLOWED_ELEMENT_KEYS.iter().any(|allowed| *allowed == key) {
        return Err(unsupported_at(format!("attribute `{key}` is outside the DOT subset"), span));
    }
    let Some(value) = attribute.value.as_deref()
    else {
        return Ok(());
    };
    match key.as_str() {
        "label" => reject_html_label(value, span),
        "shape" => {
            if value.trim().eq_ignore_ascii_case("record") || value.trim().eq_ignore_ascii_case("mrecord") {
                return Err(unsupported_at("record shapes carry field ports that the DOT subset rejects", span));
            }
            Ok(())
        }
        "fillcolor" | "color" | "fontcolor" | "labelfontcolor" => {
            check_color(value, span.clone())?;
            diagnostics.push(approximated_at(format!("`{key}` is validated but the scene surface keeps its default palette"), span));
            Ok(())
        }
        "style" => check_style(value, span, diagnostics),
        _ => Ok(()),
    }
}

fn reject_html_label(value: &str, span: Range<usize>) -> Result<(), DiagramError> {
    let trimmed = value.trim();
    if trimmed.starts_with('<') || trimmed.ends_with('>') {
        return Err(unsupported_at("HTML-like labels are outside the DOT subset", span));
    }
    Ok(())
}

fn check_color(value: &str, span: Range<usize>) -> Result<(), DiagramError> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        let valid = matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|digit| digit.is_ascii_hexdigit());
        if valid {
            return Ok(());
        }
        return Err(unsupported_at(format!("`{value}` is not a hexadecimal color"), span));
    }
    let normalized = value.to_ascii_lowercase();
    if NAMED_COLORS.contains(&normalized.as_str()) {
        return Ok(());
    }
    Err(unsupported_at(format!("color `{value}` is outside the X11 name whitelist"), span))
}

fn check_style(value: &str, span: Range<usize>, diagnostics: &mut Vec<Diagnostic>) -> Result<(), DiagramError> {
    for token in value.split(',') {
        let token = token.trim();
        if !ALLOWED_STYLE_TOKENS.contains(&token) {
            return Err(unsupported_at(format!("style `{token}` is outside the DOT subset"), span));
        }
        if matches!(token, "dashed" | "dotted") {
            diagnostics
                .push(approximated_at(format!("style `{token}` has no dash attribute on the scene surface and is drawn solid"), span.clone()));
        }
        if token == "bold" && !value.contains(',') {
            diagnostics.push(approximated_at("style `bold` is not applied as a stroke weight", span.clone()));
        }
    }
    Ok(())
}

fn lower_statement(
    statement: &Statement,
    graph_type: GraphType,
    collector: &mut Collector,
    defaults: &mut Defaults,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), DiagramError> {
    match statement {
        Statement::Node(node) => {
            lower_node(node, collector, defaults, diagnostics)?;
        }
        Statement::Edge(edge) => {
            lower_edge(edge, &graph_type, collector, defaults, diagnostics)?;
        }
        Statement::Attribute(attribute) => match attribute.target {
            AttributeTarget::Graph => {
                graph_attributes(&attribute.attributes, Direction::TopDown, diagnostics)?;
            }
            AttributeTarget::Node => {
                for child in &attribute.attributes {
                    check_attribute(child, diagnostics)?;
                    match child.name.to_ascii_lowercase().as_str() {
                        "shape" => {
                            defaults.node_shape =
                                Some(node_shape(child.value.as_deref().unwrap_or_default(), span_of(child.span), diagnostics)?);
                        }
                        "label" => {
                            if let Some(value) = &child.value {
                                defaults.node_label = Some(fold_label(value));
                            }
                        }
                        _ => {}
                    }
                }
            }
            AttributeTarget::Edge => {
                for child in &attribute.attributes {
                    check_attribute(child, diagnostics)?;
                    match child.name.to_ascii_lowercase().as_str() {
                        "style" => {
                            defaults.edge_style = Some(style_for(child.value.as_deref().unwrap_or_default(), EdgeStyle::Arrow));
                        }
                        "label" => {
                            if let Some(value) = &child.value {
                                defaults.edge_label = Some(fold_label(value));
                            }
                        }
                        _ => {}
                    }
                }
            }
        },
        Statement::Subgraph(subgraph) => lower_subgraph(subgraph, &graph_type, collector, defaults, diagnostics)?,
        Statement::Assignment(assignment) => {
            if !ALLOWED_GRAPH_KEYS.contains(&assignment.id.as_str()) {
                return Err(unsupported_at(
                    format!("graph assignment `{}` is outside the DOT subset", assignment.id),
                    span_of(assignment.span),
                ));
            }
            if assignment.id == "label" {
                diagnostics.push(approximated_at(
                    "a graph-level label has no place in the scene surface and is not rendered",
                    span_of(assignment.span),
                ));
            }
        }
    }
    Ok(())
}

fn lower_node(
    node: &NodeStatement,
    collector: &mut Collector,
    defaults: &Defaults,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<String, DiagramError> {
    for attribute in &node.attributes {
        check_attribute(attribute, diagnostics)?;
    }
    let endpoint = endpoint(&node.node_id, collector, defaults, diagnostics)?;
    for attribute in &node.attributes {
        let key = attribute.name.to_ascii_lowercase();
        let Some(value) = &attribute.value
        else {
            continue;
        };
        match key.as_str() {
            "label" => collector.set_label(&endpoint.node, value),
            "shape" => collector.set_shape(&endpoint.node, node_shape(value, span_of(attribute.span), diagnostics)?),
            _ => {}
        }
    }
    Ok(endpoint.node)
}

fn lower_edge(
    edge: &oak_dot::ast::EdgeStatement,
    graph_type: &GraphType,
    collector: &mut Collector,
    defaults: &Defaults,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<String>, DiagramError> {
    for attribute in &edge.attributes {
        check_attribute(attribute, diagnostics)?;
    }
    let mut explicit_label = None;
    let mut explicit_style = None;
    for attribute in &edge.attributes {
        match attribute.name.to_ascii_lowercase().as_str() {
            "label" => explicit_label = attribute.value.as_deref().map(fold_label),
            "style" => {
                explicit_style = Some(style_for(attribute.value.as_deref().unwrap_or_default(), EdgeStyle::Arrow));
            }
            _ => {}
        }
    }

    let mut operands = Vec::new();
    operands.push(edge_operand(&edge.left, collector, defaults, diagnostics)?);
    let mut members = Vec::new();
    for (op, operand) in &edge.edges {
        let operand_endpoint = edge_operand(operand, collector, defaults, diagnostics)?;
        match (op, graph_type) {
            (EdgeOp::Directed, GraphType::Graph) => {
                return Err(unsupported_at("a directed `->` edge inside an undirected `graph` is outside the DOT subset", span_of(edge.span)));
            }
            (EdgeOp::Undirected, GraphType::Digraph) => {
                return Err(unsupported_at("an undirected `--` edge inside a `digraph` is outside the DOT subset", span_of(edge.span)));
            }
            _ => {}
        }
        operands.push(operand_endpoint);
        let (from, to) = (&operands[operands.len() - 2], &operands[operands.len() - 1]);
        members.push(from.node.clone());
        members.push(to.node.clone());

        let style = explicit_style.or(defaults.edge_style).unwrap_or(match op {
            EdgeOp::Directed => EdgeStyle::Arrow,
            EdgeOp::Undirected => EdgeStyle::Line,
        });
        collector.edges.push(DiagramEdge {
            from: from.clone(),
            to: to.clone(),
            label: explicit_label.clone().or_else(|| defaults.edge_label.clone()),
            style,
        });
    }
    Ok(members)
}

fn node_shape(value: &str, span: Range<usize>, diagnostics: &mut Vec<Diagnostic>) -> Result<NodeShape, DiagramError> {
    let value = value.trim().to_ascii_lowercase();
    match value.as_str() {
        "box" | "rect" | "rectangle" | "square" | "plaintext" => Ok(NodeShape::Box),
        "diamond" => Ok(NodeShape::Rhombus),
        "circle" => Ok(NodeShape::Circle),
        "ellipse" | "oval" => {
            diagnostics.push(approximated_at("`ellipse` is approximated by a rounded rectangle", span));
            Ok(NodeShape::Round)
        }
        "record" | "mrecord" => Err(unsupported_at("record shapes carry field ports that the DOT subset rejects", span)),
        other => {
            diagnostics.push(approximated_at(format!("shape `{other}` is approximated by a box"), span));
            Ok(NodeShape::Box)
        }
    }
}

fn style_for(value: &str, fallback: EdgeStyle) -> EdgeStyle {
    let mut style = fallback;
    for token in value.split(',') {
        match token.trim() {
            "dashed" | "dotted" => {
                style = match style {
                    EdgeStyle::Arrow | EdgeStyle::ThickArrow | EdgeStyle::HollowArrow => EdgeStyle::DottedArrow,
                    _ => EdgeStyle::DottedLine,
                }
            }
            "bold" => {
                style = match style {
                    EdgeStyle::Arrow | EdgeStyle::DottedArrow => EdgeStyle::ThickArrow,
                    other => other,
                }
            }
            _ => {}
        }
    }
    style
}

/// Resolve one edge operand into a node endpoint.
///
/// Only compass points are accepted as ports. A named port (`:f0`) or a named
/// port with a compass suffix (`:out:n`) is rejected with the port's byte span.
fn endpoint(id: &NodeId, collector: &mut Collector, defaults: &Defaults, diagnostics: &mut Vec<Diagnostic>) -> Result<Endpoint, DiagramError> {
    let node = id.id.trim();
    if node.is_empty() {
        return Err(unsupported_at("an empty node id is outside the DOT subset", span_of(id.span)));
    }
    ensure_node(collector, defaults, node);

    let Some(port) = &id.port
    else {
        return Ok(Endpoint { node: node.to_string(), port: None });
    };
    let span = span_of(port.span);
    let compass = match (&port.id, &port.compass) {
        (Some(name), _) => compass_of(name).ok_or_else(|| {
            unsupported_at(format!("named port `:{name}` is outside the DOT subset, only compass points are accepted"), span.clone())
        })?,
        (None, Some(compass)) => compass.clone(),
        (None, None) => return Ok(Endpoint { node: node.to_string(), port: None }),
    };
    let port = collector.add_port(node, compass, span, diagnostics);
    Ok(Endpoint { node: node.to_string(), port: Some(port) })
}

/// Resolve an edge operand. Subgraphs as operands have no scene meaning here.
fn edge_operand(
    operand: &EdgeOperand,
    collector: &mut Collector,
    defaults: &Defaults,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Endpoint, DiagramError> {
    match operand {
        EdgeOperand::Node(id) => endpoint(id, collector, defaults, diagnostics),
        EdgeOperand::Subgraph(subgraph) => Err(unsupported_at("subgraphs as edge operands are outside the DOT subset", span_of(subgraph.span))),
    }
}

fn lower_subgraph(
    subgraph: &SubgraphStatement,
    graph_type: &GraphType,
    collector: &mut Collector,
    defaults: &Defaults,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), DiagramError> {
    let span = span_of(subgraph.span);
    let Some(id) = &subgraph.id
    else {
        return Err(unsupported_at("anonymous subgraphs are outside the DOT subset", span));
    };
    if subgraph.statements.iter().any(|statement| matches!(statement, Statement::Subgraph(_))) {
        return Err(unsupported_at("nested clusters are outside the DOT subset", span));
    }

    let mut members = Vec::new();
    let mut label = None;
    let mut rank_same = false;
    for statement in &subgraph.statements {
        match statement {
            Statement::Node(node) => {
                members.push(lower_node(node, collector, defaults, diagnostics)?);
            }
            Statement::Edge(edge) => members.extend(lower_edge(edge, graph_type, collector, defaults, diagnostics)?),
            Statement::Assignment(assignment) if assignment.id == "label" => {
                label = Some(fold_label(&assignment.value));
            }
            Statement::Assignment(assignment) if assignment.id == "rank" => {
                if assignment.value.trim() != "same" {
                    return Err(unsupported_at(
                        format!("`rank={}` is outside the DOT subset, only `rank=same` is accepted", assignment.value.trim()),
                        span_of(assignment.span),
                    ));
                }
                rank_same = true;
            }
            Statement::Assignment(assignment) => {
                return Err(unsupported_at(
                    format!("subgraph assignment `{}` is outside the DOT subset", assignment.id),
                    span_of(assignment.span),
                ));
            }
            Statement::Attribute(attribute) => {
                return Err(unsupported_at("attribute statements inside a subgraph are outside the DOT subset", span_of(attribute.span)));
            }
            Statement::Subgraph(nested) => {
                return Err(unsupported_at("nested clusters are outside the DOT subset", span_of(nested.span)));
            }
        }
    }

    let is_cluster = id.starts_with("cluster_");
    if is_cluster {
        let cluster_id = id.trim_start_matches("cluster_").to_string();
        if cluster_id.is_empty() {
            return Err(unsupported_at("`subgraph cluster_` needs a suffix to name the cluster", span));
        }
        if collector.clusters.iter().any(|cluster| cluster.id == cluster_id) {
            return Err(unsupported_at(format!("duplicate cluster id `{cluster_id}`"), span));
        }
        let mut ordered = Vec::new();
        for member in &members {
            if !ordered.contains(member) {
                ordered.push(member.clone());
            }
        }
        for member in &ordered {
            let index = collector.node_index(member);
            collector.nodes[index].cluster = Some(cluster_id.clone());
        }
        collector.clusters.push(Cluster { id: cluster_id, label, members: ordered });
        Ok(())
    }
    else if rank_same {
        let mut ordered = Vec::new();
        for member in members {
            if !ordered.contains(&member) {
                ordered.push(member);
            }
        }
        if !ordered.is_empty() {
            collector.constraints.push(Constraint::SameRank(ordered));
        }
        Ok(())
    }
    else {
        Err(unsupported_at("a non-cluster subgraph is only accepted for `rank=same`, which this subgraph does not declare", span))
    }
}
