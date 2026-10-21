# notedown-ir

Notedown document semantic IR (`R-7.0`). Owns stable identities, content blocks, relations, assets, provenance, and explicit semantic status.

## Boundaries

- Does **not** depend on text parsers, `oak-notedown`, Acorn format views, or Panduck.
- Oak AST, Acorn layout views, and Panduck orchestration adapt **into** this crate.
- `SemanticStatus`, `CoverageReport`, and `LossMarker` live here. Unified `rust-boost/diagnostic` is for cross-layer reporting during import/export, not a dependency of this crate.

## Wire format

Serialize with [`DocumentEnvelope`](src/wire.rs) (`notedown-ir/v1`). Call `validate()` after load before treating a graph as production-safe.

## Acceptance

Integration tests in `tests/acceptance.rs` mirror Living `规划设计/notedown/00-文档语义IR独立合同.md`.
