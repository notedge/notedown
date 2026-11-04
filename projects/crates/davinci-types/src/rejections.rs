//! The authoritative rejection catalog shared by every DaVinci entry.
//!
//! The catalog lives in `catalog/rejections.json` and is compiled into this
//! crate, so the library entry, the Panduck N-API entry and the VuTeX entry all
//! read the same file instead of each keeping its own expected values.
//!
//! A catalog row is a promise: one authored source, one code, one severity, and
//! one span that every entry must report at the same authored byte offsets.

use serde::Deserialize;

/// The rejection catalog shipped with this crate.
pub const CATALOG_JSON: &str = include_str!("../catalog/rejections.json");

/// One diagnostic code and what it means on the authoring surface.
#[derive(Clone, Debug, Deserialize)]
pub struct CodeDoc {
    /// The stable dotted code, for example `davinci.chart.unsupported-mark`.
    pub code: String,
    /// `error` or `warning`.
    pub severity: String,
    /// What the author did wrong, in author-facing words.
    pub meaning: String,
    /// The construct that triggers it.
    pub trigger: String,
    /// What the reported span covers.
    pub span: String,
}

/// One illegal authored source and the rejection every entry must report.
#[derive(Clone, Debug, Deserialize)]
pub struct RejectionCase {
    /// Stable case name used by every entry's test output.
    pub id: String,
    /// Fence language: `vega-lite`, `mermaid`, `dot`, or `plantuml`.
    pub language: String,
    /// The authored bytes handed to every entry, unchanged.
    pub source: String,
    /// The expected diagnostic code.
    pub code: String,
    /// The expected diagnostic severity, `error` or `warning`.
    pub severity: String,
    /// The exact authored slice the span must cover.
    pub span_text: String,
}

/// One row of the published capability matrix.
///
/// A rejected row is a promise: it names the code the entry reports and either
/// the catalog case that exercises it or the reason no case exists. Supported
/// and approximated rows carry no rejection evidence.
#[derive(Clone, Debug, Deserialize)]
pub struct MatrixRow {
    /// Stable row id, `chart-<n>` or `diagram-<n>`.
    pub id: String,
    /// Owning component, `chart` or `diagram`.
    pub area: String,
    /// Short author-facing description of the row.
    pub item: String,
    /// One of `supported`, `approximated`, or `rejected`.
    pub status: String,
    /// The diagnostic code a degraded row reports. A rejected row must name
    /// one, an approximated row may, and a supported row must not.
    #[serde(default)]
    pub code: Option<String>,
    /// Catalog cases that exercise a rejected row, or that prove a supported
    /// row refuses malformed values inside it.
    #[serde(default)]
    pub cases: Vec<String>,
    /// The named reason a degraded row has no catalog case, such as the test
    /// that pins its span when the behavior is a warning rather than a
    /// rejection.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Deserialize)]
struct Catalog {
    codes: Vec<CodeDoc>,
    cases: Vec<RejectionCase>,
    matrix: Vec<MatrixRow>,
}

fn catalog() -> Catalog {
    serde_json::from_str(CATALOG_JSON).expect("the shipped rejection catalog is valid JSON")
}

/// Every documented code, in catalog order.
pub fn code_docs() -> Vec<CodeDoc> {
    catalog().codes
}

/// Every rejection case, in catalog order.
pub fn cases() -> Vec<RejectionCase> {
    catalog().cases
}

/// The cases for one fence language, in catalog order.
pub fn cases_for(language: &str) -> Vec<RejectionCase> {
    cases().into_iter().filter(|case| case.language == language).collect()
}

/// Every capability matrix row, in catalog order.
pub fn matrix() -> Vec<MatrixRow> {
    catalog().matrix
}
