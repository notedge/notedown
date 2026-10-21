#![deny(missing_docs)]

//! Embedded conformance fixtures for Notedown, Panduck, and downstream Acorn
//! probes.

/// Acorn binary format fixtures.
pub mod acorn {
    /// Legacy Word `.doc` fixtures.
    pub mod doc {
        /// Minimal plaintext legacy `.doc` compound file.
        pub const LEGACY_PLAIN_DOC: &[u8] = include_bytes!("../tests/acorn/doc/legacy_plain.doc");
    }
}
