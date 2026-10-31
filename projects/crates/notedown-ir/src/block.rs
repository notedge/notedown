use serde::{Deserialize, Serialize};

use crate::{id::NodeId, inline::Inline, math::MathFormula, status::SemanticStatus};

/// Top-level block semantics (not parser syntax nodes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Block {
    /// Section heading with nested block children.
    Section {
        /// Heading level (1 is top-level).
        level: u8,
        /// Inline title content.
        title: Vec<Inline>,
        /// Child block node ids owned by this section.
        children: Vec<NodeId>,
    },
    /// Paragraph of inline content.
    Paragraph {
        /// Inline body.
        content: Vec<Inline>,
    },
    /// Ordered or unordered list.
    List {
        /// Whether list markers are numeric.
        ordered: bool,
        /// Top-level list items.
        items: Vec<ListItem>,
    },
    /// Table rows of inline cell content.
    Table {
        /// Optional human-readable caption, such as "表 1：总体准确率".
        caption: Option<String>,
        /// Table header and body rows.
        rows: Vec<TableRow>,
    },
    /// Block quotation.
    Quote {
        /// Quoted inline content.
        content: Vec<Inline>,
    },
    /// Fenced or indented code block.
    Code {
        /// Optional language tag.
        language: Option<String>,
        /// Source text.
        content: String,
    },
    /// Display math block.
    ///
    /// The formula carries the authored source and the semantic expression the
    /// lowering profile produced. Consumers render the expression; they never
    /// parse the source again.
    Math {
        /// The display formula.
        formula: MathFormula,
    },
    /// Authored diagram source with a declared diagram language.
    ///
    /// The IR stores only the raw source. Lowering it into a render scene and
    /// registering that scene belongs to a diagram subsystem, so neither render
    /// results nor geometry are duplicated here.
    Diagram {
        /// Diagram language tag such as `mermaid`.
        language: String,
        /// Raw diagram source exactly as authored inside the fence.
        source: String,
    },
    /// Format-specific payload retained without pretending full semantics.
    Opaque {
        /// Opaque payload kind identifier.
        kind: String,
        /// Human- or tool-readable payload hint.
        payload_hint: String,
        /// Confidence of the retained payload.
        status: SemanticStatus,
    },
    /// Raw HTML block from Markdown source — kept verbatim, not parsed.
    RawHtml {
        /// Verbatim HTML block as authored.
        html: String,
    },
}

/// Single list item with optional nested blocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListItem {
    /// Inline content for the list marker line.
    pub content: Vec<Inline>,
    /// Nested child blocks under this item.
    pub children: Vec<NodeId>,
    /// Explicit list number when the parent list is ordered.
    ///
    /// Unordered lists must keep this `None`. Ordered lists that want the
    /// numbering preserved across formats, after edits, or for cross-format
    /// round-trips must populate each item with a non-zero number. Numbers do
    /// not need to start from 1 but must be strictly increasing within the
    /// parent list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    /// Whether this item is a task-list item.
    ///
    /// `true` means the original Markdown had a `- [x]` / `- [ ]` marker.
    /// `false` means a plain list item — `is_checked` stays `None`.
    #[serde(default)]
    pub is_task: bool,
    /// Checked state of a task-list item.
    ///
    /// Only meaningful when `is_task` is `true`. `Some(true)` for `[x]` /
    /// `[X]`, `Some(false)` for `[ ]`, `None` for non-task items.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_checked: Option<bool>,
}

impl Default for ListItem {
    fn default() -> Self {
        Self { content: Vec::new(), children: Vec::new(), number: None, is_task: false, is_checked: None }
    }
}

/// One table row of cells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRow {
    /// Whether this row is a header row (`<thead>` / `<th>` 行).
    pub is_header: bool,
    /// Row cells, each containing inline content and optional span semantics.
    pub cells: Vec<TableCell>,
}

/// One table cell with optional column/row span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    /// Inline body.
    pub content: Vec<Inline>,
    /// Number of columns this cell occupies. Must be at least 1.
    pub colspan: u32,
    /// Number of rows this cell occupies. Must be at least 1.
    pub rowspan: u32,
}

/// Stored block node with identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockNode {
    /// Semantic node identity.
    pub id: NodeId,
    /// Block payload.
    pub block: Block,
}
