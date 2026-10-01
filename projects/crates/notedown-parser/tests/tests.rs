//! Integration tests for Notedown parser

use notedown_parser::helpers::{NotedownNode, ParseState};
use notedown_parser::parsers::*;
use notedown_ast::ast::*;
use notedown_error::NoteError;

#[cfg(test)]
mod text_tests {
    use super::*;

    #[test]
    fn test_parse_normal_text() {
        let mut state = ParseState::new("Hello world".to_string());
        let result = TextPlainNode::parse(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_text_with_special_chars() {
        let mut state = ParseState::new("Hello, world!".to_string());
        let result = TextPlainNode::parse(&mut state);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert_eq!(text.text, "Hello, world!");
    }

    #[test]
    fn test_parse_empty_text() {
        let mut state = ParseState::new("".to_string());
        let result = TextPlainNode::parse(&mut state);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod header_tests {
    use super::*;
    use notedown_parser::parsers::header::*;

    #[test]
    fn test_parse_atx_header() {
        let mut state = ParseState::new("# Header 1".to_string());
        let result = HeadingSpan::parse(&mut state);
        assert!(result.is_ok());
        let heading = result.unwrap();
        assert_eq!(heading.level, 1);
        
        let mut state = ParseState::new("## Header 2".to_string());
        let result = HeadingSpan::parse(&mut state);
        assert!(result.is_ok());
        let heading = result.unwrap();
        assert_eq!(heading.level, 2);
        
        let mut state = ParseState::new("###### Header 6".to_string());
        let result = HeadingSpan::parse(&mut state);
        assert!(result.is_ok());
        let heading = result.unwrap();
        assert_eq!(heading.level, 6);
    }

    #[test]
    fn test_invalid_header() {
        let mut state = ParseState::new("####### Too many hashes".to_string());
        let result = HeadingSpan::parse(&mut state);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_heading_start() {
        let state = ParseState::new("# Header".to_string());
        assert!(is_heading_start(&state));
        
        let state = ParseState::new("Not a header".to_string());
        assert!(!is_heading_start(&state));
    }
}

#[cfg(test)]
mod paragraph_tests {
    use super::*;
    use notedown_parser::parsers::paragraph::*;

    #[test]
    fn test_parse_simple_paragraph() {
        let mut state = ParseState::new("This is a simple paragraph.".to_string());
        let result = ParagraphSpan::parse(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_paragraph_with_styled_text() {
        let mut state = ParseState::new("This has **bold** text.".to_string());
        let result = ParagraphSpan::parse(&mut state);
        assert!(result.is_ok());
        let paragraph = result.unwrap();
        assert!(!paragraph.terms.is_empty());
    }

    #[test]
    fn test_parse_multiline_paragraph() {
        let mut state = ParseState::new("This is a\nmultiline paragraph\nwith several lines.".to_string());
        let result = parse_paragraph(&mut state);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod code_tests {
    use super::*;
    use notedown_parser::parsers::code::*;

    #[test]
    fn test_parse_inline_code() {
        let mut state = ParseState::new("`let x = 42;`".to_string());
        let result = CodeInlineSpan::parse(&mut state);
        assert!(result.is_ok());
        let code = result.unwrap();
        assert_eq!(code.code, "let x = 42;");
        assert_eq!(code.level, 1);
    }

    #[test]
    fn test_parse_code_block() {
        let mut state = ParseState::new("```rust\nfn main() {\n    println!(\"Hello\");\n}\n```".to_string());
        let result = parse_code_block(&mut state);
        assert!(result.is_ok());
        let (language, code) = result.unwrap();
        assert_eq!(language, "rust");
        assert!(code.contains("fn main()"));
    }

    #[test]
    fn test_is_inline_code_start() {
        let state = ParseState::new("`code`".to_string());
        assert!(is_inline_code_start(&state));
        
        let state = ParseState::new("not code".to_string());
        assert!(!is_inline_code_start(&state));
    }

    #[test]
    fn test_is_code_block_start() {
        let state = ParseState::new("```rust".to_string());
        assert!(is_code_block_start(&state));
        
        let state = ParseState::new("not code".to_string());
        assert!(!is_code_block_start(&state));
    }
}

#[cfg(test)]
mod math_tests {
    use super::*;
    use notedown_parser::parsers::math::*;

    #[test]
    fn test_parse_inline_math() {
        let mut state = ParseState::new("$x^2 + y^2 = z^2$".to_string());
        let result = InlineMathSpan::parse(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_display_math() {
        let mut state = ParseState::new("$$\\int_0^1 x^2 dx$$".to_string());
        let result = DisplayMathSpan::parse(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_try_parse_math() {
        let mut state = ParseState::new("$math$".to_string());
        let result = try_parse_math(&mut state);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
        
        let mut state = ParseState::new("not math".to_string());
        let result = try_parse_math(&mut state);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_is_math_start() {
        let state = ParseState::new("$math$".to_string());
        assert!(is_math_start(&state));
        
        let state = ParseState::new("not math".to_string());
        assert!(!is_math_start(&state));
    }
}

#[cfg(test)]
mod styled_tests {
    use super::*;
    use notedown_parser::parsers::styled::*;

    #[test]
    fn test_parse_bold() {
        let mut state = ParseState::new("**bold text**".to_string());
        let result = FontBoldSpan::parse(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_italic() {
        let mut state = ParseState::new("*italic text*".to_string());
        let result = FontItalicSpan::parse(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_bold_italic() {
        let mut state = ParseState::new("***bold italic***".to_string());
        let result = FontBoldItalicSpan::parse(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_underline() {
        let mut state = ParseState::new("_underlined_".to_string());
        let result = FontUnderlineSpan::parse(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_strikethrough() {
        let mut state = ParseState::new("~~strikethrough~~".to_string());
        let result = FontDeleteSpan::parse(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_try_parse_styled() {
        let mut state = ParseState::new("**bold**".to_string());
        let result = try_parse_styled(&mut state);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
        
        let mut state = ParseState::new("not styled".to_string());
        let result = try_parse_styled(&mut state);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_parse_mixed_content() {
        let content = "# Title\n\nThis is **bold** and *italic* text with `code`.";
        let mut state = ParseState::new(content.to_string());
        
        // Parse heading
        let heading = HeadingSpan::parse(&mut state);
        assert!(heading.is_ok());
        
        // Skip newlines
        state.skip_whitespace();
        
        // Parse paragraph
        let paragraph = ParagraphSpan::parse(&mut state);
        assert!(paragraph.is_ok());
    }

    #[test]
    fn test_parse_document_structure() {
        let content = r#"# Document Title

This is a paragraph with **bold** and *italic* text.

## Subsection

Here's some math: $x^2 + y^2 = z^2$

And some code: `fn main() {}`"#;
        let mut state = ParseState::new(content.to_string());
        
        // This is a basic structure test - in a real implementation,
        // we would have a document parser that handles the full structure
        assert!(!state.is_at_end());
    }

    #[test]
    fn test_error_handling() {
        // Test unclosed bold
        let mut state = ParseState::new("**unclosed bold".to_string());
        let result = FontBoldSpan::parse(&mut state);
        assert!(result.is_err());
        
        // Test unclosed math
        let mut state = ParseState::new("$unclosed math".to_string());
        let result = InlineMathSpan::parse(&mut state);
        assert!(result.is_err());
        
        // Test invalid heading level
        let mut state = ParseState::new("####### Too many hashes".to_string());
        let result = HeadingSpan::parse(&mut state);
        assert!(result.is_err());
    }
}