//! The TeX conformance corpus shared across the document pipeline.
//!
//! Each case pins one authored `.tex` document to the semantic result Notedown
//! guarantees after lowering: the block kinds in document order, the verbatim
//! LaTeX payloads kept for the layout stage, the loss codes that must appear,
//! and the loss codes that must never appear. Downstream layout consumers run
//! the same `tex` inputs against their own pipeline, so grammar, document
//! semantics, and typesetting are measured on one fixture set instead of three
//! drifting copies. A case that passes here is a document Notedown lowers
//! without silent loss; it says nothing about visual quality, which stays with
//! the layout stage.

use notedown_ir::Block;

/// One shared TeX fixture with its guaranteed lowering contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TexCorpusCase {
    /// Stable identifier, shared verbatim with downstream consumers.
    pub id: &'static str,
    /// The authored `.tex` document source.
    pub tex: &'static str,
    /// Block kinds the lowered graph must contain, in document order.
    pub expected_block_kinds: &'static [&'static str],
    /// LaTeX payloads that must appear verbatim in math blocks or inline
    /// math, in document order.
    pub expected_math_payloads: &'static [&'static str],
    /// Loss codes that must each be recorded at least once.
    pub expected_loss_codes: &'static [&'static str],
    /// Loss codes that must never be recorded.
    pub forbidden_loss_codes: &'static [&'static str],
    /// Document title the lowering must place in metadata, if the fixture
    /// declares one.
    pub expected_title: Option<&'static str>,
    /// Author names the lowering must place in metadata, in authored order.
    pub expected_authors: &'static [&'static str],
}

impl TexCorpusCase {
    /// A case that constrains block kinds only.
    const fn kinds(id: &'static str, tex: &'static str, expected_block_kinds: &'static [&'static str]) -> Self {
        Self {
            id,
            tex,
            expected_block_kinds,
            expected_math_payloads: &[],
            expected_loss_codes: &[],
            forbidden_loss_codes: &[],
            expected_title: None,
            expected_authors: &[],
        }
    }
}

/// Stable corpus name of a block kind.
///
/// The match is exhaustive over the closed `notedown-ir` block enum, so a new
/// block kind fails to compile here instead of vanishing from the corpus.
pub fn block_kind_name(block: &Block) -> &'static str {
    match block {
        Block::Section { .. } => "section",
        Block::Paragraph { .. } => "paragraph",
        Block::List { .. } => "list",
        Block::Table { .. } => "table",
        Block::Quote { .. } => "quote",
        Block::Code { .. } => "code",
        Block::Math { .. } => "math",
        Block::Diagram { .. } => "diagram",
        Block::Opaque { .. } => "opaque",
        Block::RawHtml { .. } => "raw-html",
    }
}

/// The full shared TeX conformance corpus.
pub fn tex_conformance_corpus() -> Vec<TexCorpusCase> {
    let mut cases = Vec::new();
    cases.push(TexCorpusCase {
        forbidden_loss_codes: &["import.tex.unsupported_command"],
        ..TexCorpusCase::kinds("plain-paragraphs", "hello   world\n\nsecond paragraph\n", &["paragraph", "paragraph"])
    });
    cases.push(TexCorpusCase {
        forbidden_loss_codes: &["import.tex.unsupported_command"],
        ..TexCorpusCase::kinds("comment-gobble", "a% note\nb\n", &["paragraph"])
    });
    cases.push(TexCorpusCase {
        forbidden_loss_codes: &["import.tex.unsupported_command"],
        ..TexCorpusCase::kinds("escaped-characters", "\\$5\\%\\&\\_\n", &["paragraph"])
    });
    cases.push(TexCorpusCase {
        expected_loss_codes: &["import.tex.partial_coverage"],
        ..TexCorpusCase::kinds("unknown-command", "\\frobnicate{thing}\n", &["opaque"])
    });
    cases.push(TexCorpusCase {
        expected_loss_codes: &["import.tex.unknown_environment"],
        ..TexCorpusCase::kinds("unknown-environment", "\\begin{mystery}payload\\end{mystery}\n", &["opaque"])
    });
    cases.push(TexCorpusCase {
        expected_title: Some("Energy Flow"),
        expected_authors: &["A. Chen", "B. Wu"],
        expected_loss_codes: &["import.tex.maketitle_dropped"],
        forbidden_loss_codes: &["import.tex.empty_title", "import.tex.empty_author"],
        ..TexCorpusCase::kinds("title-author-metadata", "\\title{Energy Flow}\n\\author{A. Chen}\n\\author{B. Wu}\n\\maketitle\n\\section{Intro}\nBody text.\n", &["section", "paragraph"])
    });
    cases.push(TexCorpusCase {
        expected_loss_codes: &["import.tex.starred_command"],
        ..TexCorpusCase::kinds("starred-section", "\\section*{Appendix}\n", &["section"])
    });
    cases.push(TexCorpusCase {
        forbidden_loss_codes: &["import.tex.unresolved_reference", "import.tex.label_anchor"],
        ..TexCorpusCase::kinds("section-label-ref", "\\section{Methods}\\label{sec:methods}\nSee \\ref{sec:methods} for details.\n", &["section", "paragraph"])
    });
    cases.push(TexCorpusCase {
        expected_loss_codes: &["import.tex.unresolved_reference"],
        ..TexCorpusCase::kinds("forward-reference-unresolved", "See \\ref{sec:missing}.\n", &["paragraph"])
    });
    cases.push(TexCorpusCase {
        expected_math_payloads: &["\\begin{equation}E=mc^2\\end{equation}"],
        expected_loss_codes: &["import.tex.math_numbering"],
        ..TexCorpusCase::kinds("equation-numbered", "\\begin{equation}E=mc^2\\end{equation}\n", &["math"])
    });
    cases.push(TexCorpusCase {
        expected_math_payloads: &["\\begin{gather*}a=b\\end{gather*}"],
        forbidden_loss_codes: &["import.tex.math_numbering"],
        ..TexCorpusCase::kinds("starred-gather-unnumbered", "\\begin{gather*}a=b\\end{gather*}\n", &["math"])
    });
    cases.push(TexCorpusCase {
        expected_math_payloads: &["\\begin{align}a&=b\\\\c&=d\\end{align}"],
        expected_loss_codes: &["import.tex.math_numbering"],
        ..TexCorpusCase::kinds("align-numbered", "\\begin{align}a&=b\\\\c&=d\\end{align}\n", &["math"])
    });
    cases.push(TexCorpusCase {
        expected_math_payloads: &["\\begin{equation}\\label{eq:einstein}E=mc^2\\end{equation}"],
        expected_loss_codes: &["import.tex.math_numbering"],
        forbidden_loss_codes: &["import.tex.unresolved_reference", "import.tex.label_anchor"],
        ..TexCorpusCase::kinds("equation-label-eqref", "\\begin{equation}\\label{eq:einstein}E=mc^2\\end{equation}\nSee \\eqref{eq:einstein}.\n", &["math", "paragraph"])
    });
    cases.push(TexCorpusCase {
        expected_math_payloads: &["\\int_0^1 x\\,dx"],
        forbidden_loss_codes: &["import.tex.math_numbering", "import.tex.space_marker"],
        ..TexCorpusCase::kinds("display-math-double-dollar", "$$\\int_0^1 x\\,dx$$\n", &["math"])
    });
    cases.push(TexCorpusCase {
        expected_math_payloads: &["x^2 + 1"],
        ..TexCorpusCase::kinds("inline-math", "The value $x^2 + 1$ appears.\n", &["paragraph"])
    });
    cases.push(TexCorpusCase {
        expected_math_payloads: &["\\left(\\frac{a}{b}\\right)"],
        ..TexCorpusCase::kinds("delimiter-pair-math", "The term $\\left(\\frac{a}{b}\\right)$ remains.\n", &["paragraph"])
    });
    cases.push(TexCorpusCase {
        expected_math_payloads: &["x"],
        expected_loss_codes: &["import.tex.unclosed_math"],
        ..TexCorpusCase::kinds("unclosed-math", "a $x\n", &["paragraph"])
    });
    cases.push(TexCorpusCase {
        expected_loss_codes: &["import.tex.space_marker"],
        ..TexCorpusCase::kinds("text-space-markers", "x\\,y\\!z\\quad w\n", &["paragraph"])
    });
    cases.push(TexCorpusCase {
        expected_loss_codes: &["import.tex.footnote_body"],
        ..TexCorpusCase::kinds("footnote", "Body\\footnote{A note here.} tail.\n", &["paragraph"])
    });
    cases.push(TexCorpusCase {
        forbidden_loss_codes: &["import.tex.empty_citation"],
        ..TexCorpusCase::kinds("citation", "As shown in \\cite{smith2020,jones2021}.\n", &["paragraph"])
    });
    cases.push(TexCorpusCase {
        forbidden_loss_codes: &["import.tex.empty_image_source"],
        ..TexCorpusCase::kinds("image-include", "\\includegraphics{figure.png}\n", &["paragraph"])
    });
    cases.push(TexCorpusCase::kinds("verbatim-code", "\\begin{verbatim}\nlet x = 1;\n\\end{verbatim}\n", &["code"]));
    cases.push(TexCorpusCase::kinds("itemize-list", "\\begin{itemize}\n\\item One\n\\item Two\n\\end{itemize}\n", &["list"]));
    cases.push(TexCorpusCase::kinds("tabular-table", "\\begin{tabular}{lc}\nName & Value \\\\\nA & 1 \\\\\n\\end{tabular}\n", &["table"]));
    cases
}
