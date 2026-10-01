//! Basic usage example for Notedown parser

use notedown_parser::helpers::ParseState;
use notedown_parser::parsers::*;

fn main() {
    // Example Notedown content
    let content = r#"
# Hello World

This is a **bold** text with some *italic* content.

```rust
fn main() {
    println!("Hello, world!");
}
```

- Item 1
- Item 2
- Item 3

> This is a blockquote
> with multiple lines

| Column 1 | Column 2 |
|----------|----------|
| Cell 1   | Cell 2   |
"#;

    // Create parse state
    let mut state = ParseState::new(content.to_string());
    
    // Parse the document
    match parse_document(&mut state) {
        Ok(nodes) => {
            println!("Successfully parsed {} nodes", nodes.len());
            for (i, node) in nodes.iter().enumerate() {
                println!("Node {}: {:?}", i + 1, node.kind());
            }
        }
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
        }
    }
}