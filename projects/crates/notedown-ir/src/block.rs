use serde::{Deserialize, Serialize};

use crate::id::NodeId;
use crate::inline::Inline;
use crate::status::SemanticStatus;

/// Top-level block semantics (not parser syntax nodes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Block {
    Section {
        level: u8,
        title: Vec<Inline>,
        children: Vec<NodeId>,
    },
    Paragraph {
        content: Vec<Inline>,
    },
    List {
        ordered: bool,
        items: Vec<ListItem>,
    },
    Table {
        rows: Vec<TableRow>,
    },
    Quote {
        content: Vec<Inline>,
    },
    Code {
        language: Option<String>,
        content: String,
    },
    Math {
        content: String,
        language: Option<String>,
    },
    Opaque {
        kind: String,
        payload_hint: String,
        status: SemanticStatus,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListItem {
    pub content: Vec<Inline>,
    pub children: Vec<NodeId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRow {
    pub cells: Vec<Vec<Inline>>,
}

/// Stored block node with identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockNode {
    pub id: NodeId,
    pub block: Block,
}
