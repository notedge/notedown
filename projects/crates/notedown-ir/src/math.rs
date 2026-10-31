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
    /// A numeric literal kept as one atom, such as `10`.
    Number {
        /// Digits of the number.
        text: String,
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
        /// Fraction line style.
        line: MathFractionLine,
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
    /// An accent placed over a base, such as `\hat{x}`.
    Accent {
        /// Accent mark character.
        mark: char,
        /// True for the wide accent variants such as `\widehat`.
        wide: bool,
        /// Accented base.
        base: Box<MathExpression>,
    },
    /// `\overset{over}{base}` — `over` stacked above `base`.
    StackAbove {
        /// Stacked top.
        over: Box<MathExpression>,
        /// Base expression.
        base: Box<MathExpression>,
    },
    /// `\underset{under}{base}` — `under` stacked below `base`.
    StackBelow {
        /// Stacked bottom.
        under: Box<MathExpression>,
        /// Base expression.
        base: Box<MathExpression>,
    },
    /// A wrapper that forces relation spacing, such as `\stackrel`.
    Relation(Box<MathExpression>),
    /// Explicit horizontal space.
    Space {
        /// Space amount.
        kind: MathSpace,
    },
    /// Imperative style switch such as `\displaystyle`.
    StyleSwitch {
        /// The style the following content is set in.
        style: MathStyleKind,
    },
    /// Imperative limits placement for the next scripted operator.
    LimitsPolicy {
        /// True for `\limits`, false for `\nolimits`.
        limits: bool,
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
    /// A named operator such as `\sin`.
    Operator,
    /// A binary operator such as `+`.
    Binary,
    /// A relation such as `=` or `\le`.
    Relation,
    /// An opening delimiter such as `(`.
    Opening,
    /// A closing delimiter such as `)`.
    Closing,
    /// Punctuation such as `,`.
    Punctuation,
    /// An ordinary symbol such as a Greek letter.
    Ordinary,
    /// A large operator such as `\sum`.
    LargeOperator,
    /// An inner atom.
    Inner,
    /// Explicit spacing.
    Space,
}

/// Explicit horizontal space requested by a math command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathSpace {
    /// `\,` thin space, 3mu.
    Thin,
    /// `\:` medium space, 4mu.
    Medium,
    /// `\;` thick space, 5mu.
    Thick,
    /// `\ ` interword space, 6mu.
    Interword,
    /// `\quad`, 18mu (one em).
    Quad,
    /// `\qquad`, 36mu (two em).
    DoubleQuad,
    /// `\!` negative thin space.
    NegativeThin,
}

/// The math style an imperative style switch selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathStyleKind {
    /// `\displaystyle`.
    Display,
    /// `\textstyle`.
    Text,
    /// `\scriptstyle`.
    Script,
    /// `\scriptscriptstyle`.
    ScriptScript,
}

/// Fraction line style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathFractionLine {
    /// A solid fraction bar.
    Solid,
    /// A fraction with no bar, as in `\binom`.
    None,
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
    /// Authored environment name without the trailing star, such as
    /// `alignedat`.
    pub name: String,
    /// True when the authored environment carried a `*`.
    pub starred: bool,
    /// Rows in source order.
    pub rows: Vec<MathRow>,
    /// Declared or inferred column alignment.
    pub column_alignment: Vec<MathColumnAlignment>,
    /// Declared column-pair count for `alignedat`, when the source carries one.
    pub column_pairs: Option<u32>,
}

/// The family of a math environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathEnvironmentKind {
    /// A matrix family environment.
    Matrix,
    /// `cases`.
    Cases,
    /// `aligned`.
    Aligned,
    /// `alignedat`.
    AlignedAt,
    /// `align`.
    Align,
    /// `alignat`.
    AlignAt,
    /// `flalign`.
    FlAlign,
    /// `gather` and `gather*`.
    Gather,
    /// `gathered`.
    Gathered,
    /// `split`.
    Split,
    /// `equation`.
    Equation,
    /// `multline`.
    Multline,
    /// `eqnarray`.
    EqnArray,
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
