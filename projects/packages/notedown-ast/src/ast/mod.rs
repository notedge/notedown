pub mod code;
pub mod command;
pub mod display;
pub mod escaped;
pub mod math;
pub mod paragraph;
pub mod punctuation;
pub mod style;
pub mod title;
pub mod whitespace;

pub use self::{
    code::CodeInlineSpan,
    command::CommandLineSpan,
    escaped::EscapedCommand,
    math::{DisplayMathSpan, InlineMathSpan},
    paragraph::{ParagraphSpan, ParagraphTerm},
    punctuation::{CommaNode, PeriodNode},
    style::{FontBoldItalicSpan, FontBoldSpan, FontDeleteSpan, FontItalicSpan, FontUnderlineSpan},
    title::HeadingSpan,
    whitespace::{HSpaceNode, IgnoreNode, NewlineSpan, ParagraphBreakSpan, TextSpaceNode, VSpaceNode},
};
pub use crate::hir::TextPlainNode;
use crate::hir::{
    CodeNode, CommandNode, HeadingLevel, HeadingNode, IdentifierNode, NotedownHIR, ParagraphKind, ParagraphNode, TextStyleNode,
    UriNode,
};
use deriver::From;
use notedown_error::Url;
use std::{
    fmt::{Debug, Display, Formatter, Write},
    ops::Range,
};

/// The root node of all notedown terms
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NotedownAST {
    pub terms: Vec<NotedownTerm>,
    pub path: Option<Url>,
}

#[derive(Clone, Eq, PartialEq, Hash, From)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NotedownTerm {
    Heading(Box<HeadingSpan>),
    Paragraph(Box<ParagraphSpan>),
    SpaceBreak(Box<ParagraphBreakSpan>),
    MathBlock(Box<DisplayMathSpan>),
}

impl NotedownAST {
    pub fn add_error<T: ToString>(&mut self, error: T) {
        self.terms.push(NotedownTerm::Paragraph(Box::new(ParagraphSpan {
            terms: vec![ParagraphTerm::Text(Box::new(TextPlainNode { text: error.to_string(), span: 0..0 }))],
            span: 0..0,
        })));
    }
    pub fn as_hir(&self) -> NotedownHIR {
        let mut terms = Vec::with_capacity(self.terms.len());
        for term in &self.terms {
            match term {
                NotedownTerm::Heading(v) => terms.push(v.as_hir().into()),
                NotedownTerm::Paragraph(v) => terms.push(v.as_hir().into()),
                NotedownTerm::SpaceBreak(_) => continue,
                NotedownTerm::MathBlock(v) => {
                    // Convert math block to paragraph with code
                    let code_node = v.as_hir();
                    let paragraph = ParagraphNode {
                        terms: vec![ParagraphKind::Code(Box::new(code_node))],
                        span: v.span.clone(),
                    };
                    terms.push(paragraph.into());
                }
            }
        }
        NotedownHIR { node: terms, url: self.path.clone() }
    }
}
