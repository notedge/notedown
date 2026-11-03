//! PlantUML class and sequence frontend.
//!
//! This module maps the typed `oak-plantuml` AST onto [`DiagramSpec`]. It never
//! builds scene nodes or SVG. Lowering accepts the documented subset only:
//! `@startuml` / `@enduml` around one class diagram or one sequence diagram,
//! class declarations with members, the six class relation spellings,
//! participant / actor declarations, the four message arrows, and activation
//! spans. Everything outside that set fails with a [`DiagramError`] carrying
//! the offending byte range.

use std::{collections::BTreeSet, ops::Range};

use davinci_scene::SceneResolver;
use davinci_types::{
    ByteRange, Diagnostic, DiagnosticLabel, DiagnosticLocation, DiagnosticSeverity, LabelRole, Message, NAMESPACE, SourceRef, codes, diagnostic,
};
use oak_core::{Builder, SourceText, errors::OakError, parser::session::ParseSession};
use oak_plantuml::{
    PlantUmlBuilder, PlantUmlLanguage,
    ast::{
        ClassDeclaration, ClassDiagram, ClassMember, ClassRelation, ClassRelationKind, MemberVisibility, MessageArrow, ParticipantKind,
        PlantUmlDiagram, PlantUmlRoot, SequenceDiagram,
    },
};

use crate::{
    LoweredDiagram,
    compile::{LayoutOptions, compile},
    error::DiagramError,
    render::render,
    spec::{Activation, DiagramEdge, DiagramNode, DiagramSpec, Direction, EdgeStyle, Endpoint, LayoutModel, NodeShape, SequenceLayout},
};

/// Source identity used by refusal and approximation labels.
const SOURCE: &str = "plantuml";

/// Diagram types PlantUML also understands but this frontend does not lower.
const OTHER_DIAGRAM_KEYWORDS: &[&str] = &[
    "activity",
    "state",
    "component",
    "deployment",
    "usecase",
    "object",
    "timing",
    "gantt",
    "mindmap",
    "wbs",
    "salt",
    "ditaa",
    "nwdiag",
    "rackdiag",
    "packetdiag",
    "archimate",
    "gitgraph",
    "chart",
];

/// Statements refused even inside a class or sequence source.
const REJECTED_STATEMENTS: &[&str] = &[
    "skinparam",
    "note",
    "alt",
    "else",
    "loop",
    "group",
    "par",
    "opt",
    "break",
    "critical",
    "ref",
    "box",
    "title",
    "caption",
    "header",
    "footer",
    "legend",
    "scale",
    "autonumber",
    "newpage",
    "hide",
    "show",
    "left",
    "right",
    "top",
    "bottom",
    "allow_mixing",
];

/// Parse PlantUML source into the typed `oak-plantuml` AST.
///
/// The refusal scan runs before the parser so rejected constructs keep the
/// byte range of the whole line instead of whatever token the generic lexer
/// happened to stop on.
pub fn parse_plantuml(source: &str) -> Result<PlantUmlRoot, DiagramError> {
    reject_unsupported(source)?;
    let text = SourceText::new(source);
    let language = PlantUmlLanguage::default();
    let builder = PlantUmlBuilder::new(&language);
    let mut session = ParseSession::<PlantUmlLanguage>::default();
    let output = builder.build(&text, &[], &mut session);
    let oak_diagnostics = output.diagnostics.len();

    match output.result {
        Ok(root) if root.diagrams.is_empty() => Err(DiagramError::ast_unavailable(format!(
            "`oak-plantuml` accepted {} bytes with {oak_diagnostics} Oak parser diagnostics but produced no diagram",
            source.len()
        ))),
        Ok(root) => Ok(root),
        Err(error) => Err(spanned_parse_error(source, &error)),
    }
}

/// Turn an `oak-plantuml` parser error into a refusal that keeps its span.
///
/// Family conflicts and other rejections are recorded as syntax errors at the
/// offset of the offending statement, so the refused line stays locatable
/// instead of collapsing into an unpositioned "parse failed" message.
fn spanned_parse_error(source: &str, error: &OakError) -> DiagramError {
    if let oak_core::errors::OakErrorKind::SyntaxError { message, offset, .. } = error.kind() {
        let offset = (*offset).min(source.len());
        let end = source[offset..].find('\n').map(|delta| offset + delta).unwrap_or(source.len());
        return DiagramError::unsupported(message.clone()).with_span(offset..end);
    }
    DiagramError::parse_failed(format!("`oak-plantuml` failed to build an AST: {error}"))
}

/// Lower PlantUML source into a source-independent diagram spec.
pub fn plantuml_to_spec(source: &str) -> Result<DiagramSpec, DiagramError> {
    let root = parse_plantuml(source)?;
    plantuml_root_to_spec(&root).map(|(spec, _)| spec)
}

/// Lower PlantUML source to a layout plan and resolved display scene.
///
/// Class diagrams use the same layered pass as Mermaid class diagrams and DOT
/// graphs. Sequence diagrams declare their time-ordered layout, so activation
/// spans reach the shared sequence compiler instead of being dropped.
pub fn plantuml_to_scene(source: &str) -> Result<LoweredDiagram, DiagramError> {
    let root = parse_plantuml(source)?;
    let (spec, mut diagnostics) = plantuml_root_to_spec(&root)?;
    let plan = compile(&spec, &LayoutOptions::default());
    diagnostics.extend(plan.diagnostics.iter().cloned());
    let semantic = render(&plan);
    let scene = semantic.resolve(&SceneResolver::default());
    Ok(LoweredDiagram { plan, scene, diagnostics })
}

fn single_diagram(root: &PlantUmlRoot) -> Result<&PlantUmlDiagram, DiagramError> {
    if root.diagrams.len() > 1 {
        return Err(DiagramError::unsupported(format!(
            "a PlantUML root with {} diagrams is not lowered, exactly one diagram is supported",
            root.diagrams.len()
        )));
    }
    root.diagrams.first().ok_or_else(|| DiagramError::ast_unavailable("PlantUML source produced no diagrams"))
}

fn plantuml_root_to_spec(root: &PlantUmlRoot) -> Result<(DiagramSpec, Vec<Diagnostic>), DiagramError> {
    match single_diagram(root)? {
        PlantUmlDiagram::Class(class) => class_spec(class),
        PlantUmlDiagram::Sequence(sequence) => sequence_spec(sequence).map(|spec| (spec, Vec::new())),
    }
}

/// Lower one PlantUML class diagram into the graph spec used by layered layout.
///
/// A class becomes a `Box` node whose label starts with the authored class
/// name followed by one line per member, so members travel through the existing
/// multiline label measurement. A class that appears only in a relation becomes
/// an implicit node.
///
/// The AST already reports which operand carries the UML end marker, so
/// `A <|-- B` and `A --|> B` point at the same superclass without a second
/// scan of the source text.
fn class_spec(diagram: &ClassDiagram) -> Result<(DiagramSpec, Vec<Diagnostic>), DiagramError> {
    let mut spec = DiagramSpec::new(Direction::BottomUp);
    let mut diagnostics = Vec::new();
    let mut known: BTreeSet<String> = BTreeSet::new();

    for class in &diagram.classes {
        let name = class.name.trim();
        if name.is_empty() {
            continue;
        }
        spec.nodes.push(DiagramNode {
            id: name.to_string(),
            label: Some(class_label(class)),
            shape: NodeShape::Box,
            ports: Vec::new(),
            cluster: None,
        });
        known.insert(name.to_string());
    }

    for relation in &diagram.relations {
        let left = relation.left.trim();
        let right = relation.right.trim();
        if left.is_empty() || right.is_empty() {
            return Err(DiagramError::unsupported("class relation has an empty class name").with_span(relation.span.clone().into()));
        }
        for endpoint in [left, right] {
            if known.insert(endpoint.to_string()) {
                spec.nodes.push(DiagramNode { id: endpoint.to_string(), label: None, shape: NodeShape::Box, ports: Vec::new(), cluster: None });
            }
        }

        let (from, to) = if relation.marker_at_left { (right, left) } else { (left, right) };
        if relation.left_multiplicity.is_some() || relation.right_multiplicity.is_some() {
            diagnostics.push(approximated_at(
                "class relation multiplicities share the single edge label because the spec has one label per edge",
                relation.span.clone().into(),
            ));
        }
        spec.edges.push(DiagramEdge {
            from: Endpoint { node: from.to_string(), port: None },
            to: Endpoint { node: to.to_string(), port: None },
            label: relation_label(relation),
            style: class_edge_style(&relation.kind),
        });
    }

    if spec.nodes.is_empty() {
        return Err(DiagramError::empty_diagram("class diagram contains no classes or relations"));
    }
    Ok((spec, diagnostics))
}

/// Lower one PlantUML sequence diagram into the time-ordered sequence spec.
///
/// Participants keep their declaration order, messages their source order, and
/// activation spans the indices the AST recorded while scanning. A message
/// endpoint with no declaration becomes an implicit participant, matching the
/// Mermaid sequence lowering.
fn sequence_spec(diagram: &SequenceDiagram) -> Result<DiagramSpec, DiagramError> {
    let mut spec = DiagramSpec::new(Direction::TopDown);
    let mut known: BTreeSet<String> = BTreeSet::new();

    for participant in &diagram.participants {
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

    for message in &diagram.messages {
        let from = message.from.trim();
        let to = message.to.trim();
        if from.is_empty() || to.is_empty() {
            return Err(DiagramError::unsupported("sequence message has an empty participant id").with_span(message.span.clone().into()));
        }
        for endpoint in [from, to] {
            if known.insert(endpoint.to_string()) {
                spec.nodes.push(DiagramNode { id: endpoint.to_string(), label: None, shape: NodeShape::Box, ports: Vec::new(), cluster: None });
            }
        }
        spec.edges.push(DiagramEdge {
            from: Endpoint { node: from.to_string(), port: None },
            to: Endpoint { node: to.to_string(), port: None },
            label: message.text.as_deref().map(str::trim).filter(|text| !text.is_empty()).map(str::to_string),
            style: message_style(&message.arrow),
        });
    }

    let message_count = spec.edges.len();
    let mut activations = Vec::with_capacity(diagram.activations.len());
    for span in &diagram.activations {
        if span.from_message > span.to_message || span.to_message >= message_count {
            return Err(DiagramError::unsupported(format!(
                "sequence activation span {}..={} is outside the {} authored messages",
                span.from_message, span.to_message, message_count
            )));
        }
        activations.push(Activation { participant: span.participant.clone(), from_message: span.from_message, to_message: span.to_message });
    }

    if spec.nodes.is_empty() {
        return Err(DiagramError::empty_diagram("sequence diagram contains no participants"));
    }
    spec.layout = LayoutModel::TimeOrdered(SequenceLayout {
        participants: spec.nodes.iter().map(|node| node.id.clone()).collect(),
        messages: (0..message_count).collect(),
        activations,
    });
    Ok(spec)
}

fn class_label(class: &ClassDeclaration) -> String {
    let mut label =
        class.display.as_deref().map(str::trim).filter(|display| !display.is_empty()).unwrap_or_else(|| class.name.trim()).to_string();
    for member in &class.members {
        let text = member_text(member);
        if !text.is_empty() {
            label.push('\n');
            label.push_str(&text);
        }
    }
    label
}

/// Rebuild the authored member line, including the visibility marker.
///
/// The Mermaid class lowering keeps the marker in the member text, so keeping
/// it here lets both frontends produce the same node label for the same class.
fn member_text(member: &ClassMember) -> String {
    let marker = match member.visibility {
        MemberVisibility::Public => '+',
        MemberVisibility::Private => '-',
        MemberVisibility::Protected => '#',
        MemberVisibility::Package => '~',
    };
    let text = member.text.trim();
    if text.is_empty() { String::new() } else { format!("{marker}{text}") }
}

/// Fold multiplicities and the authored label into the one edge label.
fn relation_label(relation: &ClassRelation) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(left) = &relation.left_multiplicity {
        parts.push(left.trim().to_string());
    }
    if let Some(right) = &relation.right_multiplicity {
        parts.push(right.trim().to_string());
    }
    if let Some(label) = &relation.label {
        let label = label.trim();
        if !label.is_empty() {
            parts.push(label.to_string());
        }
    }
    parts.retain(|part| !part.is_empty());
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn class_edge_style(kind: &ClassRelationKind) -> EdgeStyle {
    match kind {
        ClassRelationKind::Association => EdgeStyle::Arrow,
        ClassRelationKind::Inheritance => EdgeStyle::HollowArrow,
        ClassRelationKind::Composition => EdgeStyle::FilledDiamond,
        ClassRelationKind::Aggregation => EdgeStyle::HollowDiamond,
        ClassRelationKind::Dependency => EdgeStyle::DottedArrow,
        ClassRelationKind::Link => EdgeStyle::Line,
    }
}

fn message_style(arrow: &MessageArrow) -> EdgeStyle {
    match arrow {
        MessageArrow::Solid => EdgeStyle::Line,
        MessageArrow::Dotted => EdgeStyle::DottedLine,
        MessageArrow::SolidArrow => EdgeStyle::Arrow,
        MessageArrow::DottedArrow => EdgeStyle::DottedArrow,
    }
}

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

/// Reject everything outside the accepted PlantUML subset with the line span.
///
/// `oak-plantuml` keeps reusable tokens generic, so a rejected directive would
/// otherwise reach the AST as identifiers and lose the authored range. Scanning
/// lines before the builder runs is the same strategy the DOT frontend uses for
/// HTML labels and is not a second grammar.
fn reject_unsupported(source: &str) -> Result<(), DiagramError> {
    let mut start: Option<Range<usize>> = None;
    let mut end: Option<Range<usize>> = None;
    let mut offset = 0;

    for raw in source.split_inclusive('\n') {
        let trimmed_start = raw.len() - raw.trim_start().len();
        let line_start = offset + trimmed_start;
        let line_end = offset + raw.trim_end().len();
        let line = raw.trim();
        offset += raw.len();

        if line.is_empty() || line.starts_with('\'') {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("@start") {
            if lower.starts_with("@startuml") {
                if start.is_some() {
                    return Err(
                        DiagramError::unsupported("a PlantUML source declares more than one `@startuml`").with_span(line_start..line_end)
                    );
                }
                start = Some(line_start..line_end);
                continue;
            }
            return Err(DiagramError::unsupported("only `@startuml` is supported, other PlantUML start markers are refused")
                .with_span(line_start..line_end));
        }
        if lower.starts_with("@enduml") {
            if end.is_some() {
                return Err(DiagramError::unsupported("a PlantUML source declares more than one `@enduml`").with_span(line_start..line_end));
            }
            end = Some(line_start..line_end);
            continue;
        }
        if lower.starts_with('@') {
            return Err(DiagramError::unsupported(format!("unknown PlantUML marker `{line}`")).with_span(line_start..line_end));
        }
        if start.is_none() {
            return Err(
                DiagramError::unsupported("PlantUML content must be wrapped by `@startuml` and `@enduml`").with_span(line_start..line_end)
            );
        }
        if end.is_some() {
            return Err(DiagramError::unsupported("PlantUML content must not follow `@enduml`").with_span(line_start..line_end));
        }
        if line.starts_with('!') {
            return Err(DiagramError::unsupported("PlantUML preprocessor directives such as `!include` and `!define` are refused")
                .with_span(line_start..line_end));
        }
        let keyword = first_word(&lower);
        if OTHER_DIAGRAM_KEYWORDS.contains(&keyword.as_str()) {
            return Err(DiagramError::unsupported(format!(
                "PlantUML `{keyword}` diagrams are refused, only class and sequence diagrams are lowered"
            ))
            .with_span(line_start..line_end));
        }
        if REJECTED_STATEMENTS.contains(&keyword.as_str()) {
            return Err(DiagramError::unsupported(format!(
                "PlantUML `{keyword}` statements are not part of the accepted class and sequence subset"
            ))
            .with_span(line_start..line_end));
        }
    }

    let Some(start) = start
    else {
        return Err(DiagramError::unsupported("PlantUML source is missing `@startuml`"));
    };
    if end.is_none() {
        return Err(DiagramError::unsupported("PlantUML source is missing `@enduml`").with_span(start));
    }
    Ok(())
}

fn first_word(line: &str) -> String {
    line.split(|character: char| character.is_whitespace() || character == '{' || character == '(').next().unwrap_or("").to_string()
}
