# notedown-formats

Document import and export around `notedown-ir`. Format parsing and semantic lowering live here, not in Panduck.

## Modules

| Format | Import | Export |
|--------|--------|--------|
| `notedown` | `oak-notedown` → IR | planned |
| `markdown` | `oak-markdown` → IR | IR → Markdown |
| `docx` | `acorn-docx` + WordprocessingML → IR | IR → DOCX |
| `epub` | `acorn-epub` + XHTML → IR | planned |

## Usage

```rust
use notedown_formats::import::notedown::import_notedown_bytes;
use notedown_formats::import::markdown::import_markdown_bytes;
use notedown_formats::export::markdown::export_markdown;

let graph = import_markdown_bytes("note.md", "# Title\n\nBody.")?;
let markdown = export_markdown(&graph)?;
```

Panduck calls these modules through `panduck-convert` orchestration only.
