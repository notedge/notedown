//! Integration tests for complete Notedown document parsing

use notedown_parser::helpers::{NotedownNode, ParseState};
use notedown_parser::parsers::*;
use notedown_error::NoteError;

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_parse_complete_document() {
        let input = r#"# Main Title

This is a **bold** paragraph with *italic* text and `inline code`.

## Subsection

> This is a blockquote
> with multiple lines.

- List item 1
- List item 2
  - Nested item

1. Ordered item
2. Another ordered item

```rust
fn main() {
    println!("Hello, world!");
}
```

$$
\int_0^1 x^2 dx = \frac{1}{3}
$$

| Header 1 | Header 2 |
|----------|----------|
| Cell 1   | Cell 2   |

[Link text](https://example.com)

---

End of document."#;

        let mut state = ParseState::new(input);
        let result = parse_document(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_mixed_content() {
        let input = "**Bold** and *italic* with `code` and $math$";
        let mut state = ParseState::new(input);
        let result = parse_inline_elements(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_nested_structures() {
        let input = r#"> This is a quote with **bold text**
> and `inline code` inside."#;
        let mut state = ParseState::new(input);
        let result = parse_block_element(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_complex_list() {
        let input = r#"1. First item with **bold**
2. Second item with [link](url)
   - Nested unordered item
   - Another nested item
3. Third item with `code`"#;
        let mut state = ParseState::new(input);
        let result = parse_block_element(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_code_with_syntax() {
        let input = r#"```python
def hello():
    print("Hello, world!")
    return 42
```"#;
        let mut state = ParseState::new(input);
        let result = parse_block_element(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_math_expressions() {
        let input = r#"Inline math: $x^2 + y^2 = z^2$

Block math:
$$
\begin{align}
a &= b + c \\
d &= e + f
\end{align}
$$"#;
        let mut state = ParseState::new(input);
        let result = parse_document(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_table_with_alignment() {
        let input = r#"| Left | Center | Right |
|:-----|:------:|------:|
| L1   |   C1   |    R1 |
| L2   |   C2   |    R2 |"#;
        let mut state = ParseState::new(input);
        let result = parse_block_element(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_callout_with_content() {
        let input = r#"> [!NOTE]
> This is a note callout with **bold** text
> and multiple lines."#;
        let mut state = ParseState::new(input);
        let result = parse_block_element(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_commands_and_macros() {
        let input = r#"\command{argument}

↯macro(param1, param2)

Normal text after commands."#;
        let mut state = ParseState::new(input);
        let result = parse_document(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_escaped_characters() {
        let input = r#"\*not italic\* and \`not code\` and \[not link\]"#;
        let mut state = ParseState::new(input);
        let result = parse_inline_elements(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_empty_document() {
        let input = "";
        let mut state = ParseState::new(input);
        let result = parse_document(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_whitespace_only() {
        let input = "   \n\n  \t  \n";
        let mut state = ParseState::new(input);
        let result = parse_document(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_malformed_input() {
        let input = "[unclosed link and **unclosed bold";
        let mut state = ParseState::new(input);
        let result = parse_document(&mut state);
        // Should handle gracefully, possibly as plain text
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_mixed_line_endings() {
        let input = "Line 1\nLine 2\r\nLine 3\rLine 4";
        let mut state = ParseState::new(input);
        let result = parse_document(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_unicode_content() {
        let input = "# 标题\n\n这是一个包含**粗体**和*斜体*的段落。\n\n- 列表项 1\n- 列表项 2";
        let mut state = ParseState::new(input);
        let result = parse_document(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_deeply_nested() {
        let input = r#"> Quote level 1
> > Quote level 2
> > > Quote level 3
> > Back to level 2
> Back to level 1"#;
        let mut state = ParseState::new(input);
        let result = parse_block_element(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_performance_large_document() {
        // Create a large document for performance testing
        let mut large_doc = String::new();
        for i in 0..1000 {
            large_doc.push_str(&format!("## Header {}\n\n", i));
            large_doc.push_str(&format!("This is paragraph {} with **bold** text.\n\n", i));
            large_doc.push_str(&format!("- List item {}\n", i));
            large_doc.push_str(&format!("- Another item {}\n\n", i));
        }
        
        let mut state = ParseState::new(&large_doc);
        let start = std::time::Instant::now();
        let result = parse_document(&mut state);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        println!("Parsed large document in {:?}", duration);
        // Should complete in reasonable time (adjust threshold as needed)
        assert!(duration.as_secs() < 5);
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_error_recovery() {
        let input = "Valid text [broken link **valid bold** more text";
        let mut state = ParseState::new(input);
        let result = parse_document(&mut state);
        // Should recover from errors and continue parsing
        assert!(result.is_ok());
    }

    #[test]
    fn test_position_tracking() {
        let input = "Line 1\nLine 2\nError here";
        let mut state = ParseState::new(input);
        
        // Advance to line 3
        while !state.is_at_end() && state.peek() != Some('E') {
            state.advance();
        }
        
        let pos = state.mark_position();
        // Position tracking should work correctly
        assert!(pos.line >= 2); // Should be on line 3 (0-indexed)
    }

    #[test]
    fn test_backtracking() {
        let input = "Maybe a link [text] but not really";
        let mut state = ParseState::new(input);
        
        // Skip to the bracket
        while state.peek() != Some('[') {
            state.advance();
        }
        
        let saved_pos = state.mark_position();
        
        // Try to parse as link (should fail)
        let link_result = parse_link(&mut state);
        assert!(link_result.is_err());
        
        // Restore position and continue
        state.restore_position(saved_pos);
        assert_eq!(state.peek(), Some('['));
    }
}