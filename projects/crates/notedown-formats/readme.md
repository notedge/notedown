# notedown-formats

Document import and export around `notedown-ir`. Format parsing and semantic
lowering live here, not in Panduck.

## Modules

| Format   | Import                                            | Export                             | Feature    |
|----------|---------------------------------------------------|------------------------------------|------------|
| `notedown` | `oak-notedown` → IR (`Partial`)                 | — (not wired)                      | `notedown` |
| `markdown` | `oak-markdown` → IR (`Partial`)                 | IR → Markdown / Markdown project (`Partial`) | `markdown` |
| `html`     | `oak-html` → IR (`Partial`)                      | IR → HTML (`Partial`)              | `html` (import only) |
| `docx`     | `acorn-docx` + WordprocessingML → IR (`Partial`) | IR → DOCX (`Partial`)              | `docx`     |
| `doc`      | `acorn-doc` OLE plaintext → IR (`Partial`)       | — (`Unavailable`, legacy binary Word not wired) | `doc` |
| `pdf`      | Acorn PDF object model → IR (`Partial`)          | IR → PDF 1.4 (`Partial`)           | `pdf`      |
| `epub`     | `acorn-epub` OCF + XHTML → IR (`Partial`)        | — (not wired)                      | `epub`     |
| `tex`      | `oak-tex` → IR (`Partial`)                       | — (not wired)                      | `tex`      |
| `typst`    | `oak-typst` → IR (`Partial`)                     | — (not wired)                      | `typst`    |

Status values follow `notedown_formats::FormatStatus`: `Ready`, `Partial`,
`Planned`, `Unavailable`. `capabilities()` is the canonical registry and is
asserted by the `capability` integration test.

## Usage

```rust
use notedown_formats::import::markdown::import_markdown_bytes;
use notedown_formats::export::markdown::export_markdown;

let graph = import_markdown_bytes("note.md", "# Title\n\nBody.")?;
let markdown = export_markdown(&graph)?;
```

Panduck calls these modules through `panduck-convert` orchestration only.
