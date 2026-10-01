use super::*;
use diagnostic_quick::{FileID, Span};

/// Serialized external data
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExternalCommand {
    /// Name of the atomics
    pub cmd: String,
    /// Value of the atomics
    pub data: Vec<u8>,
}

impl ExternalCommand {
    /// Create a new external atomics
    #[inline]
    pub fn new(cmd: String, data: Vec<u8>) -> Self {
        Self { cmd, data }
    }
}
