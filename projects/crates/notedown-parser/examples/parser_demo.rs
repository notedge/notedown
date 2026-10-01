//! Demonstration of the Notedown parser combinator implementation

use notedown_parser::helpers::{NotedownNode, ParseState};
use notedown_ast::ast::*;
use notedown_error::NotedownError;

fn main() -> Result<(), NotedownError> {
    // Example 1: Simple text parsing
    println!("=== Example 1: Simple Text ===");
    let mut state = ParseState::new("Hello, world!", "demo");
    let text = text::TextPlainNode::parse(&mut state)?;
    println!("Parsed text: '{}'", text.text);
    println!();

    // Example 2: Heading parsing
    println!("=== Example 2: Heading ===");
    let mut state = ParseState::new("# Main Title", "demo");
    let heading = title::HeadingSpan::parse(&mut state)?;
    println!("Heading level: {}", heading.level);
    if let Some(paragraph::ParagraphTerm::Text(text)) = heading.text.terms.first() {
        println!("Heading text: '{}'", text.text);
    }
    println!();

    // Example 3: Styled text parsing
    println!("=== Example 3: Styled Text ===");
    let mut state = ParseState::new("**bold text**", "demo");
    let bold = style::FontBoldSpan::parse(&mut state)?;
    if let Some(paragraph::ParagraphTerm::Text(text)) = bold.text.terms.first() {
        println!("Bold text: '{}'", text.text);
    }
    println!();

    // Example 4: Math parsing
    println!("=== Example 4: Math ===");
    let mut state = ParseState::new("$E = mc^2$", "demo");
    let math = math::InlineMathSpan::parse(&mut state)?;
    println!("Math content: '{}'", math.content);
    println!();

    // Example 5: Code parsing
    println!("=== Example 5: Code ===");
    let mut state = ParseState::new("`let x = 42;`", "demo");
    let code = code::CodeInlineSpan::parse(&mut state)?;
    println!("Code content: '{}'", code.code);
    println!();

    // Example 6: Paragraph with mixed content
    println!("=== Example 6: Mixed Paragraph ===");
    let mut state = ParseState::new("This has **bold**, *italic*, and `code` elements.", "demo");
    let paragraph = paragraph::ParagraphSpan::parse(&mut state)?;
    println!("Paragraph with {} terms:", paragraph.terms.len());
    for (i, term) in paragraph.terms.iter().enumerate() {
        match term {
            paragraph::ParagraphTerm::Text(text) => println!("  {}: Text: '{}'", i, text.text),
            paragraph::ParagraphTerm::Bold(bold) => {
                if let Some(paragraph::ParagraphTerm::Text(text)) = bold.text.terms.first() {
                    println!("  {}: Bold: '{}'", i, text.text);
                }
            },
            paragraph::ParagraphTerm::Italic(italic) => {
                if let Some(paragraph::ParagraphTerm::Text(text)) = italic.text.terms.first() {
                    println!("  {}: Italic: '{}'", i, text.text);
                }
            },
            paragraph::ParagraphTerm::Code(code) => println!("  {}: Code: '{}'", i, code.code),
            _ => println!("  {}: Other term", i),
        }
    }
    println!();

    // Example 7: Complete document parsing
    println!("=== Example 7: Complete Document ===");
    let document = r#"# Document Title

This is a paragraph with **bold** and *italic* text.

## Subsection

Here's some math: $x^2 + y^2 = z^2$

And some code: `fn main() {}`
"#;
    
    let mut state = ParseState::new(document, "demo");
    let ast = NotedownAST::parse(&mut state)?;
    println!("Document parsed with {} top-level terms:", ast.terms.len());
    
    for (i, term) in ast.terms.iter().enumerate() {
        match term {
            NotedownTerm::Heading(heading) => {
                println!("  {}: Heading (level {})", i, heading.level);
            },
            NotedownTerm::Paragraph(paragraph) => {
                println!("  {}: Paragraph with {} terms", i, paragraph.terms.len());
            },
            NotedownTerm::SpaceBreak(_) => {
                println!("  {}: Space break", i);
            },
            NotedownTerm::MathBlock(math) => {
                println!("  {}: Math block: '{}'", i, math.content);
            },
        }
    }

    println!("\n=== Parser Demo Complete ===");
    Ok(())
}