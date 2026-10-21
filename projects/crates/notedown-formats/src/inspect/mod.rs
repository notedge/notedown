//! Format-neutral container inspection.
//!
//! Inspection reports container and member summaries without constructing a
//! document graph. Callers can request member decoding, but the result never
//! exposes parser, archive, or budget types from the underlying format crates.

#[cfg(feature = "docx")]
use crate::FormatError;

/// Limits applied to one inspection request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InspectBudget {
    /// Maximum bytes materialized while decoding container members.
    pub max_decoded_bytes: u64,
    /// Maximum container members indexed in one request.
    pub max_container_members: u64,
}

impl Default for InspectBudget {
    fn default() -> Self {
        Self { max_decoded_bytes: 16 * 1024 * 1024, max_container_members: 100_000 }
    }
}

/// One container member summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectedPart {
    /// Member path inside the container.
    pub path: String,
    /// Compression method identifier declared by the container.
    pub compression_method: u16,
    /// Compressed byte length stored in the container.
    pub compressed_size: u64,
    /// Uncompressed byte length declared by the container.
    pub uncompressed_size: u64,
    /// Decoded payload byte length when decoding was requested.
    pub decoded_size: Option<u64>,
}

/// Container inspection result for one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentInspect {
    /// Detected document format identifier.
    pub format: String,
    /// Outer container label.
    pub outer: String,
    /// Inner packaging label.
    pub inner: String,
    /// Member summaries in container order.
    pub parts: Vec<InspectedPart>,
}

/// Inspects a DOCX OPC package without building a document graph.
///
/// `part_filter` restricts the request to one member path. When `decode` is
/// false, only container metadata is reported and no member payload is
/// materialized. Decoding settles against one shared [`InspectBudget`].
#[cfg(feature = "docx")]
pub fn inspect_docx_bytes(
    label: &str,
    bytes: &[u8],
    part_filter: Option<&str>,
    decode: bool,
    budget: InspectBudget,
) -> Result<DocumentInspect, FormatError> {
    use acorn_core::{Budget, BudgetTracker};
    use acorn_docx::{OpcError, OpcPackage};

    if looks_like_ole(bytes) {
        return Err(FormatError::unsupported("docx", "legacy .doc OLE inspection"));
    }
    if !looks_like_zip(bytes) {
        return Err(FormatError::invalid_input("input is not a ZIP-based DOCX package"));
    }

    let package =
        OpcPackage::open(label.to_string(), bytes.to_vec()).map_err(map_docx_error)?;
    let paths = match part_filter {
        Some(path) => vec![acorn_docx::normalize_part_path(path)],
        None => package.part_paths(),
    };
    if paths.len() as u64 > budget.max_container_members {
        return Err(FormatError::budget("docx", "container member budget exhausted"));
    }

    let limits = Budget {
        max_decoded_bytes: budget.max_decoded_bytes,
        max_container_members: budget.max_container_members,
        ..Budget::default()
    };
    let mut ledger = BudgetTracker::new(limits);
    let mut decoded_total = 0u64;
    let mut parts = Vec::with_capacity(paths.len());
    for path in paths {
        let member = package.part(&path).ok_or_else(|| map_docx_error(OpcError::PartNotFound(path.clone())))?;
        let decoded_size = if decode {
            decoded_total = decoded_total.saturating_add(member.uncompressed_size);
            if decoded_total > budget.max_decoded_bytes {
                return Err(FormatError::budget("docx", "decoded byte budget exhausted"));
            }
            let decoded = package.read_part(&path, &mut ledger).map_err(map_docx_error)?;
            Some(decoded.len() as u64)
        }
        else {
            None
        };
        parts.push(InspectedPart {
            path,
            compression_method: member.compression_method,
            compressed_size: member.compressed_size,
            uncompressed_size: member.uncompressed_size,
            decoded_size,
        });
    }

    Ok(DocumentInspect { format: "docx".into(), outer: "zip".into(), inner: "opc".into(), parts })
}

#[cfg(feature = "docx")]
fn map_docx_error(error: acorn_docx::OpcError) -> FormatError {
    FormatError::parse("docx", error.to_string())
}

#[cfg(feature = "docx")]
fn looks_like_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")
}

#[cfg(feature = "docx")]
fn looks_like_ole(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1])
}
