Notedown Core with Rust
=========================

Document semantic IR and format import/export for the R-7 stack.

### Active crates

| Crate | Role |
|-------|------|
| `notedown-ir` | Document graph, relations, assets, wire `notedown-ir/v1`, validation |
| `notedown-formats` | Markdown, DOCX, EPUB import/export via Oak and Acorn |

Notedown text syntax lives in `oak-notedown` (Oaks). Panduck orchestrates conversion on top of `notedown-ir`.

### Verify

```bash
cargo test -p notedown-ir -p notedown-formats
```
