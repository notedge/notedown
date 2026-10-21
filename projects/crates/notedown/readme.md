# notedown

Single entry point for the Notedown document IR and format import/export family. Each enabled feature opens a
`notedown::…` module that re-exports one workspace crate — same types, no duplicate wrappers.

Product overview: [repository README](../../../readme.md).

## Features

Default features are **empty**. Enable only what you need:

| Feature    | Module              | Underlying crate                        |
|------------|---------------------|-----------------------------------------|
| `ir`       | `notedown::ir`      | `notedown-ir`                           |
| `formats`  | `notedown::formats` | `notedown-formats` (no format features) |
| `markdown` | `notedown::formats` | `notedown-formats` + `markdown`         |
| `notedown` | `notedown::formats` | `notedown-formats` + `notedown`         |
| `docx`     | `notedown::formats` | `notedown-formats` + `docx`             |
| `doc`      | `notedown::formats` | `notedown-formats` + `doc`              |
| `pdf`      | `notedown::formats` | `notedown-formats` + `pdf`              |
| `epub`     | `notedown::formats` | `notedown-formats` + `epub`             |
| `html`     | `notedown::formats` | `notedown-formats` + `html`             |

`formats` and every format feature also enable `ir`.

## Example

```toml
[dependencies]
notedown = { version = "0.1", default-features = false, features = ["ir", "markdown", "docx"] }
```

```rust
use notedown::ir::DocumentGraph;
use notedown::formats::import::markdown;
```
