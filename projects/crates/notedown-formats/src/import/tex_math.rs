//! TeX math AST lowering into [`notedown_ir::MathFormula`].
//!
//! This module consumes Oak's TeX syntax tree. It never rescans the source for
//! alignment tabs, row breaks, labels or command arguments: every structural
//! fact comes from `TexItem`, `TexRow`, `TexCell`, `TexCommand` and
//! `TexLeftRightPair` nodes with their spans.

use notedown_ir::{
    MathAtom, MathClass, MathColumnAlignment, MathDelimiter, MathDelimiterKind, MathDisplay, MathEnvironment, MathEnvironmentKind,
    MathExpression, MathFormula, MathFractionLine, MathRow, MathRowNumbering, MathRowPlacement, MathSpace, MathSpan, MathStyle,
    MathStyleKind,
};
use oak_tex::ast::{
    MathDelimiter as TexMathDelimiter, TexArgument, TexCell, TexCommand, TexEnvironment, TexItem, TexLeftRightPair, TexMath, TexRoot, TexRow,
};

/// Lower one Oak math node into the document math semantics.
pub(crate) fn lower_tex_math(math: &TexMath, source: &str) -> MathFormula {
    let display = if math.is_display { MathDisplay::Display } else { MathDisplay::Inline };
    let base = trimmed_base(math.content.span.start, math.content.span.end, source);
    MathFormula {
        source: math_body_source(math, source),
        language: Some("latex".into()),
        display,
        expression: lower_root(&math.content.items, source, base),
    }
}

/// The authored formula body without its math delimiters.
fn math_body_source(math: &TexMath, source: &str) -> String {
    let raw = slice(source, math.content.span.start, math.content.span.end);
    let trimmed = raw.trim();
    let (open, close) = match math.delimiter {
        TexMathDelimiter::Dollar => ("$", "$"),
        TexMathDelimiter::DoubleDollar => ("$$", "$$"),
        TexMathDelimiter::Paren => ("\\(", "\\)"),
        TexMathDelimiter::Bracket => ("\\[", "\\]"),
        TexMathDelimiter::Environment => return trimmed.to_string(),
    };
    let body = trimmed.strip_prefix(open).unwrap_or(trimmed);
    body.strip_suffix(close).unwrap_or(body).to_string()
}

/// Lower a standalone TeX formula source into the document math semantics.
///
/// This is the public entry for hosts that own a formula string without a
/// surrounding document, such as WaTeX's `measure` and `render` APIs. Syntax
/// stays in Oak: the import layer parses the body and lowers its AST, and the
/// host consumes only [`MathFormula::expression`].
pub fn lower_tex_formula_source(source: &str, display: MathDisplay) -> MathFormula {
    lower_formula_source(source, display, "latex")
}

/// Lower a raw TeX formula source that did not arrive as an Oak math node.
///
/// Markdown's `tex_math` profile and standalone formula inputs carry only the
/// authored body, so the import layer composes the Oak TeX frontend here. The
/// parse stays a syntax step, and its AST is lowered like any other TeX math.
pub(crate) fn lower_formula_source(source: &str, display: MathDisplay, language: &str) -> MathFormula {
    use oak_core::{Builder, SourceText, parser::session::ParseSession};
    use oak_tex::{TexBuilder, TexLanguage};

    let config = TexLanguage::default();
    let builder = TexBuilder::new(&config);
    let text = SourceText::new(source);
    let mut session = ParseSession::<TexLanguage>::default();
    match builder.build(&text, &[], &mut session).result {
        Ok(root) => lower_tex_formula(&root, source, display, Some(language)),
        Err(_) => MathFormula::unsupported(source, Some(language.to_string()), display, "unparsed"),
    }
}

/// Lower a whole TeX root that already is a formula body.
pub(crate) fn lower_tex_formula(root: &TexRoot, source: &str, display: MathDisplay, language: Option<&str>) -> MathFormula {
    lower_root_with(root, source, display, language)
}

fn lower_root_with(root: &TexRoot, source: &str, display: MathDisplay, language: Option<&str>) -> MathFormula {
    let base = trimmed_base(root.span.start, root.span.end, source);
    MathFormula {
        source: slice(source, root.span.start, root.span.end).trim().to_string(),
        language: language.map(str::to_string),
        display,
        expression: lower_root(&root.items, source, base),
    }
}

/// Lower environment content, exposing the environment as a semantic node.
pub(crate) fn lower_tex_environment(environment: &TexEnvironment, source: &str, display: MathDisplay) -> MathFormula {
    let base = trimmed_base(environment.span.start, environment.span.end, source);
    let expression = lower_environment(environment, source, base);
    MathFormula {
        source: slice(source, environment.span.start, environment.span.end).trim().to_string(),
        language: Some("latex".into()),
        display,
        expression,
    }
}

/// Absolute offset the trimmed formula source starts at.
///
/// [`MathFormula::source`] is the CST slice trimmed of surrounding
/// whitespace, while CST spans stay absolute, so every span the lowering
/// records subtracts this base to index the formula source.
fn trimmed_base(start: usize, end: usize, source: &str) -> usize {
    let raw = slice(source, start, end);
    start + (raw.len() - raw.trim_start().len())
}

/// Byte span into the formula source for an absolute CST range.
fn span_from(base: usize, start: usize, end: usize) -> Option<MathSpan> {
    if end <= start {
        return None;
    }
    Some(MathSpan { start: start.saturating_sub(base) as u32, end: end.saturating_sub(base) as u32 })
}

/// Lower a list of TeX items into one expression.
///
/// `base` is the absolute source offset the formula source starts at after
/// trimming, so every recorded [`MathSpan`] indexes [`MathFormula::source`].
pub(crate) fn lower_root(items: &[TexItem], source: &str, base: usize) -> MathExpression {
    let mut expressions: Vec<MathExpression> = Vec::new();
    let mut index = 0usize;
    while index < items.len() {
        let item = &items[index];
        match item {
            TexItem::Superscript(script) => {
                if expressions.is_empty() {
                    expressions.push(MathExpression::Unsupported {
                        kind: "script-base".into(),
                        source: slice(source, script.span.start, script.span.end),
                        span: span_from(base, script.span.start, script.span.end),
                    });
                }
                else {
                    attach_script(&mut expressions, Some(lower_root(&script.content.items, source, base)), None);
                }
                index += 1;
            }
            TexItem::Subscript(script) => {
                if expressions.is_empty() {
                    expressions.push(MathExpression::Unsupported {
                        kind: "script-base".into(),
                        source: slice(source, script.span.start, script.span.end),
                        span: span_from(base, script.span.start, script.span.end),
                    });
                }
                else {
                    attach_script(&mut expressions, None, Some(lower_root(&script.content.items, source, base)));
                }
                index += 1;
            }
            TexItem::Limits(limits) => {
                // `\limits` describes the operator in front of it and the
                // scripts that follow it, so fold both onto the base that
                // precedes the policy.
                let base_expression = expressions.pop().unwrap_or_else(|| MathExpression::Unsupported {
                    kind: "limits-base".into(),
                    source: slice(source, limits.span.start, limits.span.end),
                    span: span_from(base, limits.span.start, limits.span.end),
                });
                // Consecutive `\limits`/`\nolimits` replace each other: the last
                // policy before the scripts is the one that applies.
                if matches!(expressions.last(), Some(MathExpression::LimitsPolicy { .. })) {
                    expressions.pop();
                }
                let mut superscript = None;
                let mut subscript = None;
                let mut next = index + 1;
                while next < items.len() {
                    match &items[next] {
                        TexItem::Superscript(script) => superscript = Some(lower_root(&script.content.items, source, base)),
                        TexItem::Subscript(script) => subscript = Some(lower_root(&script.content.items, source, base)),
                        _ => break,
                    }
                    next += 1;
                }
                expressions.push(MathExpression::LimitsPolicy { limits: limits.is_limits });
                expressions.push(script_expression(base_expression, superscript, subscript));
                index = next;
            }
            _ => {
                if let Some(expression) = lower_item(item, source, base) {
                    append_expression(&mut expressions, expression);
                }
                index += 1;
            }
        }
    }
    sequence(expressions)
}

/// Append an expression, flattening nested sequences into the enclosing list.
fn append_expression(expressions: &mut Vec<MathExpression>, expression: MathExpression) {
    match expression {
        MathExpression::Sequence(items) => {
            for item in items {
                append_expression(expressions, item);
            }
        }
        other => expressions.push(other),
    }
}

fn script_expression(base: MathExpression, superscript: Option<MathExpression>, subscript: Option<MathExpression>) -> MathExpression {
    if superscript.is_none() && subscript.is_none() {
        return base;
    }
    MathExpression::Script { base: Box::new(base), superscript: superscript.map(Box::new), subscript: subscript.map(Box::new) }
}

/// Commands that describe a row or a label, not an atom of the expression.
fn is_row_metadata(command: &TexCommand) -> bool {
    matches!(command.name.as_str(), "label" | "tag" | "notag" | "nonumber")
}

fn sequence(mut expressions: Vec<MathExpression>) -> MathExpression {
    if expressions.len() == 1 {
        return expressions.pop().expect("one expression");
    }
    MathExpression::Sequence(expressions)
}

/// Fold a script onto the expression it modifies.
fn attach_script(expressions: &mut Vec<MathExpression>, superscript: Option<MathExpression>, subscript: Option<MathExpression>) {
    let Some(last) = expressions.pop()
    else {
        expressions.push(MathExpression::Unsupported { kind: "script-base".into(), source: String::new(), span: None });
        return;
    };
    match last {
        MathExpression::Script { base, superscript: existing_sup, subscript: existing_sub } => {
            expressions.push(script_expression(
                *base,
                superscript.or(existing_sup.map(|value| *value)),
                subscript.or(existing_sub.map(|value| *value)),
            ));
        }
        base => expressions.push(script_expression(base, superscript, subscript)),
    }
}

fn lower_item(item: &TexItem, source: &str, base: usize) -> Option<MathExpression> {
    match item {
        TexItem::Text { content, span, .. } => Some(text_expression(content, span.start, base)),
        TexItem::Command(command) => Some(lower_command(command, source, base)),
        TexItem::Group(group) => Some(MathExpression::Group { body: Box::new(lower_root(&group.content.items, source, base)) }),
        TexItem::Math(math) => Some(lower_root(&math.content.items, source, base)),
        TexItem::Environment(environment) => Some(lower_environment(environment, source, base)),
        TexItem::LeftRightPair(pair) => Some(lower_left_right(pair, source, base)),
        TexItem::Row(_) => None,
        TexItem::Superscript(_) | TexItem::Subscript(_) | TexItem::Limits(_) => Some(MathExpression::Unsupported {
            kind: "math-item".into(),
            source: slice(source, item_span(item).0, item_span(item).1),
            span: span_from(base, item_span(item).0, item_span(item).1),
        }),
        TexItem::Comment { .. } => None,
    }
}

fn item_span(item: &TexItem) -> (usize, usize) {
    match item {
        TexItem::Command(node) => (node.span.start, node.span.end),
        TexItem::Environment(node) => (node.span.start, node.span.end),
        TexItem::Row(node) => (node.span.start, node.span.end),
        TexItem::Group(node) => (node.span.start, node.span.end),
        TexItem::Math(node) => (node.span.start, node.span.end),
        TexItem::LeftRightPair(node) => (node.span.start, node.span.end),
        TexItem::Limits(node) => (node.span.start, node.span.end),
        TexItem::Superscript(node) => (node.span.start, node.span.end),
        TexItem::Subscript(node) => (node.span.start, node.span.end),
        TexItem::Text { span, .. } | TexItem::Comment { span, .. } => (span.start, span.end),
    }
}

fn lower_command(command: &TexCommand, source: &str, base: usize) -> MathExpression {
    let name = command.name.as_str();
    let command_span = span_from(base, command.span.start, command.span.end);
    match name {
        "frac" | "dfrac" | "tfrac" => fraction(command, source, MathFractionLine::Solid, base),
        "binom" => MathExpression::Delimited {
            // The parentheses are synthesized by `\binom`; the author typed
            // no delimiter, so neither carries a source span.
            open: delimiter("(", None),
            close: delimiter(")", None),
            body: Box::new(fraction(command, source, MathFractionLine::None, base)),
            middles: Vec::new(),
        },
        "sqrt" => lower_radical(command, source, base),
        "text" | "textrm" | "mbox" | "operatorname" => match command.arguments.iter().find_map(argument_text) {
            Some(text) => MathExpression::Text { text, span: command_span },
            None => unsupported(command.span.start, command.span.end, source, "text", base),
        },
        "textit" => MathExpression::Styled {
            style: MathStyle::Italic,
            body: Box::new(MathExpression::Text {
                text: command.arguments.iter().find_map(argument_text).unwrap_or_default(),
                span: command_span,
            }),
        },
        "mathrm" => styled(MathStyle::Upright, command, source, base),
        "mathit" => styled(MathStyle::Italic, command, source, base),
        "mathbf" => styled(MathStyle::Bold, command, source, base),
        "mathbb" => styled(MathStyle::DoubleStruck, command, source, base),
        "mathcal" => styled(MathStyle::Script, command, source, base),
        "mathfrak" => styled(MathStyle::Fraktur, command, source, base),
        "mathsf" => styled(MathStyle::SansSerif, command, source, base),
        "mathtt" => styled(MathStyle::Monospace, command, source, base),
        "hat" | "widehat" => accent('ˆ', name == "widehat", command, source, base),
        "bar" | "overline" => accent('¯', false, command, source, base),
        "vec" | "overrightarrow" => accent('→', false, command, source, base),
        "tilde" | "widetilde" => accent('˜', name == "widetilde", command, source, base),
        "dot" => accent('˙', false, command, source, base),
        "ddot" => accent('¨', false, command, source, base),
        "breve" => accent('˘', false, command, source, base),
        "check" => accent('ˇ', false, command, source, base),
        "overset" => stack(command, source, true, base),
        "underset" => stack(command, source, false, base),
        "stackrel" => MathExpression::Relation(Box::new(stack(command, source, true, base))),
        "quad" => MathExpression::Space { kind: MathSpace::Quad },
        "qquad" => MathExpression::Space { kind: MathSpace::DoubleQuad },
        "," | "thinspace" => MathExpression::Space { kind: MathSpace::Thin },
        ":" | "medspace" => MathExpression::Space { kind: MathSpace::Medium },
        ";" | "thickspace" => MathExpression::Space { kind: MathSpace::Thick },
        " " | "space" => MathExpression::Space { kind: MathSpace::Interword },
        "!" | "negthinspace" => MathExpression::Space { kind: MathSpace::NegativeThin },
        "displaystyle" => MathExpression::StyleSwitch { style: MathStyleKind::Display },
        "textstyle" => MathExpression::StyleSwitch { style: MathStyleKind::Text },
        "scriptstyle" => MathExpression::StyleSwitch { style: MathStyleKind::Script },
        "scriptscriptstyle" => MathExpression::StyleSwitch { style: MathStyleKind::ScriptScript },
        "{" => char_atom('{', MathClass::Opening, command_span),
        "}" => char_atom('}', MathClass::Closing, command_span),
        "|" => char_atom('‖', MathClass::Ordinary, command_span),
        _ => {
            if let Some((text, class)) = symbol_for(name) {
                return MathExpression::Atom(MathAtom { text: text.to_string(), class, span: command_span });
            }
            if named_function(name) {
                return MathExpression::Text { text: name.to_string(), span: command_span };
            }
            unsupported(command.span.start, command.span.end, source, "command", base)
        }
    }
}

fn fraction(command: &TexCommand, source: &str, line: MathFractionLine, base: usize) -> MathExpression {
    let arguments: Vec<MathExpression> = command.arguments.iter().map(|argument| lower_argument(argument, source, base)).collect();
    match arguments.as_slice() {
        [numerator, denominator] => {
            MathExpression::Fraction { numerator: Box::new(numerator.clone()), denominator: Box::new(denominator.clone()), line }
        }
        _ => unsupported(command.span.start, command.span.end, source, "fraction", base),
    }
}

fn styled(style: MathStyle, command: &TexCommand, source: &str, base: usize) -> MathExpression {
    let body =
        command.arguments.first().map(|argument| lower_argument(argument, source, base)).unwrap_or(MathExpression::Sequence(Vec::new()));
    MathExpression::Styled { style, body: Box::new(body) }
}

fn accent(mark: char, wide: bool, command: &TexCommand, source: &str, base: usize) -> MathExpression {
    let body =
        command.arguments.first().map(|argument| lower_argument(argument, source, base)).unwrap_or(MathExpression::Sequence(Vec::new()));
    MathExpression::Accent { mark, wide, base: Box::new(body), span: span_from(base, command.span.start, command.span.end) }
}

fn stack(command: &TexCommand, source: &str, above: bool, base: usize) -> MathExpression {
    let mut arguments = command.arguments.iter();
    let stacked =
        arguments.next().map(|argument| lower_argument(argument, source, base)).unwrap_or(MathExpression::Sequence(Vec::new()));
    let base_expression =
        arguments.next().map(|argument| lower_argument(argument, source, base)).unwrap_or(MathExpression::Sequence(Vec::new()));
    if above {
        MathExpression::StackAbove { over: Box::new(stacked), base: Box::new(base_expression) }
    }
    else {
        MathExpression::StackBelow { under: Box::new(stacked), base: Box::new(base_expression) }
    }
}

fn lower_radical(command: &TexCommand, source: &str, base: usize) -> MathExpression {
    let mut index = None;
    let mut radicand = None;
    for argument in &command.arguments {
        match argument {
            TexArgument::Optional(root) if index.is_none() => index = Some(lower_root(&root.items, source, base)),
            TexArgument::Required(root) if radicand.is_none() => radicand = Some(lower_root(&root.items, source, base)),
            _ => {}
        }
    }
    match radicand {
        Some(radicand) => MathExpression::Radical {
            index: index.map(Box::new),
            radicand: Box::new(radicand),
            span: span_from(base, command.span.start, command.span.end),
        },
        None => unsupported(command.span.start, command.span.end, source, "radical", base),
    }
}

fn lower_argument(argument: &TexArgument, source: &str, base: usize) -> MathExpression {
    match argument {
        TexArgument::Optional(root) | TexArgument::Required(root) => lower_root(&root.items, source, base),
    }
}

fn argument_text(argument: &TexArgument) -> Option<String> {
    match argument {
        TexArgument::Optional(root) | TexArgument::Required(root) => {
            let text: String = root.items.iter().filter_map(item_text).collect();
            (!text.is_empty()).then_some(text)
        }
    }
}

/// The authored text of an argument, kept verbatim for metadata such as `\tag`.
fn argument_source(argument: &TexArgument, source: &str) -> Option<String> {
    match argument {
        TexArgument::Optional(root) | TexArgument::Required(root) => {
            let text = slice(source, root.span.start, root.span.end);
            let trimmed = text.trim();
            let body = trimmed.strip_prefix('{').and_then(|inner| inner.strip_suffix('}')).map(str::trim).unwrap_or(trimmed);
            (!body.is_empty()).then(|| body.to_string())
        }
    }
}

fn item_text(item: &TexItem) -> Option<String> {
    match item {
        TexItem::Text { content, .. } => Some(content.clone()),
        TexItem::Group(group) => Some(group.content.items.iter().filter_map(item_text).collect()),
        TexItem::Command(command) => Some(command.name.clone()),
        _ => None,
    }
}

fn lower_left_right(pair: &TexLeftRightPair, source: &str, base: usize) -> MathExpression {
    // The AST only keeps the whole-pair span, so both delimiters point at
    // the pair they were authored in.
    let pair_span = span_from(base, pair.span.start, pair.span.end);
    MathExpression::Delimited {
        open: delimiter(&pair.left_delim, pair_span),
        close: delimiter(&pair.right_delim, pair_span),
        body: Box::new(lower_root(&pair.content.items, source, base)),
        middles: Vec::new(),
    }
}

fn delimiter(spelling: &str, span: Option<MathSpan>) -> Option<MathDelimiter> {
    let spelling = spelling.trim();
    let (kind, character) = match spelling {
        "." | "" => return None,
        "(" => (MathDelimiterKind::Parenthesis, Some('(')),
        ")" => (MathDelimiterKind::Parenthesis, Some(')')),
        "[" => (MathDelimiterKind::Bracket, Some('[')),
        "]" => (MathDelimiterKind::Bracket, Some(']')),
        "\\{" | "{" => (MathDelimiterKind::Brace, Some('{')),
        "\\}" | "}" => (MathDelimiterKind::Brace, Some('}')),
        "\\langle" => (MathDelimiterKind::Angle, Some('⟨')),
        "\\rangle" => (MathDelimiterKind::Angle, Some('⟩')),
        "|" => (MathDelimiterKind::VerticalBar, Some('|')),
        "\\|" | "\\lVert" | "\\rVert" | "‖" => (MathDelimiterKind::DoubleVerticalBar, Some('‖')),
        "/" => (MathDelimiterKind::Slash, Some('/')),
        other => (MathDelimiterKind::Other, other.chars().next()),
    };
    Some(MathDelimiter { kind, character, span })
}

fn lower_environment(environment: &TexEnvironment, source: &str, base: usize) -> MathExpression {
    let name = environment.name.as_str();
    let kind = environment_kind(name);
    let numbered = !environment.starred && environment_is_numbered(name);
    let rows = environment_rows(environment, source, &kind, numbered, base);
    let column_alignment = column_alignment(name, &rows);
    let column_pairs = match kind {
        MathEnvironmentKind::AlignedAt | MathEnvironmentKind::AlignAt => {
            environment.arguments.iter().find_map(argument_text).and_then(|text| text.trim().parse::<u32>().ok())
        }
        _ => None,
    };
    MathExpression::Environment(MathEnvironment {
        kind,
        name: name.to_string(),
        starred: environment.starred,
        rows,
        column_alignment,
        column_pairs,
        span: span_from(base, environment.span.start, environment.span.end),
    })
}

fn environment_kind(name: &str) -> MathEnvironmentKind {
    match name {
        "matrix" | "pmatrix" | "bmatrix" | "Bmatrix" | "vmatrix" | "Vmatrix" | "smallmatrix" => MathEnvironmentKind::Matrix,
        "cases" => MathEnvironmentKind::Cases,
        "aligned" => MathEnvironmentKind::Aligned,
        "alignedat" => MathEnvironmentKind::AlignedAt,
        "align" => MathEnvironmentKind::Align,
        "alignat" => MathEnvironmentKind::AlignAt,
        "flalign" => MathEnvironmentKind::FlAlign,
        "gather" | "gather*" => MathEnvironmentKind::Gather,
        "gathered" => MathEnvironmentKind::Gathered,
        "split" => MathEnvironmentKind::Split,
        "equation" | "displaymath" => MathEnvironmentKind::Equation,
        "multline" => MathEnvironmentKind::Multline,
        "eqnarray" => MathEnvironmentKind::EqnArray,
        other => MathEnvironmentKind::Other(other.to_string()),
    }
}

fn column_alignment(name: &str, rows: &[MathRow]) -> Vec<MathColumnAlignment> {
    let columns = rows.iter().map(|row| row.cells.len()).max().unwrap_or(0);
    match name {
        "cases" => vec![MathColumnAlignment::Left; columns],
        "aligned" | "alignedat" | "align" | "alignat" | "flalign" | "split" => {
            (0..columns).map(|index| if index % 2 == 0 { MathColumnAlignment::Right } else { MathColumnAlignment::Left }).collect()
        }
        "gather" | "gathered" | "equation" | "multline" => vec![MathColumnAlignment::Center; columns.max(1)],
        _ => vec![MathColumnAlignment::Center; columns],
    }
}

fn environment_rows(environment: &TexEnvironment, source: &str, kind: &MathEnvironmentKind, numbered: bool, base: usize) -> Vec<MathRow> {
    let authored: Vec<&TexRow> = environment
        .content
        .items
        .iter()
        .filter_map(|item| match item {
            TexItem::Row(row) => Some(row),
            _ => None,
        })
        .collect();
    if authored.is_empty() {
        let cells =
            if environment.content.items.is_empty() { Vec::new() } else { vec![lower_root(&environment.content.items, source, base)] };
        return vec![MathRow { cells, placement: MathRowPlacement::Normal, numbering: MathRowNumbering::None }];
    }
    let total = authored.len();
    authored
        .iter()
        .enumerate()
        .map(|(index, row)| MathRow {
            cells: row.cells.iter().map(|cell| lower_cell(cell, source, base)).collect(),
            placement: row_placement(kind, index, total),
            numbering: row_numbering(&row.cells, numbered, source),
        })
        .collect()
}

fn row_placement(kind: &MathEnvironmentKind, index: usize, total: usize) -> MathRowPlacement {
    if !matches!(kind, MathEnvironmentKind::Multline) {
        return MathRowPlacement::Normal;
    }
    if total == 1 {
        MathRowPlacement::Centered
    }
    else if index == 0 {
        MathRowPlacement::FlushLeft
    }
    else if index + 1 == total {
        MathRowPlacement::FlushRight
    }
    else {
        MathRowPlacement::Centered
    }
}

fn lower_cell(cell: &TexCell, source: &str, base: usize) -> MathExpression {
    // Row metadata describes the row; it is not an atom of the cell.
    let items: Vec<TexItem> = cell
        .content
        .items
        .iter()
        .filter(|item| !matches!(item, TexItem::Command(command) if is_row_metadata(command)))
        .cloned()
        .collect();
    lower_root(&items, source, base)
}

/// Read the row's numbering intent from the command nodes inside its cells.
fn row_numbering(cells: &[TexCell], numbered: bool, source: &str) -> MathRowNumbering {
    let mut tag = None;
    let mut suppressed = false;
    for cell in cells {
        for item in &cell.content.items {
            let TexItem::Command(command) = item
            else {
                continue;
            };
            match command.name.as_str() {
                "notag" | "nonumber" => suppressed = true,
                "tag" => tag = command.arguments.iter().find_map(|argument| argument_source(argument, source)),
                _ => {}
            }
        }
    }
    // An explicit label is the stronger statement, so `\tag` wins over
    // `\notag` when a row carries both.
    if let Some(tag) = tag {
        MathRowNumbering::Tagged(tag)
    }
    else if suppressed {
        MathRowNumbering::Suppressed
    }
    else if numbered {
        MathRowNumbering::Numbered
    }
    else {
        MathRowNumbering::None
    }
}

fn environment_is_numbered(name: &str) -> bool {
    matches!(name, "equation" | "align" | "gather" | "multline" | "eqnarray" | "flalign" | "alignat")
}

/// Row facts the importer registers as relations and numbering.
#[derive(Debug, Default)]
pub(crate) struct RowMetadata {
    /// Labels authored on the row.
    pub labels: Vec<String>,
    /// Explicit `\tag` text.
    pub tag: Option<String>,
    /// Whether the row suppresses numbering.
    pub suppressed: bool,
    /// Whether the row carries no content besides metadata.
    pub empty: bool,
}

/// The rows of an environment body in source order.
pub(crate) fn environment_rows_of(environment: &TexEnvironment) -> Vec<&TexRow> {
    environment
        .content
        .items
        .iter()
        .filter_map(|item| match item {
            TexItem::Row(row) => Some(row),
            _ => None,
        })
        .collect()
}

/// Read labels, tag and suppression from the command nodes of one row.
pub(crate) fn row_metadata(row: &TexRow, source: &str) -> RowMetadata {
    let mut metadata = RowMetadata { empty: true, ..RowMetadata::default() };
    for cell in &row.cells {
        for item in &cell.content.items {
            match item {
                TexItem::Text { content, .. } if !content.trim().is_empty() => metadata.empty = false,
                TexItem::Command(command) => match command.name.as_str() {
                    "label" => {
                        if let Some(label) = command.arguments.iter().find_map(argument_text) {
                            metadata.labels.push(label);
                        }
                    }
                    "tag" => metadata.tag = command.arguments.iter().find_map(|argument| argument_source(argument, source)),
                    "notag" | "nonumber" => metadata.suppressed = true,
                    _ => metadata.empty = false,
                },
                TexItem::Comment { .. } => {}
                _ => metadata.empty = false,
            }
        }
    }
    metadata
}

fn text_expression(text: &str, start: usize, base: usize) -> MathExpression {
    if text.chars().all(|c| c.is_ascii_digit()) && !text.is_empty() {
        return MathExpression::Number { text: text.to_string(), span: span_from(base, start, start + text.len()) };
    }
    let atoms: Vec<MathExpression> = text
        .char_indices()
        .filter(|(_, c)| !c.is_whitespace())
        .map(|(offset, c)| char_expression(c, span_from(base, start + offset, start + offset + c.len_utf8())))
        .collect();
    sequence(atoms)
}

fn char_expression(c: char, span: Option<MathSpan>) -> MathExpression {
    if c.is_ascii_digit() {
        return MathExpression::Number { text: c.to_string(), span };
    }
    let class = class_for_char(c);
    char_atom(c, class, span)
}

fn char_atom(c: char, class: MathClass, span: Option<MathSpan>) -> MathExpression {
    MathExpression::Atom(MathAtom { text: c.to_string(), class, span })
}

fn class_for_char(c: char) -> MathClass {
    match c {
        '0'..='9' => MathClass::Number,
        'a'..='z' | 'A'..='Z' => MathClass::Identifier,
        '+' | '-' | '*' | '/' | '×' | '·' | '±' | '∓' | '÷' => MathClass::Binary,
        '=' | '<' | '>' => MathClass::Relation,
        '(' | '[' => MathClass::Opening,
        ')' | ']' => MathClass::Closing,
        ',' | ';' => MathClass::Punctuation,
        c if is_greek(c) => MathClass::Identifier,
        _ => MathClass::Ordinary,
    }
}

fn is_greek(c: char) -> bool {
    matches!(c, '\u{0391}'..='\u{03A9}' | '\u{03B1}'..='\u{03C9}' | '\u{03D1}' | '\u{03D5}' | '\u{03D6}' | '\u{03F0}' | '\u{03F1}' | '\u{03F5}')
}

fn symbol_for(name: &str) -> Option<(&'static str, MathClass)> {
    let symbol = match name {
        "alpha" => ("α", MathClass::Ordinary),
        "beta" => ("β", MathClass::Ordinary),
        "gamma" => ("γ", MathClass::Ordinary),
        "delta" => ("δ", MathClass::Ordinary),
        // TeX's plain command names the variant glyph, and the `var` form
        // names the ordinary letter.
        "epsilon" => ("ϵ", MathClass::Ordinary),
        "varepsilon" => ("ε", MathClass::Ordinary),
        "zeta" => ("ζ", MathClass::Ordinary),
        "eta" => ("η", MathClass::Ordinary),
        "theta" => ("θ", MathClass::Ordinary),
        "vartheta" => ("ϑ", MathClass::Ordinary),
        "iota" => ("ι", MathClass::Ordinary),
        "kappa" => ("κ", MathClass::Ordinary),
        "varkappa" => ("ϰ", MathClass::Ordinary),
        "lambda" => ("λ", MathClass::Ordinary),
        "mu" => ("μ", MathClass::Ordinary),
        "nu" => ("ν", MathClass::Ordinary),
        "xi" => ("ξ", MathClass::Ordinary),
        "omicron" => ("ο", MathClass::Ordinary),
        "pi" => ("π", MathClass::Ordinary),
        "varpi" => ("ϖ", MathClass::Ordinary),
        "rho" => ("ρ", MathClass::Ordinary),
        "varrho" => ("ϱ", MathClass::Ordinary),
        "sigma" => ("σ", MathClass::Ordinary),
        "varsigma" => ("ς", MathClass::Ordinary),
        "tau" => ("τ", MathClass::Ordinary),
        "upsilon" => ("υ", MathClass::Ordinary),
        "phi" => ("ϕ", MathClass::Ordinary),
        "varphi" => ("φ", MathClass::Ordinary),
        "chi" => ("χ", MathClass::Ordinary),
        "psi" => ("ψ", MathClass::Ordinary),
        "omega" => ("ω", MathClass::Ordinary),
        "Gamma" => ("Γ", MathClass::Ordinary),
        "Delta" => ("Δ", MathClass::Ordinary),
        "Theta" => ("Θ", MathClass::Ordinary),
        "Lambda" => ("Λ", MathClass::Ordinary),
        "Xi" => ("Ξ", MathClass::Ordinary),
        "Pi" => ("Π", MathClass::Ordinary),
        "Sigma" => ("Σ", MathClass::Ordinary),
        "Upsilon" => ("Υ", MathClass::Ordinary),
        "Phi" => ("Φ", MathClass::Ordinary),
        "Psi" => ("Ψ", MathClass::Ordinary),
        "Omega" => ("Ω", MathClass::Ordinary),
        "sum" => ("∑", MathClass::LargeOperator),
        "prod" => ("∏", MathClass::LargeOperator),
        "coprod" => ("∐", MathClass::LargeOperator),
        "int" => ("∫", MathClass::LargeOperator),
        "iint" => ("∬", MathClass::LargeOperator),
        "oint" => ("∮", MathClass::LargeOperator),
        "pm" => ("±", MathClass::Binary),
        "mp" => ("∓", MathClass::Binary),
        "times" => ("×", MathClass::Binary),
        "cdot" => ("⋅", MathClass::Binary),
        "div" => ("÷", MathClass::Binary),
        "ast" => ("∗", MathClass::Binary),
        "star" => ("⋆", MathClass::Binary),
        "bullet" => ("∙", MathClass::Binary),
        "cup" => ("∪", MathClass::Binary),
        "cap" => ("∩", MathClass::Binary),
        "circ" => ("∘", MathClass::Binary),
        "le" | "leq" => ("≤", MathClass::Relation),
        "ge" | "geq" => ("≥", MathClass::Relation),
        "ne" | "neq" => ("≠", MathClass::Relation),
        "approx" => ("≈", MathClass::Relation),
        "equiv" => ("≡", MathClass::Relation),
        "sim" => ("∼", MathClass::Relation),
        "propto" => ("∝", MathClass::Relation),
        "to" | "rightarrow" => ("→", MathClass::Relation),
        "leftarrow" => ("←", MathClass::Relation),
        "leftrightarrow" => ("↔", MathClass::Relation),
        "Rightarrow" => ("⇒", MathClass::Relation),
        "in" => ("∈", MathClass::Relation),
        "notin" => ("∉", MathClass::Relation),
        "subset" => ("⊂", MathClass::Relation),
        "subseteq" => ("⊆", MathClass::Relation),
        "supset" => ("⊃", MathClass::Relation),
        "perp" => ("⊥", MathClass::Relation),
        "parallel" => ("∥", MathClass::Relation),
        "infty" => ("∞", MathClass::Ordinary),
        "partial" => ("∂", MathClass::Ordinary),
        "nabla" => ("∇", MathClass::Ordinary),
        "forall" => ("∀", MathClass::Ordinary),
        "exists" => ("∃", MathClass::Ordinary),
        "emptyset" => ("∅", MathClass::Ordinary),
        "ldots" => ("…", MathClass::Ordinary),
        "cdots" => ("⋯", MathClass::Ordinary),
        "ddots" => ("⋱", MathClass::Ordinary),
        "prime" => ("′", MathClass::Ordinary),
        "angle" => ("∠", MathClass::Ordinary),
        "degree" => ("°", MathClass::Ordinary),
        _ => return None,
    };
    Some(symbol)
}

/// Named operators that render as upright multi-character text.
///
/// The name table is TeX's operator dictionary: a spelling is a named
/// function exactly when it is one of the documented operator names.
fn named_function(name: &str) -> bool {
    const NAMES: &[&str] = &[
        "lim", "limsup", "liminf", "max", "min", "sup", "inf", "det", "gcd", "Pr", "argmax", "argmin", "sin", "cos", "tan", "cot", "sec",
        "csc", "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh", "log", "ln", "lg", "exp", "deg", "dim", "hom", "ker",
    ];
    NAMES.contains(&name)
}

fn unsupported(start: usize, end: usize, source: &str, kind: &str, base: usize) -> MathExpression {
    MathExpression::Unsupported { kind: kind.to_string(), source: slice(source, start, end), span: span_from(base, start, end) }
}

fn slice(source: &str, start: usize, end: usize) -> String {
    source.get(start..end).unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use oak_core::{Builder, SourceText, parser::session::ParseSession};
    use oak_tex::{TexBuilder, TexLanguage};

    fn parse(source: &str) -> TexRoot {
        let config = TexLanguage::default();
        let builder = TexBuilder::new(&config);
        let text = SourceText::new(source);
        let mut session = ParseSession::<TexLanguage>::default();
        builder.build(&text, &[], &mut session).result.expect("TeX parses")
    }

    fn lower(source: &str) -> MathExpression {
        lower_tex_formula(&parse(source), source, MathDisplay::Inline, Some("latex")).expression
    }

    #[test]
    fn scripts_fold_onto_their_base() {
        let expression = lower("x^2");
        let MathExpression::Script { base, superscript, .. } = expression
        else {
            panic!("expected a script, got {expression:?}")
        };
        assert!(matches!(&*base, MathExpression::Atom(atom) if atom.text == "x"));
        assert!(matches!(superscript.as_deref(), Some(MathExpression::Number { text, .. }) if text == "2"));
    }

    #[test]
    fn fraction_and_radical_keep_their_structure() {
        let expression = lower(r"\frac{a}{b}");
        assert!(matches!(expression, MathExpression::Fraction { .. }));
        let expression = lower(r"\sqrt[3]{x}");
        assert!(matches!(expression, MathExpression::Radical { index: Some(_), .. }));
    }

    #[test]
    fn accents_and_stacking_keep_their_semantics() {
        let expression = lower(r"\hat{x}");
        assert!(matches!(expression, MathExpression::Accent { mark: 'ˆ', .. }));
        let expression = lower(r"\overset{!}{=}");
        assert!(matches!(expression, MathExpression::StackAbove { .. }));
    }

    #[test]
    fn alignedat_keeps_its_declared_column_pairs() {
        let source = r"\begin{alignedat}{2} a &= b \end{alignedat}";
        let MathExpression::Environment(environment) = lower(source)
        else {
            panic!("expected an environment")
        };
        assert_eq!(environment.kind, MathEnvironmentKind::AlignedAt);
        assert_eq!(environment.column_pairs, Some(2));
        assert!(matches!(&environment.rows[0].cells[0], MathExpression::Atom(atom) if atom.text == "a"));
    }

    #[test]
    fn starred_alignedat_keeps_its_declared_column_pairs() {
        let source = r"\begin{alignedat*}{2} a &= b \end{alignedat*}";
        let MathExpression::Environment(environment) = lower(source)
        else {
            panic!("expected an environment")
        };
        assert_eq!(environment.kind, MathEnvironmentKind::AlignedAt);
        assert_eq!(environment.column_pairs, Some(2));
        assert!(matches!(&environment.rows[0].cells[0], MathExpression::Atom(atom) if atom.text == "a"));
    }

    fn lower_formula(source: &str) -> MathFormula {
        lower_tex_formula_source(source, MathDisplay::Inline)
    }

    fn slice_span(formula: &MathFormula, span: MathSpan) -> String {
        formula.source.get(span.start as usize..span.end as usize).unwrap_or("").to_string()
    }

    #[test]
    fn atom_spans_index_the_trimmed_formula_source() {
        // Leading whitespace is trimmed out of the formula source, so spans
        // must still index the trimmed text.
        let formula = lower_formula("  x + \\alpha");
        let MathExpression::Sequence(items) = &formula.expression
        else {
            panic!("expected a sequence")
        };
        let [x, plus, alpha] = items.as_slice()
        else {
            panic!("expected three atoms")
        };
        let MathExpression::Atom(x) = x
        else {
            panic!("expected an atom")
        };
        let MathExpression::Atom(alpha) = alpha
        else {
            panic!("expected an atom")
        };
        assert_eq!(x.span, Some(MathSpan { start: 0, end: 1 }));
        assert_eq!(slice_span(&formula, x.span.unwrap()), "x");
        let MathExpression::Atom(plus) = plus
        else {
            panic!("expected an atom")
        };
        assert_eq!(plus.span, Some(MathSpan { start: 2, end: 3 }));
        assert_eq!(slice_span(&formula, plus.span.unwrap()), "+");
        // The multi-character command span covers the whole `\alpha`.
        assert_eq!(alpha.span, Some(MathSpan { start: 4, end: 10 }));
        assert_eq!(slice_span(&formula, alpha.span.unwrap()), "\\alpha");
    }

    #[test]
    fn accent_span_covers_the_command_and_the_base_keeps_its_own() {
        let formula = lower_formula("\\hat{x}");
        let MathExpression::Accent { base, span, .. } = &formula.expression
        else {
            panic!("expected an accent")
        };
        assert_eq!(*span, Some(MathSpan { start: 0, end: 7 }));
        assert_eq!(slice_span(&formula, span.unwrap()), "\\hat{x}");
        let MathExpression::Atom(atom) = &**base
        else {
            panic!("expected an atom base")
        };
        assert_eq!(atom.span, Some(MathSpan { start: 5, end: 6 }));
        assert_eq!(slice_span(&formula, atom.span.unwrap()), "x");
    }

    #[test]
    fn script_spans_stay_on_their_own_terms() {
        let formula = lower_formula("x^2");
        let MathExpression::Script { base, superscript, .. } = &formula.expression
        else {
            panic!("expected a script")
        };
        let MathExpression::Atom(base) = &**base
        else {
            panic!("expected an atom base")
        };
        assert_eq!(base.span, Some(MathSpan { start: 0, end: 1 }));
        let MathExpression::Number { span, .. } = &**superscript.as_ref().expect("a superscript")
        else {
            panic!("expected a number superscript")
        };
        assert_eq!(*span, Some(MathSpan { start: 2, end: 3 }));
        assert_eq!(slice_span(&formula, span.unwrap()), "2");
    }

    #[test]
    fn starred_alignedat_cells_carry_spans_into_the_environment() {
        let formula = lower_formula("\\begin{alignedat*}{2} a &= b \\end{alignedat*}");
        let MathExpression::Environment(environment) = &formula.expression
        else {
            panic!("expected an environment")
        };
        assert_eq!(environment.span, Some(MathSpan { start: 0, end: 45 }));
        let MathExpression::Atom(first) = &environment.rows[0].cells[0]
        else {
            panic!("expected an atom cell")
        };
        assert_eq!(first.span, Some(MathSpan { start: 22, end: 23 }));
        assert_eq!(slice_span(&formula, first.span.unwrap()), "a");
        // The second cell keeps the relation and the atom as separate spans.
        let MathExpression::Sequence(cell_items) = &environment.rows[0].cells[1]
        else {
            panic!("expected a sequence cell")
        };
        let MathExpression::Atom(relation) = &cell_items[0]
        else {
            panic!("expected a relation atom")
        };
        assert_eq!(relation.span, Some(MathSpan { start: 25, end: 26 }));
        assert_eq!(slice_span(&formula, relation.span.unwrap()), "=");
        let MathExpression::Atom(second) = &cell_items[1]
        else {
            panic!("expected an atom cell")
        };
        assert_eq!(second.span, Some(MathSpan { start: 27, end: 28 }));
        assert_eq!(slice_span(&formula, second.span.unwrap()), "b");
    }

    #[test]
    fn synthesized_binom_delimiters_carry_no_span() {
        let formula = lower_formula("\\binom{a}{b}");
        let MathExpression::Delimited { open, close, .. } = &formula.expression
        else {
            panic!("expected a delimited expression")
        };
        // The author typed no parentheses, so the synthesized delimiters
        // must not pretend to point at the source.
        assert!(open.is_some_and(|delimiter| delimiter.span.is_none()));
        assert!(close.is_some_and(|delimiter| delimiter.span.is_none()));
    }

    #[test]
    fn left_right_delimiters_point_at_their_pair() {
        let formula = lower_formula("\\left( x \\right)");
        let MathExpression::Delimited { open, close, .. } = &formula.expression
        else {
            panic!("expected a delimited expression")
        };
        let open = open.expect("an open delimiter");
        let close = close.expect("a close delimiter");
        assert_eq!(open.span, Some(MathSpan { start: 0, end: 16 }));
        assert_eq!(open.span, close.span);
    }

    #[test]
    fn unparsed_formula_keeps_no_span() {
        let formula = MathFormula::unsupported("x +", Some("latex".into()), MathDisplay::Inline, "unparsed");
        let MathExpression::Unsupported { span, .. } = &formula.expression
        else {
            panic!("expected an unsupported expression")
        };
        assert_eq!(*span, None);
    }

    #[test]
    fn environment_rows_and_cells_come_from_the_ast() {
        let source = r"\begin{cases}1 & x > 0 \\ 0 & x \le 0\end{cases}";
        let expression = lower(source);
        let MathExpression::Environment(environment) = expression
        else {
            panic!("expected an environment, got {expression:?}")
        };
        assert_eq!(environment.kind, MathEnvironmentKind::Cases);
        assert_eq!(environment.name, "cases");
        assert_eq!(environment.rows.len(), 2);
        assert_eq!(environment.rows[0].cells.len(), 2);
    }
}
