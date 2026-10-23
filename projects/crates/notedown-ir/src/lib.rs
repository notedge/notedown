#![deny(missing_docs)]
//! Notedown document semantic IR.
//!
//! Stable identities, block semantics, relations, assets, and provenance.
//! Independent from Oak AST, text parsers, Acorn binary views, and Panduck
//! writers.

mod asset;
mod block;
mod graph;
mod id;
mod inline;
mod relation;
mod source;
mod status;
mod transform_errors;
mod validate;
mod walk;
mod wire;

pub use asset::{Asset, AssetKind};
pub use block::{Block, BlockNode, ListItem, TableCell, TableRow};
pub use graph::{DocumentGraph, DocumentMetadata, IdAllocator};
pub use id::{AssetId, DocumentId, LinkId, NodeId};
pub use inline::{AttributePair, Attributes, BreakKind, Inline};
pub use relation::{Relation, RelationEndpoint, RelationKind};
pub use source::{CoverageReport, SourceKind, SourceRef};
pub use status::{LossMarker, SemanticStatus};
pub use transform_errors::{Diagnostic, RewriteResult, Traversal, TransformError};
pub use validate::{ValidationIssue, ValidationReport};
pub use wire::{DocumentEnvelope, SCHEMA_VERSION, WireError};
