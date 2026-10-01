#![doc = include_str!("readme.md")]

use std::{
    fmt::{Debug, Display, Formatter},
    hash::Hash,
    ops::RangeInclusive,
};

use diagnostic_quick::{error_3rd::NodeLocation, FileID, QError, QResult, Span};
use num::{Signed, Zero};
use serde::{Deserialize, Serialize};

use crate::{
    command::CommandArguments,
    traits::{IntoNotedown, Slugify},
    Command, NotedownValue,
};

pub use self::{
    elements::{code_block::*, delimiter::*, header::*, math::*, styled::*, text::*},
    link::{
        hyper_link::{HyperLink, HyperLinkTarget},
        image_link::{ImageLayout, ImageLink},
        other::EmailLink,
        rd::ResourceDescriptor,
        reference::TagReference,
        two_way::TwoWayLink,
        SmartLink,
    },
    list::{detailed::DetailedList, item::ListItem, ordered::OrderedList, orderless::OrderlessList, prefix::ListPrefixMark, ListView},
    quote::QuoteBlock,
    table::{SimpleTable, TableView},
};

mod codegen;
pub(crate) mod elements;
pub(crate) mod link;
pub(crate) mod list;
pub(crate) mod quote;
pub(crate) mod table;

/// Represents an AST object with position
pub type NotedownNode = NodeLocation<NotedownKind>;
/// Represents a list of AST objects with position
pub type NotedownNodes = Vec<NotedownNode>;

/// ## ASTKing
/// Typed info of the Node
///
/// - Block: Statements, Paragraph
/// - Span: Text, Styled
/// - Node: Code, Math, Link, Command
///
/// ### Notice
/// If a constructor returns individual elements, then interface accept what is
/// needed (`T`).
///
/// If a constructor returns [`NotedownNode`], then the interface implements
/// polymorphic input (`impl Into<T>`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotedownKind {
    /// Top Scope
    Statements(NotedownNodes),
    /// - block only
    Paragraph(NotedownNodes),
    /// - block only
    Delimiter(Box<Delimiter>),
    /// - block only
    Header(Box<Header>),
    /// - block only
    TableView(TableView),
    /// - block only
    ListView(ListView),
    /// - block only
    QuoteNode(Box<QuoteBlock>),
    /// - block + inline
    CodeNode(Box<CodeNode>),
    /// - block + inline
    MathNode(Box<MathNode>),
    /// - block + inline
    LinkNode(SmartLink),
    /// - inline only
    TextSpan(Box<TextSpan>),
    /// - inline only
    StyledSpan(Box<StyleNode>),
    /// - context sensitive
    Command(Command),
    /// - never bared
    Value(Box<NotedownValue>),
}

impl Default for NotedownKind {
    fn default() -> Self {
        Self::Value(Box::new(NotedownValue::Null))
    }
}

impl NotedownKind {
    /// Constructor of [`NotedownKind::Statements`]
    #[inline]
    pub fn statements(children: NotedownNodes, span: &Span, file: &FileID) -> NotedownNode {
        Self::Statements(children).into_node(span, file)
    }
    /// Constructor of [`NotedownKind::Paragraph`]
    #[inline]
    pub fn paragraph(children: NotedownNodes, span: &Span, file: &FileID) -> NotedownNode {
        Self::Paragraph(children).into_node(span, file)
    }
    /// Constructor of [`Delimiter::HorizontalRule`]
    #[inline]
    pub fn hr(span: &Span, file: &FileID) -> NotedownNode {
        Delimiter::HorizontalRule.into_node(span, file)
    }
}
