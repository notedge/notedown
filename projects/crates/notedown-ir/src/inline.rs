//! Inline semantic content and its supporting value objects.

use serde::{Deserialize, Serialize};

use crate::{
    id::{AssetId, NodeId},
    relation::RelationEndpoint,
};

/// Inline semantic content inside a block.
///
/// Variants express document semantics, not parser syntax: a link keeps its
/// destination separate from its display content, and a media reference keeps
/// its logical identity separate from its storage location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Inline {
    /// Plain text run.
    Text {
        /// Text payload.
        text: String,
    },
    /// Styled inline span.
    Styled {
        /// Style name or class.
        style: String,
        /// Nested inline content receiving the style.
        children: Vec<Inline>,
    },
    /// Inline code span.
    InlineCode {
        /// Code text.
        text: String,
    },
    /// Inline math span.
    InlineMath {
        /// Math source text.
        content: String,
        /// Optional language tag such as `latex`.
        language: Option<String>,
    },
    /// Intra-document reference by node id.
    Reference {
        /// Referenced node id.
        target: NodeId,
        /// Authored display text.
        display: String,
    },
    /// Authored hyperlink. Display content is independent from the destination,
    /// and an unresolved destination stays a real relation rather than a gap.
    Link {
        /// Link destination endpoint.
        target: RelationEndpoint,
        /// Visible link content.
        display: Vec<Inline>,
        /// Optional title attribute.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    /// Authored media reference. `source` records the requested location while
    /// `asset` binds to the registered asset identity, which survives renames.
    Image {
        /// Requested source location or URI.
        source: String,
        /// Alternative text content.
        alt: Vec<Inline>,
        /// Optional title attribute.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// Bound asset identity when import registered the media.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        asset: Option<AssetId>,
    },
    /// Note (footnote/endnote) reference, addressed by label.
    Note {
        /// Note label.
        label: String,
        /// Optional authored display content.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        display: Option<Vec<Inline>>,
    },
    /// Citation reference, addressed by citation key.
    Citation {
        /// Citation key.
        key: String,
        /// Optional authored display content.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        display: Option<Vec<Inline>>,
    },
    /// Explicit inline break, distinct from block-level thematic breaks.
    Break {
        /// Break semantics.
        kind: BreakKind,
    },
    /// Attributes attached to the wrapped inline content.
    Attributed {
        /// Identity, class, and key/value attributes.
        attributes: Attributes,
        /// Wrapped inline content.
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
    /// Optional DOM-style id attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Class names applied to the object.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    /// Additional key/value attributes in stable order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pairs: Vec<AttributePair>,
}

/// Single attribute key/value pair, ordered to keep serialization stable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributePair {
    /// Attribute name.
    pub key: String,
    /// Attribute value.
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
    /// This is not a semantic projection: style, target identity, asset
    /// binding, and attributes are dropped. Formats that understand the
    /// node must read its fields directly instead of relying on this
    /// projection.
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
