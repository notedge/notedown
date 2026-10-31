//! TeX math AST lowering into [`notedown_ir::MathFormula`].
//!
//! This module consumes Oak's TeX syntax tree. It never rescans the source for
//! alignment tabs, row breaks, labels or command arguments: every structural
//! fact comes from `TexItem`, `TexRow`, `TexCell`, `TexCommand` and
//! `TexLeftRightPair` nodes with their spans.

use notedown_ir::{
    MathAtom, MathClass, MathColumnAlignment, MathDelimiter, MathDelimiterKind, MathDisplay, MathEnvironment, MathEnvironmentKind,
    MathExpression, MathFormula, MathRow, MathRowNumbering, MathRowPlacement, MathStyle,
};
use oak_tex::ast::{
    MathDelimiter as TexMathDelimiter, TexArgument, TexCell, TexCommand, TexEnvironment, TexItem, TexLeftRightPair, TexMath, TexRoot, TexRow,
};

/// Lower one Oak math node into the document math semantics.
pub(crate) fn lower_tex_math(math: &TexMath, source: &str) -> MathFormula {
    let display = if math.is_display { MathDisplay::Display } else { MathDisplay::Inline };
    MathFormula {
        source: math_body_source(math, source),
        language: Some("latex".into()),
        display,
        expression: lower_root(&math.content.items, source),
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
    MathFormula {
        source: slice(source, root.span.start, root.span.end).trim().to_string(),
        language: language.map(str::to_string),
        display,
        expression: lower_root(&root.items, source),
    }
}

/// Lower environment content, exposing the environment as a semantic node.
pub(crate) fn lower_tex_environment(environment: &TexEnvironment, source: &str, display: MathDisplay) -> MathFormula {
    let expression = lower_environment(environment, source);
    MathFormula {
        source: slice(source, environment.span.start, environment.span.end).trim().to_string(),
        language: Some("latex".into()),
        display,
        expression,
    }
}

/// Lower a list of TeX items into one expression.
pub(crate) fn lower_root(items: &[TexItem], source: &str) -> MathExpression {
    let mut expressions: Vec<MathExpression> = Vec::new();
    for item in items {
        match item {
            TexItem::Superscript(script) => attach_script(&mut expressions, Some(lower_root(&script.content.items, source)), None),
            TexItem::Subscript(script) => attach_script(&mut expressions, None, Some(lower_root(&script.content.items, source))),
            TexItem::Command(command) if is_row_metadata(command) => {}
            _ => {
                if let Some(expression) = lower_item(item, source) {
                    expressions.push(expression);
                }
            }
        }
    }
    sequence(expressions)
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
        let source = String::new();
        expressions.push(MathExpression::Unsupported { kind: "script-base".into(), source });
        return;
    };
    match last {
        MathExpression::Script { base, superscript: existing_sup, subscript: existing_sub } => {
            expressions.push(MathExpression::Script {
                base,
                superscript: superscript.map(Box::new).or(existing_sup),
                subscript: subscript.map(Box::new).or(existing_sub),
            });
        }
        base => {
            expressions.push(MathExpression::Script {
                base: Box::new(base),
                superscript: superscript.map(Box::new),
                subscript: subscript.map(Box::new),
            });
        }
    }
}

fn lower_item(item: &TexItem, source: &str) -> Option<MathExpression> {
    match item {
        TexItem::Text { content, .. } => Some(text_expression(content)),
        TexItem::Command(command) => Some(lower_command(command, source)),
        TexItem::Group(group) => Some(MathExpression::Group { body: Box::new(lower_root(&group.content.items, source)) }),
        TexItem::Math(math) => Some(lower_root(&math.content.items, source)),
        TexItem::Environment(environment) => Some(lower_environment(environment, source)),
        TexItem::LeftRightPair(pair) => Some(lower_left_right(pair, source)),
        TexItem::Row(_) | TexItem::Superscript(_) | TexItem::Subscript(_) | TexItem::Limits(_) => {
            let span = item_span(item);
            Some(MathExpression::Unsupported { kind: "math-item".into(), source: slice(source, span.0, span.1) })
        }
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

fn lower_command(command: &TexCommand, source: &str) -> MathExpression {
    let name = command.name.as_str();
    match name {
        "frac" | "dfrac" | "tfrac" => {
            let arguments: Vec<MathExpression> = command.arguments.iter().map(|argument| lower_argument(argument, source)).collect();
            match arguments.as_slice() {
                [numerator, denominator] => {
                    MathExpression::Fraction { numerator: Box::new(numerator.clone()), denominator: Box::new(denominator.clone()) }
                }
                _ => unsupported(command.span.start, command.span.end, source, "fraction"),
            }
        }
        "sqrt" => lower_radical(command, source),
        "text" | "mbox" | "textrm" => match command.arguments.iter().find_map(argument_text) {
            Some(text) => MathExpression::Text { text },
            None => unsupported(command.span.start, command.span.end, source, "text"),
        },
        "mathrm" => styled(MathStyle::Upright, command, source),
        "mathit" => styled(MathStyle::Italic, command, source),
        "mathbf" => styled(MathStyle::Bold, command, source),
        "mathbb" => styled(MathStyle::DoubleStruck, command, source),
        "mathcal" => styled(MathStyle::Script, command, source),
        "mathfrak" => styled(MathStyle::Fraktur, command, source),
        "mathsf" => styled(MathStyle::SansSerif, command, source),
        "mathtt" => styled(MathStyle::Monospace, command, source),
        "limits" | "nolimits" => unsupported(command.span.start, command.span.end, source, "limits"),
        "left" | "right" | "middle" => unsupported(command.span.start, command.span.end, source, "delimiter-command"),
        _ => match symbol_for(name) {
            Some((text, class)) => MathExpression::Atom(MathAtom { text: text.to_string(), class }),
            None => unsupported(command.span.start, command.span.end, source, "command"),
        },
    }
}

fn styled(style: MathStyle, command: &TexCommand, source: &str) -> MathExpression {
    let body = command.arguments.first().map(|argument| lower_argument(argument, source)).unwrap_or(MathExpression::Sequence(Vec::new()));
    MathExpression::Styled { style, body: Box::new(body) }
}

fn lower_radical(command: &TexCommand, source: &str) -> MathExpression {
    let mut index = None;
    let mut radicand = None;
    for argument in &command.arguments {
        match argument {
            TexArgument::Optional(root) if index.is_none() => index = Some(lower_root(&root.items, source)),
            TexArgument::Required(root) if radicand.is_none() => radicand = Some(lower_root(&root.items, source)),
            _ => {}
        }
    }
    match radicand {
        Some(radicand) => MathExpression::Radical { index: index.map(Box::new), radicand: Box::new(radicand) },
        None => unsupported(command.span.start, command.span.end, source, "radical"),
    }
}

fn lower_argument(argument: &TexArgument, source: &str) -> MathExpression {
    match argument {
        TexArgument::Optional(root) | TexArgument::Required(root) => lower_root(&root.items, source),
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

fn item_text(item: &TexItem) -> Option<String> {
    match item {
        TexItem::Text { content, .. } => Some(content.clone()),
        TexItem::Group(group) => Some(group.content.items.iter().filter_map(item_text).collect()),
        TexItem::Command(command) => Some(command.name.clone()),
        _ => None,
    }
}

fn lower_left_right(pair: &TexLeftRightPair, source: &str) -> MathExpression {
    MathExpression::Delimited {
        open: delimiter(&pair.left_delim),
        close: delimiter(&pair.right_delim),
        body: Box::new(lower_root(&pair.content.items, source)),
        middles: Vec::new(),
    }
}

fn delimiter(spelling: &str) -> Option<MathDelimiter> {
    let spelling = spelling.trim();
    let (kind, character) = match spelling {
        "." | "" => return None,
        "(" | ")" => (MathDelimiterKind::Parenthesis, Some('(')),
        "[" | "]" => (MathDelimiterKind::Bracket, Some('[')),
        "\\{" | "\\}" | "{" | "}" => (MathDelimiterKind::Brace, Some('{')),
        "\\langle" | "\\rangle" => (MathDelimiterKind::Angle, Some('<')),
        "|" => (MathDelimiterKind::VerticalBar, Some('|')),
        "\\|" | "\\lVert" | "\\rVert" => (MathDelimiterKind::DoubleVerticalBar, Some('|')),
        "/" => (MathDelimiterKind::Slash, Some('/')),
        other => (MathDelimiterKind::Other, other.chars().next()),
    };
    Some(MathDelimiter { kind, character })
}

fn lower_environment(environment: &TexEnvironment, source: &str) -> MathExpression {
    let kind = environment_kind(&environment.name);
    let rows = environment_rows(environment, source);
    MathExpression::Environment(MathEnvironment { kind, rows, column_alignment: column_alignment(&environment.name) })
}

fn environment_kind(name: &str) -> MathEnvironmentKind {
    match name {
        "matrix" | "pmatrix" | "bmatrix" | "Bmatrix" | "vmatrix" | "Vmatrix" | "smallmatrix" => MathEnvironmentKind::Matrix,
        "cases" => MathEnvironmentKind::Cases,
        "aligned" | "alignedat" => MathEnvironmentKind::Aligned,
        "align" | "alignat" | "flalign" => MathEnvironmentKind::Align,
        "gathered" | "gather" => MathEnvironmentKind::Gathered,
        "split" => MathEnvironmentKind::Split,
        "equation" | "displaymath" => MathEnvironmentKind::Equation,
        other => MathEnvironmentKind::Other(other.to_string()),
    }
}

fn column_alignment(name: &str) -> Vec<MathColumnAlignment> {
    match name {
        "aligned" | "alignedat" | "align" | "alignat" | "flalign" | "split" => {
            vec![MathColumnAlignment::Right, MathColumnAlignment::Left]
        }
        "equation" | "displaymath" | "gather" | "gathered" | "multline" | "cases" => vec![MathColumnAlignment::Center],
        _ => Vec::new(),
    }
}

fn environment_rows(environment: &TexEnvironment, source: &str) -> Vec<MathRow> {
    let numbered = !environment.starred && environment_is_numbered(&environment.name);
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
        let cells = if environment.content.items.is_empty() { Vec::new() } else { vec![lower_root(&environment.content.items, source)] };
        return vec![MathRow { cells, placement: MathRowPlacement::Normal, numbering: MathRowNumbering::None }];
    }
    authored.iter().map(|row| lower_row(row, source, numbered)).collect()
}

fn lower_row(row: &TexRow, source: &str, numbered: bool) -> MathRow {
    let cells: Vec<MathExpression> = row.cells.iter().map(|cell| lower_cell(cell, source)).collect();
    // Row placement needs the `multline` line rules, which the syntax layer
    // does not express yet; every row stays on the ordinary grid.
    let placement = MathRowPlacement::Normal;
    MathRow { cells, placement, numbering: row_numbering(&row.cells, numbered) }
}

fn lower_cell(cell: &TexCell, source: &str) -> MathExpression {
    lower_root(&cell.content.items, source)
}

/// Read the row's numbering intent from the command nodes inside its cells.
fn row_numbering(cells: &[TexCell], numbered: bool) -> MathRowNumbering {
    for cell in cells {
        for item in &cell.content.items {
            let TexItem::Command(command) = item
            else {
                continue;
            };
            match command.name.as_str() {
                "notag" | "nonumber" => return MathRowNumbering::Suppressed,
                "tag" => {
                    if let Some(label) = command.arguments.iter().find_map(argument_text) {
                        return MathRowNumbering::Tagged(label);
                    }
                }
                _ => {}
            }
        }
    }
    if numbered { MathRowNumbering::Numbered } else { MathRowNumbering::None }
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
pub(crate) fn row_metadata(row: &TexRow) -> RowMetadata {
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
                    "tag" => metadata.tag = command.arguments.iter().find_map(argument_text),
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

fn text_expression(text: &str) -> MathExpression {
    let atoms: Vec<MathExpression> = text.chars().map(atom_for_char).collect();
    sequence(atoms)
}

fn atom_for_char(c: char) -> MathExpression {
    MathExpression::Atom(MathAtom { text: c.to_string(), class: class_for_char(c) })
}

fn class_for_char(c: char) -> MathClass {
    match c {
        '0'..='9' => MathClass::Number,
        'a'..='z' | 'A'..='Z' => MathClass::Identifier,
        '=' | '<' | '>' => MathClass::Relation,
        '+' | '-' | '*' | '/' | '±' | '∓' | '×' | '÷' | '·' => MathClass::Operator,
        ',' | ';' | ':' => MathClass::Punctuation,
        c if c.is_whitespace() => MathClass::Space,
        c if is_greek(c) => MathClass::Identifier,
        _ => MathClass::Ordinary,
    }
}

fn is_greek(c: char) -> bool {
    matches!(c, '\u{0391}'..='\u{03A9}' | '\u{03B1}'..='\u{03C9}' | '\u{03D1}' | '\u{03D5}' | '\u{03D6}' | '\u{03F0}' | '\u{03F1}' | '\u{03F5}')
}

fn symbol_for(name: &str) -> Option<(&'static str, MathClass)> {
    let symbol = match name {
        "alpha" => ("α", MathClass::Identifier),
        "beta" => ("β", MathClass::Identifier),
        "gamma" => ("γ", MathClass::Identifier),
        "delta" => ("δ", MathClass::Identifier),
        "epsilon" => ("ϵ", MathClass::Identifier),
        "varepsilon" => ("ε", MathClass::Identifier),
        "zeta" => ("ζ", MathClass::Identifier),
        "eta" => ("η", MathClass::Identifier),
        "theta" => ("θ", MathClass::Identifier),
        "vartheta" => ("ϑ", MathClass::Identifier),
        "iota" => ("ι", MathClass::Identifier),
        "kappa" => ("κ", MathClass::Identifier),
        "varkappa" => ("ϰ", MathClass::Identifier),
        "lambda" => ("λ", MathClass::Identifier),
        "mu" => ("μ", MathClass::Identifier),
        "nu" => ("ν", MathClass::Identifier),
        "xi" => ("ξ", MathClass::Identifier),
        "omicron" => ("ο", MathClass::Identifier),
        "pi" => ("π", MathClass::Identifier),
        "varpi" => ("ϖ", MathClass::Identifier),
        "rho" => ("ρ", MathClass::Identifier),
        "varrho" => ("ϱ", MathClass::Identifier),
        "sigma" => ("σ", MathClass::Identifier),
        "varsigma" => ("ς", MathClass::Identifier),
        "tau" => ("τ", MathClass::Identifier),
        "upsilon" => ("υ", MathClass::Identifier),
        "phi" => ("ϕ", MathClass::Identifier),
        "varphi" => ("φ", MathClass::Identifier),
        "chi" => ("χ", MathClass::Identifier),
        "psi" => ("ψ", MathClass::Identifier),
        "omega" => ("ω", MathClass::Identifier),
        "Gamma" => ("Γ", MathClass::Identifier),
        "Delta" => ("Δ", MathClass::Identifier),
        "Theta" => ("Θ", MathClass::Identifier),
        "Lambda" => ("Λ", MathClass::Identifier),
        "Xi" => ("Ξ", MathClass::Identifier),
        "Pi" => ("Π", MathClass::Identifier),
        "Sigma" => ("Σ", MathClass::Identifier),
        "Upsilon" => ("Υ", MathClass::Identifier),
        "Phi" => ("Φ", MathClass::Identifier),
        "Psi" => ("Ψ", MathClass::Identifier),
        "Omega" => ("Ω", MathClass::Identifier),
        "sum" => ("∑", MathClass::Operator),
        "prod" => ("∏", MathClass::Operator),
        "coprod" => ("∐", MathClass::Operator),
        "int" => ("∫", MathClass::Operator),
        "iint" => ("∬", MathClass::Operator),
        "oint" => ("∮", MathClass::Operator),
        "lim" => ("lim", MathClass::Operator),
        "pm" => ("±", MathClass::Operator),
        "mp" => ("∓", MathClass::Operator),
        "times" => ("×", MathClass::Operator),
        "cdot" => ("·", MathClass::Operator),
        "div" => ("÷", MathClass::Operator),
        "cup" => ("∪", MathClass::Operator),
        "cap" => ("∩", MathClass::Operator),
        "setminus" => ("∖", MathClass::Operator),
        "oplus" => ("⊕", MathClass::Operator),
        "otimes" => ("⊗", MathClass::Operator),
        "le" | "leq" => ("≤", MathClass::Relation),
        "ge" | "geq" => ("≥", MathClass::Relation),
        "ne" | "neq" => ("≠", MathClass::Relation),
        "approx" => ("≈", MathClass::Relation),
        "equiv" => ("≡", MathClass::Relation),
        "sim" => ("∼", MathClass::Relation),
        "propto" => ("∝", MathClass::Relation),
        "in" => ("∈", MathClass::Relation),
        "notin" => ("∉", MathClass::Relation),
        "subset" => ("⊂", MathClass::Relation),
        "supset" => ("⊃", MathClass::Relation),
        "to" | "rightarrow" => ("→", MathClass::Relation),
        "leftarrow" => ("←", MathClass::Relation),
        "Rightarrow" => ("⇒", MathClass::Relation),
        "Leftarrow" => ("⇐", MathClass::Relation),
        "leftrightarrow" => ("↔", MathClass::Relation),
        "mapsto" => ("↦", MathClass::Relation),
        "forall" => ("∀", MathClass::Ordinary),
        "exists" => ("∃", MathClass::Ordinary),
        "partial" => ("∂", MathClass::Identifier),
        "nabla" => ("∇", MathClass::Ordinary),
        "infty" => ("∞", MathClass::Ordinary),
        "emptyset" => ("∅", MathClass::Ordinary),
        "ell" => ("ℓ", MathClass::Identifier),
        "ldots" | "dots" => ("…", MathClass::Punctuation),
        "cdots" => ("⋯", MathClass::Punctuation),
        "vdots" => ("⋮", MathClass::Punctuation),
        "quad" => (" ", MathClass::Space),
        "qquad" => ("  ", MathClass::Space),
        "," | "thinspace" => (" ", MathClass::Space),
        ":" | "medspace" => (" ", MathClass::Space),
        ";" | "thickspace" => (" ", MathClass::Space),
        "!" | "negthinspace" => ("", MathClass::Space),
        "{" => ("{", MathClass::Ordinary),
        "}" => ("}", MathClass::Ordinary),
        "_" => ("_", MathClass::Ordinary),
        "&" => ("&", MathClass::Ordinary),
        "%" => ("%", MathClass::Ordinary),
        "$" => ("$", MathClass::Ordinary),
        "#" => ("#", MathClass::Ordinary),
        _ => return None,
    };
    Some(symbol)
}

fn unsupported(start: usize, end: usize, source: &str, kind: &str) -> MathExpression {
    MathExpression::Unsupported { kind: kind.to_string(), source: slice(source, start, end) }
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
        assert!(matches!(superscript.as_deref(), Some(MathExpression::Atom(atom)) if atom.text == "2"));
    }

    #[test]
    fn fraction_and_radical_keep_their_structure() {
        let expression = lower(r"\frac{a}{b}");
        assert!(matches!(expression, MathExpression::Fraction { .. }));
        let expression = lower(r"\sqrt[3]{x}");
        assert!(matches!(expression, MathExpression::Radical { index: Some(_), .. }));
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
        assert_eq!(environment.rows.len(), 2);
        assert_eq!(environment.rows[0].cells.len(), 2);
    }
}
