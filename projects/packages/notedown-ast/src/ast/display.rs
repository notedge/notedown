use super::*;

impl Debug for NotedownTerm {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NotedownTerm::Heading(v) => Debug::fmt(v, f),
            NotedownTerm::Paragraph(v) => Debug::fmt(v, f),
            NotedownTerm::SpaceBreak(v) => Debug::fmt(v, f),
            NotedownTerm::MathBlock(v) => Debug::fmt(v, f),
        }
    }
}

impl Display for NotedownAST {
    fn fmt(&self, _f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl Display for NotedownTerm {
    fn fmt(&self, _f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl Debug for ParagraphTerm {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParagraphTerm::Text(v) => Debug::fmt(v, f),
            ParagraphTerm::WhiteSpace(v) => Debug::fmt(v, f),
            ParagraphTerm::NewLine(v) => Debug::fmt(v, f),
            ParagraphTerm::Comma(v) => Debug::fmt(v, f),
            ParagraphTerm::Period(v) => Debug::fmt(v, f),
            ParagraphTerm::Escape(v) => Debug::fmt(v, f),
            ParagraphTerm::Italic(v) => Debug::fmt(v, f),
            ParagraphTerm::Bold(v) => Debug::fmt(v, f),
            ParagraphTerm::BoldItalic(v) => Debug::fmt(v, f),
            ParagraphTerm::Underline(v) => Debug::fmt(v, f),
            ParagraphTerm::Delete(v) => Debug::fmt(v, f),
            ParagraphTerm::Code(v) => Debug::fmt(v, f),
            ParagraphTerm::CommandLine(v) => Debug::fmt(v, f),
            ParagraphTerm::Uri(v) => Debug::fmt(v, f),
            ParagraphTerm::DisplayMath(v) => Debug::fmt(v, f),
            ParagraphTerm::InlineMath(v) => Debug::fmt(v, f),
        }
    }
}
