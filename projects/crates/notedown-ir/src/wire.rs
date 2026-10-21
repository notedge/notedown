//! Versioned serialization envelope for `DocumentGraph`.

use serde::{Deserialize, Serialize};

use crate::graph::DocumentGraph;

/// Current on-disk / wire schema identifier.
pub const SCHEMA_VERSION: &str = "notedown-ir/v1";

/// Versioned document payload for stable round-trip and tooling probes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentEnvelope {
    /// Wire schema identifier for `document`.
    pub schema_version: String,
    /// Serialized document graph payload.
    pub document: DocumentGraph,
}

impl DocumentEnvelope {
    /// Wrap a graph with the current schema version.
    pub fn new(document: DocumentGraph) -> Self {
        Self { schema_version: SCHEMA_VERSION.to_string(), document }
    }

    /// Reject unknown major wire contracts before loading semantic data.
    pub fn ensure_supported(&self) -> Result<(), WireError> {
        if self.schema_version == SCHEMA_VERSION {
            Ok(())
        }
        else {
            Err(WireError::UnsupportedSchema { found: self.schema_version.clone(), expected: SCHEMA_VERSION.to_string() })
        }
    }
}

/// Wire-format contract failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    /// Schema version in the envelope is not supported by this build.
    UnsupportedSchema {
        /// Version string found in the payload.
        found: String,
        /// Version string expected by this build.
        expected: String,
    },
}
