/// Whether a format module reads or writes documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatDirection {
    /// Import external bytes into `notedown-ir`.
    Import,
    /// Export `notedown-ir` into external bytes.
    Export,
}

/// Implementation readiness for a format module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatStatus {
    /// Production-ready for the advertised surface.
    Ready,
    /// Usable with known semantic gaps.
    Partial,
    /// Scaffolded but not yet wired.
    Planned,
    /// Explicitly not available (must not be routed).
    Unavailable,
}

/// One import or export capability entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatCapability {
    /// Stable format identifier such as `markdown` or `docx`.
    pub id: &'static str,
    /// Whether this entry describes import or export.
    pub direction: FormatDirection,
    /// Implementation readiness for the advertised direction.
    pub status: FormatStatus,
}
