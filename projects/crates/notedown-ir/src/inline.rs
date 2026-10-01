use serde::{Deserialize, Serialize};

use crate::id::NodeId;

/// Inline semantic content inside a block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Inline {
    Text { text: String },
    Styled {
        style: String,
        children: Vec<Inline>,
    },
    InlineCode { text: String },
    InlineMath { content: String, language: Option<String> },
    Reference { target: NodeId, display: String },
}
