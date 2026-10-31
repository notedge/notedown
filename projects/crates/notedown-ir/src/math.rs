//! Language-neutral math semantics carried by the document IR.
//!
//! Syntax stays in Oak. This module only describes the mathematical
//! structure a document contains: sequences, atoms with their mathematical
//! class, scripts, fractions, radicals, delimited bodies and environments.
//! It deliberately carries no tokens, brackets, escapes or parser nodes.
//!
//! `MathFormula::source` preserves the authored text for round-trip, preview
//! and diagnostics. Consumers must render [`MathFormula::expression`]; they
//! must not parse `source` themselves.

use serde::{Deserialize, Serialize};

/// Whether a formula is typeset inline or as a display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathDisplay {
    /// Inline formula inside a text line.
    Inline,
    /// Display formula occupying its own line.
    Display,
}

/// A formula as it enters the document IR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MathFormula {
    /// Authored source text, kept for round-trip and diagnostics only.
    pub source: String,
    /// Lowering profile that produced `expression`, such as `latex` or `typst`.
    pub language: Option<String>,
    /// Inline or display.
    pub display: MathDisplay,
    /// The semantic expression consumers render.
    pub expression: MathExpression,
}

impl MathFormula {
    /// A formula whose language has no semantic lowering yet.
    ///
    /// The source stays visible for preview and round-trip, but consumers must
    /// treat the formula as unsupported instead of guessing a grammar.
    pub fn unsupported(source: impl Into<String>, language: Option<String>, display: MathDisplay, kind: impl Into<String>) -> Self {
        let source = source.into();
        Self { expression: MathExpression::Unsupported { kind: kind.into(), source: source.clone() }, source, language, display }
    }
}

/// One semantic math expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MathExpression {
    /// Ordered sequence of expressions.
    Sequence(Vec<MathExpression>),
    /// A single atom with its mathematical class.
    Atom(MathAtom),
    /// A semantic grouping that has no visible delimiter.
    Group {
        /// Grouped body.
        body: Box<MathExpression>,
    },
    /// A base with an optional superscript and subscript.
    Script {
        /// Base expression.
        base: Box<MathExpression>,
        /// Superscript, if authored.
        superscript: Option<Box<MathExpression>>,
        /// Subscript, if authored.
        subscript: Option<Box<MathExpression>>,
    },
    /// A fraction.
    Fraction {
        /// Numerator.
        numerator: Box<MathExpression>,
        /// Denominator.
        denominator: Box<MathExpression>,
    },
    /// A radical with an optional index.
    Radical {
        /// Radical index such as the `3` in a cube root.
        index: Option<Box<MathExpression>>,
        /// Radicand.
        radicand: Box<MathExpression>,
    },
    /// A delimited construct such as `\left( ... \right)`.
    Delimited {
        /// Opening delimiter, `None` for an invisible delimiter or none.
        open: Option<MathDelimiter>,
        /// Closing delimiter.
        close: Option<MathDelimiter>,
        /// Delimited body.
        body: Box<MathExpression>,
        /// Middle delimiters (`\middle`).
        middles: Vec<MathExpression>,
    },
    /// A mathematical face applied to a body, such as `\mathbf{...}`.
    Styled {
        /// The semantic face.
        style: MathStyle,
        /// Styled body.
        body: Box<MathExpression>,
    },
    /// A multi-row environment.
    Environment(MathEnvironment),
    /// Text embedded in math, such as `\text{...}`.
    Text {
        /// The literal text.
        text: String,
    },
    /// A fragment the lowering profile could not express.
    ///
    /// Consumers must keep the source visible as plain text and report the gap;
    /// they must never re-parse `source`.
    Unsupported {
        /// What the fragment is, for diagnostics.
        kind: String,
        /// The authored source of the fragment.
        source: String,
    },
}

/// A single math atom and its mathematical class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MathAtom {
    /// Authored text of the atom, already stripped of syntax markers.
    pub text: String,
    /// Mathematical class used for tagging and spacing semantics.
    pub class: MathClass,
}

/// The mathematical class of an atom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathClass {
    /// A variable-like identifier.
    Identifier,
    /// A numeric literal.
    Number,
    /// An operator such as `+` or `\sum`.
    Operator,
    /// A relation such as `=` or `\le`.
    Relation,
    /// Punctuation such as `,`.
    Punctuation,
    /// An ordinary symbol such as a parenthesis or a Greek letter.
    Ordinary,
    /// Explicit spacing.
    Space,
}

/// A semantic delimiter, independent of how the source spelled it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MathDelimiter {
    /// Delimiter family.
    pub kind: MathDelimiterKind,
    /// Authored character when it has one.
    pub character: Option<char>,
}

/// A semantic mathematical face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathStyle {
    /// Upright letters.
    Upright,
    /// Italic letters.
    Italic,
    /// Bold letters.
    Bold,
    /// Double-struck letters.
    DoubleStruck,
    /// Script/calligraphic letters.
    Script,
    /// Fraktur letters.
    Fraktur,
    /// Sans-serif letters.
    SansSerif,
    /// Monospace letters.
    Monospace,
}

/// The family of a semantic delimiter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathDelimiterKind {
    /// Invisible delimiter (`.`) or an absent delimiter.
    None,
    /// Parenthesis `(` / `)`.
    Parenthesis,
    /// Bracket `[` / `]`.
    Bracket,
    /// Brace `{` / `}`.
    Brace,
    /// Angle bracket `\langle` / `\rangle`.
    Angle,
    /// Vertical bar.
    VerticalBar,
    /// Double vertical bar.
    DoubleVerticalBar,
    /// Slash.
    Slash,
    /// A delimiter the semantic profile has no family for.
    Other,
}

/// A multi-row math environment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MathEnvironment {
    /// Environment family.
    pub kind: MathEnvironmentKind,
    /// Rows in source order.
    pub rows: Vec<MathRow>,
    /// Declared or inferred column alignment.
    pub column_alignment: Vec<MathColumnAlignment>,
}

/// The family of a math environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathEnvironmentKind {
    /// A matrix family environment.
    Matrix,
    /// `cases`.
    Cases,
    /// `aligned` / `alignedat`.
    Aligned,
    /// `align` / `alignat`.
    Align,
    /// `gathered`.
    Gathered,
    /// `split`.
    Split,
    /// `equation`.
    Equation,
    /// Any other environment, named verbatim.
    Other(String),
}

/// Column alignment inside an environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathColumnAlignment {
    /// Left aligned.
    Left,
    /// Centered.
    Center,
    /// Right aligned.
    Right,
    /// Spread across the column.
    Spread,
}

/// One row of a math environment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MathRow {
    /// Cells in source order.
    pub cells: Vec<MathExpression>,
    /// Horizontal placement of the row, used by `multline`.
    pub placement: MathRowPlacement,
    /// Row numbering intent.
    pub numbering: MathRowNumbering,
}

/// Horizontal placement of one environment row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathRowPlacement {
    /// Ordinary row on the environment grid.
    Normal,
    /// First line of `multline`.
    FlushLeft,
    /// Middle line of `multline`.
    Centered,
    /// Last line of `multline`.
    FlushRight,
}

/// Numbering intent of one environment row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathRowNumbering {
    /// No numbering information was authored.
    None,
    /// The row carries an automatic number.
    Numbered,
    /// The row explicitly suppresses numbering.
    Suppressed,
    /// The row carries an explicit label.
    Tagged(String),
}
