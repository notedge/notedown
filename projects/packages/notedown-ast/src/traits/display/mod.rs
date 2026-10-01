mod link;
mod list;
mod quote;
mod table;
mod value;

use crate::{nodes::*, value::*};
use itertools::Itertools;
use std::fmt::{Debug, Display, Formatter, Write};

impl Display for NotedownKind {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match &self {
            NotedownKind::Statements(children) => {
                let s: Vec<_> = children.iter().map(|e| format!("{}", e)).collect();
                write!(f, "{}", s.join("\n\n"))
            }
            NotedownKind::Header { .. } => unimplemented!(),
            NotedownKind::Paragraph { .. } => unimplemented!(),
            NotedownKind::Delimiter(inner) => Display::fmt(inner, f),
            NotedownKind::TableView(inner) => Display::fmt(inner, f),
            NotedownKind::ListView(inner) => Display::fmt(inner, f),
            NotedownKind::TextSpan(inner) => Display::fmt(inner, f),
            NotedownKind::StyledSpan(inner) => Display::fmt(inner, f),
            NotedownKind::MathNode(inner) => Display::fmt(inner, f),
            NotedownKind::CodeNode(inner) => Display::fmt(inner, f),
            NotedownKind::LinkNode(inner) => Display::fmt(inner, f),
            NotedownKind::QuoteNode(inner) => Display::fmt(inner, f),
            NotedownKind::Command(inner) => Display::fmt(inner, f),
            NotedownKind::Value(inner) => Display::fmt(inner, f),
        }
    }
}

impl Display for Delimiter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HorizontalRule => f.write_str("---"),
            Self::HTMLRawBlock(s) => f.write_str(s),
        }
    }
}

impl Debug for TextSpan {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal(s) => Debug::fmt(s, f),
            Self::Raw(s) => Debug::fmt(s, f),
            Self::HTMLRawInline(s) => Debug::fmt(s, f),
            Self::Escaped(c) => {
                write!(f, "TextNode::Escaped({})", c)
            }
            Self::Emoji(emoji) => write!(f, "TextNode::Emoji({})", emoji),
            Self::SoftNewline => f.write_str("TextNode::SoftNewline"),
            Self::HardNewline => f.write_str("TextNode::HardNewline"),
            Self::CheckBox(b) => {
                let w = &mut f.debug_struct("TextNode::CheckBox");
                w.field("complete", b);
                w.finish()
            }
            Self::Empty => f.write_str("TextNode::Empty"),
        }
    }
}

impl Display for TextSpan {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal(_) => {
                unimplemented!()
            }
            Self::HTMLRawInline(_) => {
                unimplemented!()
            }
            Self::Escaped(c) => {
                f.write_char('\\')?;
                f.write_char(*c)
            }
            Self::Emoji(emoji) => f.write_str(emoji),
            Self::SoftNewline => f.write_char('\n'),
            Self::HardNewline => f.write_char('\n'),
            Self::CheckBox(b) => match b {
                true => f.write_str("[x]"),
                false => f.write_str("[ ]"),
            },
            Self::Empty => Ok(()),
            Self::Raw(_) => Ok(()),
        }
    }
}

impl Display for StyleNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.kind.surround_in())?;
        for child in &self.children {
            write!(f, "{}", child.value)?;
        }
        f.write_str(self.kind.surround_out())?;
        Ok(())
    }
}

impl Display for NotedownValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => f.write_str("null"),
            Self::Boolean(v) => f.write_str(&v.to_string()),
            Self::Integer(v) => f.write_str(&v.to_string()),
            Self::Decimal(v) => f.write_str(&v.to_string()),
            Self::String(_) => {
                unimplemented!()
            }
            Self::Set(_) => {
                unimplemented!()
            }
            Self::Array(_) => {
                unimplemented!()
            }
            Self::Object(_) => {
                unimplemented!()
            }
        }
    }
}
