//! TeX/LaTeX import via `oak-tex`.
//!
//! The lowering keeps document semantics the IR can express: sections,
//! paragraphs, inline styles, lists, simple tables, math source, and verbatim
//! code. Everything that only makes sense to a TeX engine (macro definitions,
//! packages, floats, arbitrary layout commands) stays an explicit opaque block
//! or a recorded loss marker instead of being silently dropped.
//!
//! Two AST constructs have no dedicated IR semantic and are carried
//! structurally instead of being flattened into text:
//!
//! - `\left...\right` pairs keep the authored opening and closing delimiters as
//!   `tex-delimiter-pair` attributes around the lowered body content.
//! - `\limits` / `\nolimits` markers become adjacent `tex-limits` inline
//!   markers plus a loss marker, because the IR has no large-operator
//!   attachment.
//! - Text-mode spacing commands (`\,`, `\:`, `\;`, `\!`, `\quad`, `\qquad`)
//!   become `tex-space` inline markers whose `data-tex-space` value keeps the
//!   authored distinction (thin, medium, thick, negative, quad, double-quad)
//!   plus a loss marker, because the IR has no typographic space node. Inside
//!   math these commands stay verbatim in the preserved math source.
//!
//! Math mode keeps one authored source per semantic node: `$...$` and
//! `\(...\)` both become inline math, `$$...$$` and `\[...\]` both become a
//! display math block, and a math environment (`equation`, `align`, `cases`,
//! `aligned`, ...) becomes a display math block holding the whole
//! `\begin{...}...\end{...}` source, so environment type, alignment tabs and row
//! breaks survive for the math layout stage. Equation numbering has no IR field,
//! so a numbered environment records that gap as a loss marker instead of
//! dropping it silently.
//!
//! Paper-level document semantics are lowered, not preserved as syntax:
//! `\title` and `\author` fill `DocumentMetadata`, `\date` and `\maketitle`
//! record a loss because the metadata has no field for them, sectioning
//! commands carry a computed number (`1`, `1.2`) as a `tex-section-number`
//! marker in the section title, `\label` registers a `References` relation on
//! the preceding block, `\ref` and `\eqref` become links whose display text is
//! the resolved number with forward references rewritten in a post-pass,
//! `\footnote` becomes a numbered `Inline::Note` carrying its lowered body,
//! and `\cite` becomes `Inline::Citation` plus one `Cites` relation per key.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use notedown_ir::{
    AttributePair, Attributes, Block, DocumentGraph, DocumentId, DocumentMetadata, Inline, LinkId, ListItem, LossMarker, NodeId, Relation,
    RelationEndpoint, RelationKind, SemanticStatus, SourceKind, SourceRef, TableCell, TableRow,
};
use oak_core::{Builder, SourceText, parser::session::ParseSession};
use oak_tex::{
    TexBuilder, TexLanguage,
    ast::{MathDelimiter, TexArgument, TexCommand, TexEnvironment, TexItem, TexLeftRightPair, TexLimits, TexMath, TexRoot},
};

use super::common::register_image_assets;
use crate::FormatError;

/// Import a TeX file from disk into `notedown-ir`.
pub fn import_tex(path: impl AsRef<Path>) -> Result<DocumentGraph, FormatError> {
    let path = path.as_ref();
    let text =
        std::fs::read_to_string(path).map_err(|error| FormatError::invalid_input(format!("failed to read {}: {error}", path.display())))?;
    import_tex_bytes(&path.display().to_string(), &text)
}

/// Import TeX source text into `notedown-ir`.
pub fn import_tex_bytes(label: &str, text: &str) -> Result<DocumentGraph, FormatError> {
    let root = parse_tex(label, text)?;
    let mut lowerer = TexLowerer { graph: DocumentGraph::new(document_id_for(label)), source: text, state: TexDocState::default() };
    let mut sink = Sink::document();
    lowerer.lower_root(&root.items, 0, text.len(), &mut sink);
    sink.flush(&mut lowerer.graph);

    for label in resolve_pending_references(&mut lowerer.graph, &lowerer.state.labels) {
        lowerer.loss(
            "import.tex.unresolved_reference",
            format!("reference target has no semantic node identity: {label}"),
            SemanticStatus::Unresolved,
        );
    }
    if lowerer.state.title.is_some() || !lowerer.state.authors.is_empty() {
        let state = &mut lowerer.state;
        lowerer.graph.set_metadata(DocumentMetadata {
            title: state.title.take(),
            language: None,
            authors: std::mem::take(&mut state.authors),
            tags: Vec::new(),
        });
    }

    if lowerer.graph.blocks.is_empty() {
        lowerer.loss("import.tex.empty_document", "TeX source produced no supported semantic blocks", SemanticStatus::Partial);
    }
    lowerer.loss(
        "import.tex.partial_coverage",
        "TeX import preserves metadata, numbered sections, paragraphs, inline styles, lists, simple tabular tables, math source, verbatim code, label anchors, references, footnotes, and citations, but not macros, packages, floats, or arbitrary layout semantics",
        SemanticStatus::Partial,
    );
    register_image_assets(&mut lowerer.graph);
    Ok(lowerer.graph)
}

fn parse_tex(label: &str, text: &str) -> Result<TexRoot, FormatError> {
    let source = SourceText::new(text);
    let language = TexLanguage::default();
    let builder = TexBuilder::new(&language);
    let mut session = ParseSession::<TexLanguage>::default();
    builder.build(&source, &[], &mut session).result.map_err(|error| FormatError::parse("tex", format!("{label}: {error}")))
}

fn document_id_for(label: &str) -> DocumentId {
    let mut hash = 1u64;
    for byte in label.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u64::from(byte));
    }
    DocumentId(hash)
}

/// Inline and child-block accumulator shared by document, environment, and
/// list-item lowering.
struct Sink {
    inlines: Vec<Inline>,
    item_content: Vec<Inline>,
    children: Vec<NodeId>,
    content_taken: bool,
    item_mode: bool,
}

impl Sink {
    fn document() -> Self {
        Self { inlines: Vec::new(), item_content: Vec::new(), children: Vec::new(), content_taken: false, item_mode: false }
    }

    fn item() -> Self {
        Self { inlines: Vec::new(), item_content: Vec::new(), children: Vec::new(), content_taken: false, item_mode: true }
    }

    fn flush(&mut self, graph: &mut DocumentGraph) {
        trim_inlines(&mut self.inlines);
        if self.inlines.is_empty() {
            return;
        }
        let inlines = std::mem::take(&mut self.inlines);
        if self.item_mode && !self.content_taken {
            self.item_content = inlines;
            self.content_taken = true;
        }
        else if self.item_mode {
            let id = graph.push_block(Block::Paragraph { content: inlines });
            self.children.push(id);
        }
        else {
            graph.push_block(Block::Paragraph { content: inlines });
        }
    }
}

struct TexLowerer<'a> {
    graph: DocumentGraph,
    source: &'a str,
    state: TexDocState,
}

/// Document-level lowering state shared by the whole TeX import: metadata,
/// section, equation, and footnote counters, label anchors, and cited keys.
#[derive(Default)]
struct TexDocState {
    title: Option<String>,
    authors: Vec<String>,
    counters: [u32; 4],
    equation: u32,
    footnote: u32,
    last_block: Option<NodeId>,
    last_number: Option<String>,
    labels: HashMap<String, LabelInfo>,
    cited: HashSet<String>,
    next_link: u64,
}

/// Anchor a `\label` registers: the bound block and the computed number when
/// the target carries one.
#[derive(Clone)]
struct LabelInfo {
    node: Option<NodeId>,
    number: Option<String>,
}

impl TexDocState {
    fn next_link_id(&mut self) -> LinkId {
        self.next_link += 1;
        LinkId(self.next_link)
    }
}

impl<'a> TexLowerer<'a> {
    fn raw(&self, start: usize, end: usize) -> &'a str {
        self.source.get(start.min(self.source.len())..end.min(self.source.len())).unwrap_or("")
    }

    fn loss(&mut self, code: &str, message: impl Into<String>, status: SemanticStatus) {
        self.graph.push_loss(LossMarker { code: code.into(), message: message.into(), status });
    }

    fn push_block(&mut self, sink: &mut Sink, block: Block) -> NodeId {
        sink.flush(&mut self.graph);
        let id = self.graph.push_block(block);
        self.state.last_block = Some(id);
        if sink.item_mode {
            sink.children.push(id);
        }
        id
    }

    /// Push a block that came from one contiguous authored span and record that
    /// span as its provenance.
    fn push_spanned_block(&mut self, sink: &mut Sink, block: Block, start: usize, end: usize) -> NodeId {
        let id = self.push_block(sink, block);
        self.graph.attach_source(SourceRef {
            node: id,
            kind: SourceKind::TextSpan { start: start as u32, end: end as u32 },
            precision: SemanticStatus::Resolved,
        });
        id
    }

    /// Lower a run of AST items while also recovering block environments that
    /// `oak-tex` drops from the AST (lists and tables become element kinds the
    /// builder does not emit), using the raw source spans between items.
    fn lower_root(&mut self, items: &[TexItem], region_start: usize, region_end: usize, sink: &mut Sink) {
        let mut cursor = region_start;
        let mut skip_until = 0usize;
        let mut after_comment = false;
        for item in items {
            let Some((start, end)) = item_span(item) else {
                continue;
            };
            if start < skip_until {
                continue;
            }
            if !self.lower_gap(cursor, start, sink, after_comment) {
                self.lower_dropped_regions(cursor, start, sink);
            }
            after_comment = matches!(item, TexItem::Comment { .. });
            if let TexItem::Command(command) = item {
                if command_name(self.raw(command.span.start, command.span.end)) == "verb" {
                    skip_until = command.span.end + self.verb_consumed_len(command.span.end);
                }
            }
            self.lower_item(item, sink);
            cursor = end;
        }
        self.lower_dropped_regions(cursor, region_end, sink);
    }

    fn lower_item(&mut self, item: &TexItem, sink: &mut Sink) {
        match item {
            TexItem::Text { content, .. } => self.push_text(content, sink),
            // Comments carry no document semantics, so dropping them is intended.
            TexItem::Comment { .. } => {}
            TexItem::Group(group) => {
                let inlines = self.collect_inlines(&group.content.items, group.span.start, group.span.end, false);
                sink.inlines.extend(inlines);
            }
            TexItem::Superscript(superscript) => {
                let children = self.collect_inlines(&superscript.content.items, superscript.span.start, superscript.span.end, true);
                sink.inlines.push(Inline::Styled { style: "superscript".into(), children });
            }
            TexItem::Subscript(subscript) => {
                let children = self.collect_inlines(&subscript.content.items, subscript.span.start, subscript.span.end, true);
                sink.inlines.push(Inline::Styled { style: "subscript".into(), children });
            }
            TexItem::Math(math) => {
                let (content, display) = self.math_source(math);
                if display {
                    self.push_spanned_block(sink, Block::Math { content, language: Some("latex".into()) }, math.span.start, math.span.end);
                }
                else {
                    sink.inlines.push(Inline::InlineMath { content, language: Some("latex".into()) });
                }
            }
            TexItem::Command(command) => self.lower_command(command, sink),
            TexItem::Environment(environment) => self.lower_environment(environment, sink),
            TexItem::LeftRightPair(pair) => {
                let inline = self.left_right_inline(pair);
                sink.inlines.push(inline);
            }
            TexItem::Limits(limits) => {
                let inline = self.limits_inline(limits);
                sink.inlines.push(inline);
            }
        }
    }

    /// Lower a `\left...\right` pair.
    ///
    /// The pair keeps its authored delimiters as inline attributes while the
    /// body keeps its own inline structure. An invisible delimiter stays the
    /// authored `"."` so it cannot be confused with a missing delimiter.
    fn left_right_inline(&mut self, pair: &TexLeftRightPair) -> Inline {
        if pair.left_delim.is_empty() || pair.right_delim.is_empty() {
            self.loss(
                "import.tex.unclosed_delimiters",
                format!("`\\left` pair has no readable delimiter pair: left `{}` right `{}`", pair.left_delim, pair.right_delim),
                SemanticStatus::Unsupported,
            );
        }
        let children = self.collect_inlines(&pair.content.items, pair.span.start, pair.span.end, true);
        Inline::Attributed {
            attributes: Attributes {
                id: None,
                classes: vec!["tex-delimiter-pair".into()],
                pairs: vec![
                    AttributePair { key: "data-tex-left".into(), value: pair.left_delim.clone() },
                    AttributePair { key: "data-tex-right".into(), value: pair.right_delim.clone() },
                ],
            },
            children,
        }
    }

    /// Lower a `\limits` / `\nolimits` marker.
    ///
    /// The marker is recorded as an adjacent inline because the IR has no
    /// large-operator attachment, and a loss marker states that attachment
    /// limitation instead of discarding the construct.
    fn limits_inline(&mut self, limits: &TexLimits) -> Inline {
        let value = if limits.is_limits { "limits" } else { "nolimits" };
        self.loss(
            "import.tex.limits_marker",
            format!("\\{value} marker is kept as an inline marker because the IR has no large-operator attachment"),
            SemanticStatus::Partial,
        );
        Inline::Attributed {
            attributes: Attributes {
                id: None,
                classes: vec!["tex-limits".into()],
                pairs: vec![AttributePair { key: "data-tex-limits".into(), value: value.into() }],
            },
            children: Vec::new(),
        }
    }

    /// Math source and display flag of one authored math span.
    fn math_source(&mut self, math: &TexMath) -> (String, bool) {
        let raw = self.raw(math.span.start, math.span.end);
        match math_source_strip(raw, math.delimiter) {
            Some(body) => (body.trim().to_string(), math.is_display),
            None => {
                let (open, close) = math_delimiters(math.delimiter);
                self.loss(
                    "import.tex.unclosed_math",
                    format!("math span opened with `{open}` has no closing `{close}`, the authored body is kept"),
                    SemanticStatus::Unsupported,
                );
                let body = raw.trim().strip_prefix(open).unwrap_or(raw.trim());
                (body.trim().to_string(), math.is_display)
            }
        }
    }

    fn lower_command(&mut self, command: &TexCommand, sink: &mut Sink) {
        let raw = self.raw(command.span.start, command.span.end);
        let name = command_name(raw);
        if let Some(level) = section_level(&name) {
            let mut title = self
                .required_argument(command)
                .map(|fragment| self.lower_inline_fragment(fragment))
                .unwrap_or_default();
            if command.starred {
                self.state.last_number = None;
                self.loss(
                    "import.tex.starred_command",
                    format!("`\\{name}*` is unnumbered, but the IR section kind has no numbering state"),
                    SemanticStatus::Partial,
                );
            }
            if title.is_empty() {
                self.loss(
                    "import.tex.section_without_title",
                    format!("section command has no readable title: {}", raw.trim()),
                    SemanticStatus::Partial,
                );
            }
            if !command.starred {
                let number = self.next_section_number(level);
                self.state.last_number = Some(number.clone());
                title.insert(0, Inline::Text { text: " ".into() });
                title.insert(
                    0,
                    Inline::Attributed {
                        attributes: Attributes {
                            id: None,
                            classes: vec!["tex-section-number".into()],
                            pairs: vec![AttributePair { key: "data-tex-number".into(), value: number.clone() }],
                        },
                        children: vec![Inline::Text { text: number }],
                    },
                );
            }
            self.push_block(sink, Block::Section { level, title, children: Vec::new() });
            return;
        }
        match name.as_str() {
            "par" => sink.flush(&mut self.graph),
            "verb" => self.lower_verb(command, sink),
            _ => match self.inline_from_command(command) {
                InlineOutcome::Inline(inline) => sink.inlines.push(inline),
                InlineOutcome::Inlines(list) => sink.inlines.extend(list),
                InlineOutcome::Dropped => {}
                InlineOutcome::Unknown => {
                    let kind = if name.is_empty() { "tex.unknown".to_string() } else { format!("tex.{name}") };
                    if is_macro_definition(&name) {
                        self.loss(
                            "import.tex.macro_definition",
                            format!("TeX macro definition kept opaque: {}", raw.trim()),
                            SemanticStatus::Unsupported,
                        );
                    }
                    self.push_block(
                        sink,
                        Block::Opaque { kind, payload_hint: raw.trim().to_string(), status: SemanticStatus::Partial },
                    );
                }
            },
        }
    }

    fn lower_environment(&mut self, environment: &TexEnvironment, sink: &mut Sink) {
        let raw = self.raw(environment.span.start, environment.span.end);
        let name = environment.name.clone();
        self.check_environment_boundaries(environment);
        if environment.is_math {
            self.lower_math_environment(environment, raw, sink);
            return;
        }
        match name.as_str() {
            "document" => self.lower_root(&environment.content.items, environment.span.start, environment.span.end, sink),
            "verbatim" | "lstlisting" | "minted" | "Verbatim" => {
                let content = self.code_environment_content(environment.span.start, environment.span.end);
                self.push_spanned_block(sink, Block::Code { language: None, content }, environment.span.start, environment.span.end);
            }
            "itemize" | "enumerate" | "description" => self.lower_list(&name, environment.span.start, environment.span.end, sink),
            "tabular" | "array" => self.lower_table(environment.span.start, environment.span.end, sink),
            _ => {
                self.loss(
                    "import.tex.unknown_environment",
                    format!("unknown TeX environment kept opaque: {}", environment.name),
                    SemanticStatus::Unsupported,
                );
                let kind = if name.is_empty() { "tex.environment.unknown".to_string() } else { format!("tex.environment.{name}") };
                self.push_block(
                    sink,
                    Block::Opaque { kind, payload_hint: raw.trim().to_string(), status: SemanticStatus::Partial },
                );
            }
        }
    }

    /// Lower a recognized math environment.
    ///
    /// The whole `\begin{...}...\end{...}` source is kept as one math payload, so
    /// the environment name, its star variant, alignment tabs and row breaks stay
    /// available to the math layout stage instead of being flattened into plain
    /// text. `math` itself is inline math, every other recognized math
    /// environment is a display math block.
    fn lower_math_environment(&mut self, environment: &TexEnvironment, raw: &'a str, sink: &mut Sink) {
        let stated = if environment.starred { format!("{}*", environment.name) } else { environment.name.clone() };
        if math_environment_is_numbered(&environment.name, environment.starred) {
            self.loss(
                "import.tex.math_numbering",
                format!("math environment `{stated}` carries an equation number, but `Block::Math` has no number or anchor field to hold it"),
                SemanticStatus::Partial,
            );
        }
        if environment.name == "math" {
            let inlines = self.collect_inlines(&environment.content.items, environment.span.start, environment.span.end, true);
            sink.inlines.extend(inlines);
            return;
        }
        let node = self.push_spanned_block(
            sink,
            Block::Math { content: raw.trim().to_string(), language: Some("latex".into()) },
            environment.span.start,
            environment.span.end,
        );
        self.register_math_labels(raw, &environment.name, environment.starred, node);
    }

    /// Increment the section counter for `level`, reset deeper levels, and
    /// format the dotted number (`1`, `1.2`, `1.2.3`).
    fn next_section_number(&mut self, level: u8) -> String {
        let level = (level as usize).min(self.state.counters.len() - 1);
        self.state.counters[level] += 1;
        for deeper in (level + 1)..self.state.counters.len() {
            self.state.counters[deeper] = 0;
        }
        (1..=level).map(|current| self.state.counters[current].to_string()).collect::<Vec<_>>().join(".")
    }

    /// Register the `\label` anchors carried by a math environment.
    ///
    /// The environment body stays one verbatim payload, so its labels can only
    /// be found by scanning the authored source at the environment's own
    /// nesting depth. Each numbered row consumes one equation counter slot, a
    /// row carrying `\notag` or `\nonumber` consumes none, and `\tag` replaces
    /// the displayed number.
    fn register_math_labels(&mut self, raw: &str, name: &str, starred: bool, node: NodeId) {
        let numbered = math_environment_is_numbered(name, starred);
        let (rows, tag) = scan_math_body(raw);
        let mut first_number: Option<String> = None;
        for row in rows {
            if raw[row.start..row.end].trim().is_empty() && row.labels.is_empty() && !row.notag {
                continue;
            }
            let number = if numbered && !row.notag {
                self.state.equation += 1;
                let display = tag.clone().unwrap_or_else(|| self.state.equation.to_string());
                if first_number.is_none() {
                    first_number = Some(display.clone());
                }
                Some(display)
            }
            else {
                None
            };
            for label in row.labels {
                if label.is_empty() {
                    continue;
                }
                self.state.labels.insert(label.clone(), LabelInfo { node: Some(node), number: number.clone() });
                let id = self.state.next_link_id();
                self.graph.push_relation(Relation {
                    id,
                    kind: RelationKind::References,
                    source: RelationEndpoint::Unresolved { label },
                    target: RelationEndpoint::Node(node),
                });
            }
        }
        self.state.last_number = first_number;
    }

    /// Compare the authored `\begin{...}` and `\end{...}` names of an environment.
    ///
    /// The parser matches the closing keyword without checking the name, so a
    /// mismatched or missing closing marker has to be reported here instead of
    /// passing as a well-formed environment.
    fn check_environment_boundaries(&mut self, environment: &TexEnvironment) {
        let raw = self.raw(environment.span.start, environment.span.end);
        let Some((begin_name, _)) = parse_begin(raw.trim_start()) else {
            return;
        };
        let Some(footer) = raw.rfind("\\end{") else {
            self.loss("import.tex.unclosed_environment", format!("environment `{begin_name}` has no closing marker"), SemanticStatus::Unsupported);
            return;
        };
        let end_name = raw[footer..].strip_prefix("\\end{").and_then(|rest| rest.split('}').next()).map(|name| name.trim()).unwrap_or("");
        if end_name != begin_name {
            self.loss(
                "import.tex.mismatched_end",
                format!("environment opens as `{begin_name}` but closes as `{end_name}`"),
                SemanticStatus::Unsupported,
            );
        }
    }

    /// Lower a command in inline position.
    fn inline_from_command(&mut self, command: &TexCommand) -> InlineOutcome {
        let raw = self.raw(command.span.start, command.span.end);
        let name = command_name(raw);
        let styled = |style: &str, children: Vec<Inline>| Inline::Styled { style: style.into(), children };
        let plain = match name.as_str() {
            "&" => Some("&"),
            "%" => Some("%"),
            "$" => Some("$"),
            "#" => Some("#"),
            "_" => Some("_"),
            "{" => Some("{"),
            "}" => Some("}"),
            " " | "space" => Some(" "),
            _ => None,
        };
        if let Some(text) = plain {
            return InlineOutcome::Inline(Inline::Text { text: text.into() });
        }
        if name == "\\" || name == "newline" {
            return InlineOutcome::Inline(Inline::Break { kind: notedown_ir::BreakKind::Hard });
        }
        match name.as_str() {
            "textbf" | "textsc" | "texttt" | "underline" | "textit" | "emph" | "text" => {
                let children = self.required_argument(command).map(|fragment| self.lower_inline_fragment(fragment)).unwrap_or_default();
                if children.is_empty() {
                    return InlineOutcome::Dropped;
                }
                let style = match name.as_str() {
                    "textbf" => "bold",
                    "textit" | "emph" => "italic",
                    "textsc" => "small-caps",
                    "texttt" => "monospace",
                    "underline" => "underline",
                    _ => "text",
                };
                InlineOutcome::Inline(styled(style, children))
            }
            "title" => {
                let text = self.argument_text(command);
                if text.is_empty() {
                    self.loss("import.tex.empty_title", "title command has no readable text", SemanticStatus::Partial);
                }
                else {
                    self.state.title = Some(text);
                }
                InlineOutcome::Dropped
            }
            "author" => {
                let text = self.argument_text(command);
                if text.is_empty() {
                    self.loss("import.tex.empty_author", "author command has no readable name", SemanticStatus::Partial);
                }
                else {
                    self.state.authors.push(text);
                }
                InlineOutcome::Dropped
            }
            "date" => {
                self.loss(
                    "import.tex.metadata_date",
                    "`\\date` content is dropped because `DocumentMetadata` has no date field",
                    SemanticStatus::Partial,
                );
                InlineOutcome::Dropped
            }
            "maketitle" => {
                self.loss(
                    "import.tex.maketitle_dropped",
                    "`\\maketitle` is dropped because title and author are carried as document metadata",
                    SemanticStatus::Partial,
                );
                InlineOutcome::Dropped
            }
            "footnote" => {
                let body = self
                    .required_argument(command)
                    .map(|fragment| self.lower_inline_fragment(fragment))
                    .unwrap_or_default();
                self.state.footnote += 1;
                let label = self.state.footnote.to_string();
                self.loss(
                    "import.tex.footnote_body",
                    format!("footnote {label} body is carried on the reference inline because the IR has no footnote collection block"),
                    SemanticStatus::Partial,
                );
                InlineOutcome::Inline(Inline::Note { label, display: Some(body) })
            }
            "cite" => {
                let authored = self.required_argument(command).unwrap_or_default().trim().to_string();
                let keys: Vec<String> = authored.split(',').map(|key| key.trim().to_string()).filter(|key| !key.is_empty()).collect();
                if keys.is_empty() {
                    self.loss("import.tex.empty_citation", "citation command has no key", SemanticStatus::Partial);
                    return InlineOutcome::Dropped;
                }
                let mut inlines = Vec::new();
                for key in keys {
                    if self.state.cited.insert(key.clone()) {
                        let id = self.state.next_link_id();
                        self.graph.push_relation(Relation {
                            id,
                            kind: RelationKind::Cites,
                            source: RelationEndpoint::Document(self.graph.id),
                            target: RelationEndpoint::Unresolved { label: key.clone() },
                        });
                    }
                    inlines.push(Inline::Citation { key, display: None });
                }
                InlineOutcome::Inlines(inlines)
            }
            "includegraphics" => {
                let source = self.required_argument(command).unwrap_or_default().trim().to_string();
                if source.is_empty() {
                    self.loss("import.tex.empty_image_source", "image command has no target", SemanticStatus::Partial);
                    return InlineOutcome::Dropped;
                }
                InlineOutcome::Inline(Inline::Image { source, alt: Vec::new(), title: None, asset: None })
            }
            "ref" | "eqref" => {
                let label = self.required_argument(command).unwrap_or_default().trim().to_string();
                let eqref = name == "eqref";
                let resolved = self.state.labels.get(&label).cloned();
                let target = match resolved.as_ref().and_then(|info| info.node) {
                    Some(node) => RelationEndpoint::Node(node),
                    None => RelationEndpoint::Unresolved { label: label.clone() },
                };
                let text = match resolved.as_ref().and_then(|info| info.number.clone()) {
                    Some(number) if eqref => format!("({number})"),
                    Some(number) => number,
                    None if eqref => format!("({label})"),
                    None => label.clone(),
                };
                InlineOutcome::Inline(Inline::Attributed {
                    attributes: Attributes {
                        id: None,
                        classes: vec!["tex-ref".into()],
                        pairs: vec![
                            AttributePair { key: "data-tex-ref-kind".into(), value: if eqref { "eqref".into() } else { "ref".into() } },
                            AttributePair { key: "data-tex-ref-label".into(), value: label },
                        ],
                    },
                    children: vec![Inline::Link { target, display: vec![Inline::Text { text }], title: None }],
                })
            }
            "label" => {
                let label = self.required_argument(command).unwrap_or_default().trim().to_string();
                if label.is_empty() {
                    self.loss("import.tex.label_anchor", "label command has no readable name", SemanticStatus::Partial);
                    return InlineOutcome::Dropped;
                }
                match self.state.last_block {
                    Some(node) => {
                        let number = self.state.last_number.clone();
                        self.state.labels.insert(label.clone(), LabelInfo { node: Some(node), number });
                        let id = self.state.next_link_id();
                        self.graph.push_relation(Relation {
                            id,
                            kind: RelationKind::References,
                            source: RelationEndpoint::Unresolved { label },
                            target: RelationEndpoint::Node(node),
                        });
                    }
                    None => {
                        self.loss(
                            "import.tex.label_anchor",
                            format!("label anchor has no preceding block to bind: {label}"),
                            SemanticStatus::Partial,
                        );
                    }
                }
                InlineOutcome::Dropped
            }
            // Text-mode spacing commands carry a visible typographic hint the IR
            // cannot express as a first-class node, so each one stays a `tex-space`
            // marker whose value keeps the authored distinction for the layout
            // host (thin `3mu`, medium `4mu`, thick `5mu`, negative `-3mu`, quad
            // `1em`, double-quad `2em`) plus one loss marker. Inside math the same
            // commands survive verbatim in the preserved math source instead.
            "," | ":" | ";" | "!" | "quad" | "qquad" => {
                let value = match name.as_str() {
                    "," => "thin",
                    ":" => "medium",
                    ";" => "thick",
                    "!" => "negative",
                    "quad" => "quad",
                    _ => "double-quad",
                };
                self.loss(
                    "import.tex.space_marker",
                    format!("`\\{name}` is kept as a spacing marker because the IR has no typographic space node"),
                    SemanticStatus::Partial,
                );
                InlineOutcome::Inline(Inline::Attributed {
                    attributes: Attributes {
                        id: None,
                        classes: vec!["tex-space".into()],
                        pairs: vec![AttributePair { key: "data-tex-space".into(), value: value.into() }],
                    },
                    children: Vec::new(),
                })
            }
            // Math style/font/limits commands belong inside math mode; lowering
            // does not interpret them as inline semantics, and when they do
            // appear outside a math span (parser edge case or stray token)
            // dropping them silently is correct because the math layout stage
            // (VuTeX) never sees these stray non-math tokens.
            "displaystyle" | "textstyle" | "scriptstyle" | "scriptscriptstyle" | "mathbf" | "mathit"
            | "mathrm" | "mathcal" | "mathbb" | "mathfrak" | "limits" | "nolimits" => InlineOutcome::Dropped,
            _ => InlineOutcome::Unknown,
        }
    }

    fn lower_verb(&mut self, command: &TexCommand, sink: &mut Sink) {
        let start = command.span.end;
        let Some((content, _)) = self.verb_content(start) else {
            self.loss("import.tex.verb_unterminated", "verb command has no closing delimiter", SemanticStatus::Partial);
            self.push_block(
                sink,
                Block::Opaque {
                    kind: "tex.verb".into(),
                    payload_hint: self.raw(command.span.start, command.span.end).trim().to_string(),
                    status: SemanticStatus::Partial,
                },
            );
            return;
        };
        self.push_block(sink, Block::Code { language: None, content });
    }

    /// Number of bytes occupied by a `\verb<delimiter>...<delimiter>` body.
    fn verb_consumed_len(&self, start: usize) -> usize {
        match self.verb_content(start) {
            Some((_, end)) => end.saturating_sub(start),
            None => 0,
        }
    }

    fn verb_content(&self, start: usize) -> Option<(String, usize)> {
        let rest = self.source.get(start..)?;
        let delimiter = rest.chars().next()?;
        if delimiter.is_alphanumeric() || delimiter.is_whitespace() {
            return None;
        }
        let body_start = start + delimiter.len_utf8();
        let body = self.source.get(body_start..)?;
        let end = body.find(delimiter)?;
        Some((body[..end].to_string(), body_start + end + delimiter.len_utf8()))
    }

    fn required_argument(&self, command: &TexCommand) -> Option<&'a str> {
        command.arguments.iter().find_map(|argument| match argument {
            TexArgument::Required(root) => Some(self.argument_fragment(root)),
            TexArgument::Optional(_) => None,
        })
    }

    /// Flattened plain text of the first required argument.
    fn argument_text(&mut self, command: &TexCommand) -> String {
        self.required_argument(command)
            .map(|fragment| self.lower_inline_fragment(fragment))
            .unwrap_or_default()
            .iter()
            .map(Inline::plain_text)
            .collect::<String>()
            .trim()
            .to_string()
    }

    fn argument_fragment(&self, root: &TexRoot) -> &'a str {
        let raw = self.raw(root.span.start, root.span.end).trim();
        let raw = raw.strip_prefix('{').or_else(|| raw.strip_prefix('[')).unwrap_or(raw);
        raw.strip_suffix('}').or_else(|| raw.strip_suffix(']')).unwrap_or(raw)
    }

    fn code_environment_content(&self, start: usize, end: usize) -> String {
        let raw = self.raw(start, end);
        let Some(header_end) = begin_header_end(raw) else {
            return raw.to_string();
        };
        let Some(footer_start) = raw.rfind("\\end{") else {
            return raw[header_end.min(raw.len())..].trim_matches('\n').to_string();
        };
        if footer_start <= header_end {
            return String::new();
        }
        raw[header_end..footer_start].trim_matches('\n').to_string()
    }

    fn lower_list(&mut self, name: &str, start: usize, end: usize, sink: &mut Sink) {
        let raw = self.raw(start, end);
        let Some(header_end) = begin_header_end(raw) else {
            self.loss("import.tex.malformed_list", "list environment header is not readable", SemanticStatus::Partial);
            return;
        };
        let Some(footer_start) = raw.rfind("\\end{") else {
            self.loss("import.tex.malformed_list", "list environment has no closing marker", SemanticStatus::Partial);
            return;
        };
        if footer_start <= header_end {
            self.loss("import.tex.empty_list", "list environment has no body", SemanticStatus::Partial);
            return;
        }
        let interior = &raw[header_end..footer_start];
        let slices = split_list_items(interior);
        if slices.is_empty() {
            self.loss("import.tex.list_without_items", "list environment contains no item marker", SemanticStatus::Partial);
            return;
        }
        let ordered = name == "enumerate";
        let mut items = Vec::new();
        for (index, slice) in slices.into_iter().enumerate() {
            if slice.label.is_some() {
                self.loss(
                    "import.tex.description_label",
                    "custom list item marker is not represented in the list item content",
                    SemanticStatus::Partial,
                );
            }
            let fragment = interior.get(slice.start..slice.end).unwrap_or("").trim();
            let mut item_sink = Sink::item();
            self.lower_fragment_into(fragment, &mut item_sink);
            item_sink.flush(&mut self.graph);
            items.push(ListItem {
                content: item_sink.item_content,
                children: item_sink.children,
                number: ordered.then_some((index + 1) as u32),
                is_task: false,
                is_checked: None,
            });
        }
        self.push_block(sink, Block::List { ordered, items });
    }

    fn lower_table(&mut self, start: usize, end: usize, sink: &mut Sink) {
        let raw = self.raw(start, end);
        let Some(header_end) = begin_header_end(raw) else {
            self.loss("import.tex.malformed_table", "tabular environment header is not readable", SemanticStatus::Partial);
            return;
        };
        let Some(footer_start) = raw.rfind("\\end{") else {
            self.loss("import.tex.malformed_table", "tabular environment has no closing marker", SemanticStatus::Partial);
            return;
        };
        if footer_start <= header_end {
            self.loss("import.tex.empty_table", "tabular environment has no body", SemanticStatus::Partial);
            return;
        }
        let body_start = column_spec_end(raw, header_end).unwrap_or(header_end);
        if body_start >= footer_start {
            self.loss("import.tex.empty_table", "tabular environment has no rows", SemanticStatus::Partial);
            return;
        }
        let interior = &raw[body_start..footer_start];
        if interior.contains("\\multicolumn") || interior.contains("\\multirow") {
            self.loss(
                "import.tex.table_spans",
                "multi-column or multi-row table cell spans are not represented by the IR table cells",
                SemanticStatus::Partial,
            );
        }
        let mut rows = Vec::new();
        for row in split_table_rows(interior) {
            let cells = split_table_cells(row)
                .into_iter()
                .map(|cell| TableCell { content: self.lower_inline_fragment(cell), colspan: 1, rowspan: 1 })
                .collect();
            rows.push(TableRow { is_header: false, cells });
        }
        if rows.is_empty() {
            self.loss("import.tex.empty_table", "tabular environment produced no table rows", SemanticStatus::Partial);
        }
        self.push_block(sink, Block::Table { caption: None, rows });
    }

    /// Emit the authored whitespace that sits between two lowered items.
    ///
    /// `oak-tex` ends every text token at whitespace, so the space between two
    /// words belongs to no item span. A blank line inside such a gap ends the
    /// current paragraph; any other whitespace run collapses to a single
    /// space, the collapsing TeX itself applies to its input. Whitespace the
    /// terminating comment already consumed is gobbled first. Returns whether
    /// the gap was pure whitespace, so the caller can fall back to recovering
    /// dropped environments for content gaps.
    fn lower_gap(&mut self, start: usize, end: usize, sink: &mut Sink, after_comment: bool) -> bool {
        if start >= end {
            return false;
        }
        let gap = &self.source[start..end];
        if !gap.chars().all(char::is_whitespace) {
            return false;
        }
        let gap = gobble_after_comment(gap, after_comment);
        if gap.is_empty() {
            return true;
        }
        if split_paragraphs(gap).len() > 1 {
            sink.flush(&mut self.graph);
        }
        else {
            push_text_inlines(&mut sink.inlines, " ");
        }
        true
    }

    /// Emit the single space a whitespace-only gap carries in inline content.
    ///
    /// Inline text has no paragraph blocks, so every whitespace run between
    /// items collapses to one space regardless of newlines. Math mode ignores
    /// authored whitespace entirely, matching TeX.
    fn push_gap_space(&mut self, start: usize, end: usize, inlines: &mut Vec<Inline>, after_comment: bool, math: bool) -> bool {
        if start >= end {
            return false;
        }
        let gap = &self.source[start..end];
        if !gap.chars().all(char::is_whitespace) {
            return false;
        }
        if math {
            return true;
        }
        let gap = gobble_after_comment(gap, after_comment);
        if !gap.is_empty() {
            push_text_inlines(inlines, " ");
        }
        true
    }

    fn lower_dropped_regions(&mut self, start: usize, end: usize, sink: &mut Sink) {
        if start >= end {
            return;
        }
        let mut cursor = start;
        while cursor < end {
            let Some(offset) = self.source.get(cursor..end).and_then(|rest| rest.find("\\begin{")) else {
                return;
            };
            let begin = cursor + offset;
            let Some((name, _)) = parse_begin(&self.source[begin..end]) else {
                cursor = begin + 7;
                continue;
            };
            if !matches!(name.as_str(), "itemize" | "enumerate" | "description" | "tabular" | "array") {
                cursor = begin + 7;
                continue;
            }
            let Some(environment_end) = matching_environment_end(self.source, begin, end) else {
                self.loss(
                    "import.tex.unclosed_environment",
                    format!("environment {name} has no closing marker"),
                    SemanticStatus::Unsupported,
                );
                return;
            };
            match name.as_str() {
                "itemize" | "enumerate" | "description" => self.lower_list(&name, begin, environment_end, sink),
                _ => self.lower_table(begin, environment_end, sink),
            }
            cursor = environment_end;
        }
    }

    fn lower_fragment_into(&mut self, fragment: &str, sink: &mut Sink) {
        let Ok(root) = parse_tex("<fragment>", fragment) else {
            self.loss("import.tex.fragment_parse_failed", "nested TeX fragment could not be parsed", SemanticStatus::Partial);
            return;
        };
        let mut inner = TexLowerer {
            graph: std::mem::replace(&mut self.graph, DocumentGraph::new(DocumentId(0))),
            source: fragment,
            state: std::mem::take(&mut self.state),
        };
        inner.lower_root(&root.items, 0, fragment.len(), sink);
        self.graph = inner.graph;
        self.state = inner.state;
    }

    fn lower_inline_fragment(&mut self, fragment: &str) -> Vec<Inline> {
        let Ok(root) = parse_tex("<inline>", fragment) else {
            self.loss("import.tex.fragment_parse_failed", "inline TeX fragment could not be parsed", SemanticStatus::Partial);
            return Vec::new();
        };
        let mut inner = TexLowerer {
            graph: std::mem::replace(&mut self.graph, DocumentGraph::new(DocumentId(0))),
            source: fragment,
            state: std::mem::take(&mut self.state),
        };
        let mut inlines = inner.collect_inlines(&root.items, 0, fragment.len(), false);
        trim_inlines(&mut inlines);
        self.graph = inner.graph;
        self.state = inner.state;
        inlines
    }

    fn collect_inlines(&mut self, items: &[TexItem], region_start: usize, region_end: usize, math: bool) -> Vec<Inline> {
        let mut inlines = Vec::new();
        let mut cursor = region_start;
        let mut skip_until = 0usize;
        let mut after_comment = false;
        for item in items {
            let Some((start, end)) = item_span(item) else {
                continue;
            };
            if start < skip_until {
                continue;
            }
            if !self.push_gap_space(cursor, start, &mut inlines, after_comment, math) {
                self.record_dropped_regions_loss(cursor, start);
            }
            after_comment = matches!(item, TexItem::Comment { .. });
            match item {
                TexItem::Text { content, .. } => push_text_inlines(&mut inlines, content),
                // Comments carry no document semantics, so dropping them is intended.
                TexItem::Comment { .. } => {}
                TexItem::Group(group) => {
                    let nested = self.collect_inlines(&group.content.items, group.span.start, group.span.end, math);
                    inlines.extend(nested);
                }
                TexItem::Superscript(superscript) => {
                    let children = self.collect_inlines(&superscript.content.items, superscript.span.start, superscript.span.end, true);
                    inlines.push(Inline::Styled { style: "superscript".into(), children });
                }
                TexItem::Subscript(subscript) => {
                    let children = self.collect_inlines(&subscript.content.items, subscript.span.start, subscript.span.end, true);
                    inlines.push(Inline::Styled { style: "subscript".into(), children });
                }
                TexItem::Math(math) => {
                    let (content, _) = self.math_source(math);
                    inlines.push(Inline::InlineMath { content, language: Some("latex".into()) });
                }
                TexItem::Command(command) => {
                    let raw = self.raw(command.span.start, command.span.end);
                    if command_name(raw) == "verb" {
                        if let Some((content, end)) = self.verb_content(command.span.end) {
                            skip_until = end;
                            inlines.push(Inline::InlineCode { text: content });
                        }
                        else {
                            self.loss("import.tex.verb_unterminated", "verb command has no closing delimiter", SemanticStatus::Partial);
                        }
                        continue;
                    }
                    match self.inline_from_command(command) {
                        InlineOutcome::Inline(inline) => inlines.push(inline),
                        InlineOutcome::Inlines(list) => inlines.extend(list),
                        InlineOutcome::Dropped => {}
                        InlineOutcome::Unknown => self.loss(
                            "import.tex.unsupported_command",
                            format!("inline command has no semantic mapping: {}", raw.trim()),
                            SemanticStatus::Unsupported,
                        ),
                    }
                }
                TexItem::Environment(environment) => {
                    let raw = self.raw(environment.span.start, environment.span.end);
                    let name = environment.name.clone();
                    if matches!(environment.name.as_str(), "itemize" | "enumerate" | "description" | "tabular" | "array") {
                        self.loss(
                            "import.tex.block_in_inline_context",
                            format!("block environment {name} cannot be represented as inline content"),
                            SemanticStatus::Partial,
                        );
                    }
                    else if matches!(environment.name.as_str(), "verbatim" | "lstlisting" | "minted" | "Verbatim") {
                        let text = self.code_environment_content(environment.span.start, environment.span.end);
                        inlines.push(Inline::InlineCode { text });
                    }
                    else if environment.is_math {
                        self.check_environment_boundaries(environment);
                        inlines.push(Inline::InlineMath { content: raw.trim().to_string(), language: Some("latex".into()) });
                    }
                    else {
                        self.loss(
                            "import.tex.unsupported_command",
                            format!("environment has no inline semantic mapping: {name}"),
                            SemanticStatus::Unsupported,
                        );
                    }
                }
                TexItem::LeftRightPair(pair) => {
                    let inline = self.left_right_inline(pair);
                    inlines.push(inline);
                }
                TexItem::Limits(limits) => {
                    let inline = self.limits_inline(limits);
                    inlines.push(inline);
                }
            }
            cursor = end;
        }
        self.record_dropped_regions_loss(cursor, region_end);
        inlines
    }

    fn record_dropped_regions_loss(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let mut cursor = start;
        while cursor < end {
            let Some(offset) = self.source.get(cursor..end).and_then(|rest| rest.find("\\begin{")) else {
                return;
            };
            let begin = cursor + offset;
            let Some((name, _)) = parse_begin(&self.source[begin..end]) else {
                cursor = begin + 7;
                continue;
            };
            if matches!(name.as_str(), "itemize" | "enumerate" | "description" | "tabular" | "array") {
                self.loss(
                    "import.tex.block_in_inline_context",
                    format!("block environment {name} cannot be represented as inline content"),
                    SemanticStatus::Partial,
                );
            }
            let Some(environment_end) = matching_environment_end(self.source, begin, end) else {
                return;
            };
            cursor = environment_end;
        }
    }

    fn push_text(&mut self, text: &str, sink: &mut Sink) {
        for (index, chunk) in split_paragraphs(text).into_iter().enumerate() {
            if index > 0 {
                sink.flush(&mut self.graph);
            }
            push_text_inlines(&mut sink.inlines, chunk);
        }
    }
}

/// Source span of a `TexItem`.
///
/// The match is exhaustive over the closed `oak-tex` AST enum, so a new variant
/// fails to compile here instead of losing its source span silently.
fn item_span(item: &TexItem) -> Option<(usize, usize)> {
    match item {
        TexItem::Command(command) => Some((command.span.start, command.span.end)),
        TexItem::Environment(environment) => Some((environment.span.start, environment.span.end)),
        TexItem::Group(group) => Some((group.span.start, group.span.end)),
        TexItem::Math(math) => Some((math.span.start, math.span.end)),
        TexItem::Superscript(superscript) => Some((superscript.span.start, superscript.span.end)),
        TexItem::Subscript(subscript) => Some((subscript.span.start, subscript.span.end)),
        TexItem::LeftRightPair(pair) => Some((pair.span.start, pair.span.end)),
        TexItem::Limits(limits) => Some((limits.span.start, limits.span.end)),
        TexItem::Text { span, .. } | TexItem::Comment { span, .. } => Some((span.start, span.end)),
    }
}

fn command_name(raw: &str) -> String {
    let name = raw.trim_start().strip_prefix('\\').unwrap_or(raw.trim_start());
    let Some(first) = name.chars().next() else {
        return String::new();
    };
    if first.is_ascii_alphabetic() {
        name.chars().take_while(|character| character.is_ascii_alphabetic()).collect()
    }
    else {
        first.to_string()
    }
}

/// Parse a `\begin{name}` header at the start of `raw`.
fn parse_begin(raw: &str) -> Option<(String, usize)> {
    let rest = raw.strip_prefix("\\begin{")?;
    let closing = rest.find('}')?;
    let name = rest[..closing].trim().to_string();
    Some((name, "\\begin{".len() + closing + 1))
}

fn begin_header_end(raw: &str) -> Option<usize> {
    parse_begin(raw.trim_start()).map(|(_, end)| end + (raw.len() - raw.trim_start().len()))
}

fn section_level(name: &str) -> Option<u8> {
    match name {
        "part" | "chapter" | "section" => Some(1),
        "subsection" => Some(2),
        "subsubsection" => Some(3),
        _ => None,
    }
}

fn is_macro_definition(name: &str) -> bool {
    matches!(
        name,
        "newcommand"
            | "renewcommand"
            | "providecommand"
            | "declarerobustcommand"
            | "def"
            | "edef"
            | "gdef"
            | "xdef"
            | "let"
            | "newenvironment"
            | "renewenvironment"
    )
}

/// Result of mapping a TeX command to an inline semantic.
enum InlineOutcome {
    Inline(Inline),
    /// The command was understood and produces more than one inline node.
    Inlines(Vec<Inline>),
    /// The command was understood and intentionally produces no inline node.
    Dropped,
    /// The command has no inline semantic mapping.
    Unknown,
}

/// Authored opening and closing markers of one math delimiter style.
fn math_delimiters(delimiter: MathDelimiter) -> (&'static str, &'static str) {
    match delimiter {
        MathDelimiter::Dollar => ("$", "$"),
        MathDelimiter::DoubleDollar => ("$$", "$$"),
        MathDelimiter::Paren => ("\\(", "\\)"),
        MathDelimiter::Bracket => ("\\[", "\\]"),
        MathDelimiter::Environment => ("", ""),
    }
}

/// Split the authored math source into its body.
///
/// Every delimiter form strips its own opening and closing marker, so `$x$` and
/// `\(x\)` produce the same inline payload and `$$x$$` and `\[x\]` produce the
/// same display payload. A missing closing marker yields `None`, so the caller
/// keeps the authored body and records the defect instead of dropping the span.
fn math_source_strip<'a>(raw: &'a str, delimiter: MathDelimiter) -> Option<&'a str> {
    let trimmed = raw.trim();
    let (open, close) = math_delimiters(delimiter);
    if open.is_empty() {
        return Some(trimmed);
    }
    let body = trimmed.strip_prefix(open)?;
    body.strip_suffix(close)
}

/// Whether a math environment emits an equation number in LaTeX.
///
/// Starred variants and the inner environments (`aligned`, `gathered`, `split`,
/// `cases`, matrix family, `math`, `displaymath`) never number their content.
fn math_environment_is_numbered(name: &str, starred: bool) -> bool {
    !starred && matches!(name, "equation" | "align" | "gather" | "multline" | "eqnarray" | "flalign" | "alignat")
}

/// Row segment of a math environment body at the environment's own depth.
struct MathRow {
    start: usize,
    end: usize,
    labels: Vec<String>,
    notag: bool,
}

/// Scan the authored environment source for row breaks, labels, and a `\tag`
/// override, tracking `\begin`/`\end` depth so nested environment bodies do
/// not contribute rows or labels to the outer environment.
fn scan_math_body(raw: &str) -> (Vec<MathRow>, Option<String>) {
    let bytes = raw.as_bytes();
    let mut rows: Vec<MathRow> = Vec::new();
    let mut tag: Option<String> = None;
    let mut depth = 0usize;
    let mut row_start = 0usize;
    let mut row_labels: Vec<String> = Vec::new();
    let mut row_notag = false;
    let mut terminated = false;
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'\\' {
            index += 1;
            continue;
        }
        let rest = &raw[index..];
        if let Some(after) = rest.strip_prefix("\\begin{") {
            depth += 1;
            index += "\\begin{".len() + after.find('}').map_or(after.len(), |closing| closing + 1);
            continue;
        }
        if let Some(after) = rest.strip_prefix("\\end{") {
            if depth == 1 {
                rows.push(MathRow { start: row_start, end: index, labels: std::mem::take(&mut row_labels), notag: row_notag });
                row_notag = false;
                terminated = true;
            }
            depth = depth.saturating_sub(1);
            index += "\\end{".len() + after.find('}').map_or(after.len(), |closing| closing + 1);
            continue;
        }
        if rest.starts_with("\\\\") {
            if depth == 1 {
                rows.push(MathRow { start: row_start, end: index, labels: std::mem::take(&mut row_labels), notag: row_notag });
                row_notag = false;
                row_start = index + 2;
            }
            index += 2;
            continue;
        }
        if depth == 1 {
            if let Some(after) = rest.strip_prefix("\\label{") {
                if let Some(closing) = after.find('}') {
                    row_labels.push(after[..closing].trim().to_string());
                    index += "\\label{".len() + closing + 1;
                    continue;
                }
            }
            if let Some(after) = rest.strip_prefix("\\tag") {
                let mut consumed = "\\tag".len();
                let mut tail = after;
                if let Some(starred) = tail.strip_prefix('*') {
                    consumed += 1;
                    tail = starred;
                }
                let spaces = tail.len() - tail.trim_start().len();
                consumed += spaces;
                tail = tail.trim_start();
                if let Some(inner) = tail.strip_prefix('{') {
                    if let Some(closing) = inner.find('}') {
                        tag = Some(inner[..closing].trim().to_string());
                        index += consumed + closing + 1;
                        continue;
                    }
                }
            }
            if rest.starts_with("\\notag") || rest.starts_with("\\nonumber") {
                row_notag = true;
                index += if rest.starts_with("\\nonumber") { "\\nonumber".len() } else { "\\notag".len() };
                continue;
            }
        }
        index += 1;
    }
    if !terminated {
        rows.push(MathRow { start: row_start, end: bytes.len(), labels: row_labels, notag: row_notag });
    }
    (rows, tag)
}

/// Rewrite forward references once every label anchor is known and return the
/// labels that stayed unresolved.
fn resolve_pending_references(graph: &mut DocumentGraph, labels: &HashMap<String, LabelInfo>) -> Vec<String> {
    let mut unresolved = Vec::new();
    for node in &mut graph.blocks {
        match &mut node.block {
            Block::Section { title, .. } => fix_pending_refs(title, labels, &mut unresolved),
            Block::Paragraph { content } | Block::Quote { content } => fix_pending_refs(content, labels, &mut unresolved),
            Block::List { items, .. } => {
                for item in items {
                    fix_pending_refs(&mut item.content, labels, &mut unresolved);
                }
            }
            Block::Table { rows, .. } => {
                for row in rows {
                    for cell in &mut row.cells {
                        fix_pending_refs(&mut cell.content, labels, &mut unresolved);
                    }
                }
            }
            _ => {}
        }
    }
    unresolved.sort();
    unresolved.dedup();
    unresolved
}

/// Resolve the `tex-ref` markers of one inline tree in place.
fn fix_pending_refs(inlines: &mut Vec<Inline>, labels: &HashMap<String, LabelInfo>, unresolved: &mut Vec<String>) {
    for inline in inlines.iter_mut() {
        match inline {
            Inline::Attributed { attributes, children } if attributes.classes.iter().any(|class| class == "tex-ref") => {
                let Some(label) = attributes.pairs.iter().find(|pair| pair.key == "data-tex-ref-label").map(|pair| pair.value.clone()) else {
                    continue;
                };
                let Some(Inline::Link { target, display, .. }) = children.first_mut() else {
                    continue;
                };
                if !matches!(target, RelationEndpoint::Unresolved { .. }) {
                    continue;
                }
                match labels.get(&label) {
                    Some(info) => {
                        if let Some(node) = info.node {
                            *target = RelationEndpoint::Node(node);
                        }
                        if let Some(number) = &info.number {
                            let eqref = attributes.pairs.iter().any(|pair| pair.key == "data-tex-ref-kind" && pair.value == "eqref");
                            *display = vec![Inline::Text { text: if eqref { format!("({number})") } else { number.clone() } }];
                        }
                    }
                    None => unresolved.push(label),
                }
            }
            Inline::Attributed { children, .. } | Inline::Styled { children, .. } => fix_pending_refs(children, labels, unresolved),
            Inline::Link { display, .. } => fix_pending_refs(display, labels, unresolved),
            Inline::Image { alt, .. } => fix_pending_refs(alt, labels, unresolved),
            Inline::Note { display, .. } | Inline::Citation { display, .. } => {
                if let Some(display) = display {
                    fix_pending_refs(display, labels, unresolved);
                }
            }
            _ => {}
        }
    }
}

/// Drop the whitespace a TeX comment consumes: the newline that terminates
/// the comment and the spaces or tabs opening the following line.
fn gobble_after_comment(gap: &str, after_comment: bool) -> &str {
    if !after_comment {
        return gap;
    }
    let rest = gap.trim_start_matches([' ', '\t', '\r']);
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    rest.trim_start_matches([' ', '\t', '\r'])
}

fn split_paragraphs(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'\n' {
            let mut next = index + 1;
            while next < bytes.len() && matches!(bytes[next], b' ' | b'\t' | b'\r') {
                next += 1;
            }
            if next < bytes.len() && bytes[next] == b'\n' {
                chunks.push(&text[start..index]);
                index = next + 1;
                start = index;
                continue;
            }
        }
        index += 1;
    }
    chunks.push(&text[start..]);
    chunks
}

fn push_text_inlines(inlines: &mut Vec<Inline>, text: &str) {
    let normalized = normalize_text(text);
    if normalized.is_empty() {
        return;
    }
    if inlines.is_empty() {
        let trimmed = normalized.trim_start();
        if !trimmed.is_empty() {
            push_inline_text(inlines, trimmed);
        }
        return;
    }
    if last_inline_ends_with_space(inlines) && normalized.starts_with(' ') {
        push_inline_text(inlines, &normalized[1..]);
        return;
    }
    push_inline_text(inlines, &normalized);
}

fn normalize_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut pending_space = false;
    for character in text.chars() {
        if character.is_whitespace() {
            pending_space = !normalized.is_empty();
            continue;
        }
        if pending_space {
            normalized.push(' ');
            pending_space = false;
        }
        normalized.push(character);
    }
    if pending_space {
        normalized.push(' ');
    }
    if normalized.is_empty() && !text.is_empty() {
        return " ".into();
    }
    normalized
}

fn push_inline_text(inlines: &mut Vec<Inline>, text: &str) {
    if let Some(Inline::Text { text: previous }) = inlines.last_mut() {
        previous.push_str(text);
        return;
    }
    inlines.push(Inline::Text { text: text.into() });
}

fn last_inline_ends_with_space(inlines: &[Inline]) -> bool {
    matches!(inlines.last(), Some(Inline::Text { text }) if text.ends_with(' '))
}

fn trim_inlines(inlines: &mut Vec<Inline>) {
    if let Some(Inline::Text { text }) = inlines.last_mut() {
        let trimmed = text.trim_end().len();
        text.truncate(trimmed);
    }
    if matches!(inlines.last(), Some(Inline::Text { text }) if text.is_empty()) {
        inlines.pop();
    }
    if let Some(Inline::Text { text }) = inlines.first_mut() {
        if text.starts_with(' ') {
            *text = text.trim_start().to_string();
        }
    }
    if matches!(inlines.first(), Some(Inline::Text { text }) if text.is_empty()) {
        inlines.remove(0);
    }
}

struct ListItemSlice {
    start: usize,
    end: usize,
    label: Option<String>,
}

/// Split a list environment body on top-level `\item` markers.
fn split_list_items(interior: &str) -> Vec<ListItemSlice> {
    let bytes = interior.as_bytes();
    let mut markers = Vec::new();
    let mut depth = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
                continue;
            }
            b'\\' => {
                if interior[index..].starts_with("\\begin{") {
                    depth += 1;
                    index += "\\begin{".len();
                    continue;
                }
                if interior[index..].starts_with("\\end{") {
                    depth = depth.saturating_sub(1);
                    index += "\\end{".len();
                    continue;
                }
                if interior[index..].starts_with("\\item") && !is_command_continuation(bytes.get(index + 5)) {
                    let after = index + "\\item".len();
                    if depth == 0 {
                        let (label, content_start) = optional_label(interior, after);
                        markers.push((index, content_start, label));
                    }
                    index = after;
                    continue;
                }
            }
            _ => {}
        }
        index += 1;
    }
    let mut slices = Vec::new();
    for (position, (marker_start, content_start, label)) in markers.iter().enumerate() {
        let end = markers.get(position + 1).map(|next| next.0).unwrap_or(interior.len());
        let end = end.max(*marker_start).max(*content_start).min(interior.len());
        slices.push(ListItemSlice { start: *content_start, end, label: label.clone() });
    }
    slices
}

fn is_command_continuation(byte: Option<&u8>) -> bool {
    byte.is_some_and(|value| value.is_ascii_alphabetic())
}

fn optional_label(interior: &str, start: usize) -> (Option<String>, usize) {
    let rest = &interior[start..];
    let skipped = rest.len() - rest.trim_start().len();
    let after_spaces = &rest[skipped..];
    if let Some(body) = after_spaces.strip_prefix('[') {
        if let Some(closing) = body.find(']') {
            let label = body[..closing].trim().to_string();
            return (Some(label), start + skipped + 1 + closing + 1);
        }
    }
    (None, start)
}

/// Skip the required argument after `\begin{tabular}` (the column spec).
fn column_spec_end(raw: &str, header_end: usize) -> Option<usize> {
    let mut index = header_end;
    let bytes = raw.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    if index < bytes.len() && bytes[index] == b'[' {
        let mut depth = 0usize;
        while index < bytes.len() {
            match bytes[index] {
                b'[' => depth += 1,
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        index += 1;
                        break;
                    }
                }
                _ => {}
            }
            index += 1;
        }
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
    }
    if index >= bytes.len() || bytes[index] != b'{' {
        return Some(index);
    }
    let mut depth = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            b'\\' => {
                index += 1;
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn split_table_rows(interior: &str) -> Vec<&str> {
    let mut rows = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    let mut index = 0usize;
    let bytes = interior.as_bytes();
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
                continue;
            }
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b'\\' => {
                if interior[index..].starts_with("\\begin{") {
                    depth += 1;
                    index += "\\begin{".len();
                    continue;
                }
                if interior[index..].starts_with("\\end{") {
                    depth = depth.saturating_sub(1);
                    index += "\\end{".len();
                    continue;
                }
                if depth == 0 && interior[index..].starts_with("\\\\") {
                    rows.push(&interior[start..index]);
                    index += 2;
                    if index < bytes.len() && bytes[index] == b'[' {
                        while index < bytes.len() && bytes[index] != b']' {
                            index += 1;
                        }
                        index = (index + 1).min(bytes.len());
                    }
                    start = index;
                    continue;
                }
                index += 1;
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    rows.push(&interior[start..]);
    rows.into_iter().map(strip_table_decorations).filter(|row| !row.trim().is_empty()).collect()
}

fn strip_table_decorations(row: &str) -> &str {
    let mut row = row.trim();
    loop {
        let trimmed = strip_leading_decoration(row);
        if trimmed.len() == row.len() {
            break;
        }
        row = trimmed.trim_start();
    }
    row = row.trim_end();
    loop {
        let trimmed = strip_trailing_decoration(row);
        if trimmed.len() == row.len() {
            break;
        }
        row = trimmed.trim_end();
    }
    row
}

fn strip_leading_decoration(row: &str) -> &str {
    for decoration in ["\\hline", "\\toprule", "\\midrule", "\\bottomrule", "\\hline"] {
        if let Some(rest) = row.strip_prefix(decoration) {
            return rest;
        }
    }
    if let Some(rest) = row.strip_prefix("\\cline") {
        if let Some(closing) = rest.find('}') {
            return &rest[closing + 1..];
        }
    }
    row
}

fn strip_trailing_decoration(row: &str) -> &str {
    for decoration in ["\\hline", "\\toprule", "\\midrule", "\\bottomrule"] {
        if let Some(rest) = row.strip_suffix(decoration) {
            return rest;
        }
    }
    row
}

fn split_table_cells(row: &str) -> Vec<&str> {
    let mut cells = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    let mut index = 0usize;
    let bytes = row.as_bytes();
    while index < bytes.len() {
        match bytes[index] {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b'\\' => {
                if row[index..].starts_with("\\begin{") {
                    depth += 1;
                    index += "\\begin{".len();
                    continue;
                }
                if row[index..].starts_with("\\end{") {
                    depth = depth.saturating_sub(1);
                    index += "\\end{".len();
                    continue;
                }
                index += 1;
                continue;
            }
            b'&' if depth == 0 => {
                cells.push(&row[start..index]);
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    cells.push(&row[start..]);
    cells
}

/// Find the matching `\end{...}` for the environment beginning at `begin`.
fn matching_environment_end(source: &str, begin: usize, limit: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = begin;
    while index < limit {
        let rest = &source[index..limit];
        if rest.starts_with("\\begin{") {
            depth += 1;
            index += "\\begin{".len();
            continue;
        }
        if rest.starts_with("\\end{") {
            depth = depth.saturating_sub(1);
            let Some(closing) = source[index..limit].find('}') else {
                return None;
            };
            index += closing + 1;
            if depth == 0 {
                return Some(index);
            }
            continue;
        }
        index += 1;
    }
    None
}
