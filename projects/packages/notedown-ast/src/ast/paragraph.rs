use super::*;
use crate::hir::EscapeNode;

/// Sequence of paragraph ast
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParagraphSpan {
    pub terms: Vec<ParagraphTerm>,
    pub span: Range<u32>,
}

/// Item of paragraph ast
#[derive(Clone, Eq, PartialEq, Hash, From)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ParagraphTerm {
    /// Normal ast with white space
    Text(Box<TextPlainNode>),
    /// `*italic*`
    Italic(Box<FontItalicSpan>),
    /// `**bold**`
    Bold(Box<FontBoldSpan>),
    /// `**bold italic**`
    BoldItalic(Box<FontBoldItalicSpan>),
    /// `~underline~`
    Underline(Box<FontUnderlineSpan>),
    /// `~~delete~~`
    Delete(Box<FontDeleteSpan>),
    /// `` `code` ``
    Code(Box<CodeInlineSpan>),
    /// `$$display math$$`
    DisplayMath(Box<DisplayMathSpan>),
    /// `$inline math$`
    InlineMath(Box<InlineMathSpan>),
    /// `https://example.com`
    Uri(Box<UriNode>),
    /// `\cmd: rest of the line`
    CommandLine(Box<CommandLineSpan>),
    /// ` `
    WhiteSpace(Box<TextSpaceNode>),
    /// `\n`
    NewLine(Box<NewlineSpan>),
    /// `,`
    Comma(Box<CommaNode>),
    /// `.`
    Period(Box<PeriodNode>),
    /// `\\`
    Escape(Box<EscapeNode>),
}

impl ParagraphSpan {
    pub fn as_hir(&self) -> ParagraphNode {
        let mut terms: Vec<ParagraphKind> = Vec::with_capacity(self.terms.len());
        for term in &self.terms {
            match term {
                ParagraphTerm::Text(v) => terms.push(ParagraphKind::Plain(v.clone())),
                ParagraphTerm::WhiteSpace(v) => {
                    terms.push(ParagraphKind::Space(v.clone()));
                }
                ParagraphTerm::Italic(v) => {
                    terms.push(ParagraphKind::Style(Box::new(v.as_hir())));
                }
                ParagraphTerm::Bold(v) => {
                    terms.push(ParagraphKind::Style(Box::new(v.as_hir())));
                }
                ParagraphTerm::BoldItalic(v) => terms.push(ParagraphKind::Style(Box::new(v.as_hir()))),
                ParagraphTerm::NewLine(v) => {
                    terms.push(ParagraphKind::Space(Box::new(v.as_hir())));
                }
                ParagraphTerm::Comma(v) => {
                    terms.push(ParagraphKind::Plain(Box::new(v.as_hir())));
                }
                ParagraphTerm::Period(v) => {
                    terms.push(ParagraphKind::Plain(Box::new(v.as_hir())));
                }
                ParagraphTerm::Escape(v) => {
                    match v.escape {
                        // skip \s, \r, \n
                        c if c.is_ascii_whitespace() => continue,
                        'n' => terms.push(ParagraphKind::text('\n', v.span.clone())),
                        't' => terms.push(ParagraphKind::text('\t', v.span.clone())),
                        _ => terms.push(ParagraphKind::text(v.escape, v.span.clone())),
                    }
                    // terms.push(v.as_hir().into());
                }
                ParagraphTerm::Underline(v) => terms.push(ParagraphKind::Style(Box::new(v.as_hir()))),
                ParagraphTerm::Delete(v) => terms.push(ParagraphKind::Style(Box::new(v.as_hir()))),
                ParagraphTerm::Code(v) => terms.push(ParagraphKind::Code(Box::new(v.as_hir()))),
                ParagraphTerm::CommandLine(v) => terms.push(ParagraphKind::Command(Box::new(v.as_hir()))),
                ParagraphTerm::Uri(v) => terms.push(ParagraphKind::Uri(v.clone())),
                ParagraphTerm::DisplayMath(v) => terms.push(ParagraphKind::Code(Box::new(v.as_hir()))),
                ParagraphTerm::InlineMath(v) => terms.push(ParagraphKind::Code(Box::new(v.as_hir()))),
            }
        }
        ParagraphNode { terms, span: self.span.clone() }
    }
}
