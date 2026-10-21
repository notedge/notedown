//! Inline semantic content and its supporting value objects.

use serde::{Deserialize, Serialize};

use crate::id::{AssetId, NodeId};
use crate::relation::RelationEndpoint;

/// Inline semantic content inside a block.
///
/// Variants express document semantics, not parser syntax: a link keeps its
/// destination separate from its display content, and a media reference keeps
/// its logical identity separate from its storage location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Inline {
    Text {
        text: String,
    },
    Styled {
        style: String,
        children: Vec<Inline>,
    },
    InlineCode {
        text: String,
    },
    InlineMath {
        content: String,
        language: Option<String>,
    },
    Reference {
        target: NodeId,
        display: String,
    },
    /// Authored hyperlink. Display content is independent from the destination,
    /// and an unresolved destination stays a real relation rather than a gap.
    Link {
        target: RelationEndpoint,
        display: Vec<Inline>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    /// Authored media reference. `source` records the requested location while
    /// `asset` binds to the registered asset identity, which survives renames.
    Image {
        source: String,
        alt: Vec<Inline>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        asset: Option<AssetId>,
    },
    /// Note (footnote/endnote) reference, addressed by label.
    Note {
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        display: Option<Vec<Inline>>,
    },
    /// Citation reference, addressed by citation key.
    Citation {
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        display: Option<Vec<Inline>>,
    },
    /// Explicit inline break, distinct from block-level thematic breaks.
    Break {
        kind: BreakKind,
    },
    /// Attributes attached to the wrapped inline content.
    Attributed {
        attributes: Attributes,
        children: Vec<Inline>,
    },
}

/// Kind of explicit inline break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BreakKind {
    /// A line break that renders as whitespace.
    Soft,
    /// A line break that renders as a forced line ending.
    Hard,
}

/// Identity, class, and key/value attributes of a semantic object.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attributes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pairs: Vec<AttributePair>,
}

/// Single attribute key/value pair, ordered to keep serialization stable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributePair {
    pub key: String,
    pub value: String,
}

impl Attributes {
    /// Whether no identity, class, or key/value attribute is present.
    pub fn is_empty(&self) -> bool {
        self.id.is_none() && self.classes.is_empty() && self.pairs.is_empty()
    }
}

impl Inline {
    /// Flattened text content for consumers that can only render text.
    ///
    /// This is not a semantic projection: style, target identity, asset binding,
    /// and attributes are dropped. Formats that understand the node must read
    /// its fields directly instead of relying on this projection.
    pub fn plain_text(&self) -> String {
        match self {
            Self::Text { text } | Self::InlineCode { text } => text.clone(),
            Self::Styled { children, .. } | Self::Attributed { children, .. } => children.iter().map(Self::plain_text).collect(),
            Self::InlineMath { content, .. } => content.clone(),
            Self::Reference { display, .. } => display.clone(),
            Self::Link { display, .. } => display.iter().map(Self::plain_text).collect(),
            Self::Image { alt, .. } => alt.iter().map(Self::plain_text).collect(),
            Self::Note { label, display } => match display {
                Some(display) => display.iter().map(Self::plain_text).collect(),
                None => format!("[^{label}]"),
            },
            Self::Citation { key, display } => match display {
                Some(display) => display.iter().map(Self::plain_text).collect(),
                None => format!("[@{key}]"),
            },
            Self::Break { kind } => match kind {
                BreakKind::Soft => " ".to_string(),
                BreakKind::Hard => "\n".to_string(),
            },
        }
    }
}
