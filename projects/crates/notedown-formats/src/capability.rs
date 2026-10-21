/// Whether a format module reads or writes documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatDirection {
    Import,
    Export,
}

/// Implementation readiness for a format module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatStatus {
    Ready,
    Partial,
    Planned,
    Unavailable,
}

/// One import or export capability entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatCapability {
    pub id: &'static str,
    pub direction: FormatDirection,
    pub status: FormatStatus,
}
