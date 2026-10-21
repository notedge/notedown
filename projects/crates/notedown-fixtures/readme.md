# notedown-fixtures

Downstream Oak language conformance fixtures for Notedown and Panduck.

Heavy lexer, parser, and diagnostic golden tests for `oak-markdown`, `oak-notedown`, `oak-html`, `oak-xml`, and
`oak-org-mode` live here so upstream `oak.rs` CI stays fast.

## Layout

```text
tests/
  acorn/doc/  # acorn-doc legacy .doc + notedown-formats doc probes
  markdown/   # oak-markdown
  notedown/   # oak-notedown
  html/       # oak-html
  xml/        # oak-xml
  org-mode/   # oak-org-mode
src/
  lib.rs      # `include_bytes!` exports for Panduck and other downstream crates
```

## Upstream smoke-only contract

| Upstream crate | Upstream tests             | Downstream fixtures |
|----------------|----------------------------|---------------------|
| `oak-markdown` | `tests/main.rs` smoke only | `tests/markdown/`   |
| `oak-notedown` | `tests/main.rs` smoke only | `tests/notedown/`   |
| `acorn-doc`    | `tests/main.rs` smoke only | `tests/acorn/doc/`  |

Checked-in binary fixtures and cross-format golden probes **must not** live in `oak.rs` or `acorn.rs`. Downstream crates
(`panduck-convert`, `notedown-formats` integration) consume bytes via `notedown_fixtures::…` (`include_bytes!` in
`src/lib.rs`), not crate-local `tests/fixtures/`.

`acorn-cfb` may keep in-memory oracle tests (programmatic bytes, no checked-in files). Anything that needs a persisted
`.doc`/`.docx`/`.epub` sample belongs here.

## Run

```bash
cargo test -p notedown-fixtures
```
