use crate::{
    nodes::{NotedownKind, NotedownNode},
    traits::ContextAware,
    NotedownNodes,
};
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextKind {
    ///
    None,
    ///
    Text,
    ///
    Function,
    ///
    Math,
    ///
    Code,
}

impl ContextAware for NotedownNodes {
    fn context_aware(&self, offset: u32) -> ContextKind {
        for item in self {
            let e = item.value.context_aware(offset);
            if e != ContextKind::None {
                return e;
            }
        }
        return ContextKind::None;
    }
}

impl ContextAware for NotedownNode {
    fn context_aware(&self, offset: u32) -> ContextKind {
        self.value.context_aware(offset)
    }
}

impl ContextAware for NotedownKind {
    fn context_aware(&self, offset: u32) -> ContextKind {
        match self {
            Self::Statements(children) => children.context_aware(offset),
            Self::Header { .. } => unimplemented!(),
            Self::Paragraph(children) => children.context_aware(offset),
            Self::TableView { .. } => unimplemented!(),
            Self::ListView { .. } => unimplemented!(),
            Self::TextSpan(_) => unimplemented!(),
            Self::MathNode(_) => ContextKind::Math,
            Self::Command { .. } => unimplemented!(),
            Self::Value { .. } => unimplemented!(),
            Self::Delimiter(_) => unimplemented!(),
            Self::CodeNode(_) => unimplemented!(),
            Self::LinkNode(_) => unimplemented!(),
            Self::StyledSpan(_) => unimplemented!(),
            Self::QuoteNode(_) => unimplemented!(),
        }
    }
}
