use crate::helpers::{NotedownNode, ParseState};
use notedown_ast::ast::{ParagraphSpan, ParagraphTerm, TextPlainNode};
use notedown_error::NoteError;

use notedown_ast::ast::CodeInlineSpan;
use crate::parsers::code::is_inline_code_start;

fn parse_inline_content<F>(state: &mut ParseState, mut is_end: F) -> Result<Vec<ParagraphTerm>, NoteError>
where
    F: FnMut(&mut ParseState) -> bool,
{
    let mut terms = Vec::new();

    // Skip leading whitespace
    state.skip_whitespace();

    while !state.is_at_end() && !is_end(state) {
        // Try to parse different inline elements
        if is_inline_code_start(state) {
            if let Ok(code) = CodeInlineSpan::parse(state) {
                terms.push(ParagraphTerm::Code(code));
            } else {
                // Fallback to text
                if let Ok(text) = TextPlainNode::parse(state) {
                    terms.push(ParagraphTerm::Text(text));
                } else {
                    break;
                }
            }
        } else {
            // Parse as plain text
            if let Ok(text) = TextPlainNode::parse(state) {
                terms.push(ParagraphTerm::Text(text));
            } else {
                break;
            }
        }
    }
    Ok(terms)
}

impl NotedownNode for ParagraphSpan {
    type Target = Self;

    fn parse(state: &mut ParseState) -> Result<Self::Target, NoteError> {
        let start = state.mark_position();
        let terms = parse_inline_content(state, |s| s.peek() == Some('\n') || s.peek() == Some('\r'))?;

        if terms.is_empty() {
            return Err(NoteError::syntax_error(
                "Empty paragraph",
                start.offset..state.position.offset,
            ));
        }

        let end = state.position;
        let span = start.offset..end.offset;

        Ok(ParagraphSpan {
            terms,
            span,
        })
    }
}

pub fn parse_paragraph_content(
    state: &mut ParseState,
    delimiter: &str,
) -> Result<ParagraphSpan, NoteError> {
    let start = state.mark_position();
    let terms = parse_inline_content(state, |s| s.starts_with(delimiter))?;
    let end = state.position;
    let span = start.offset..end.offset;
    Ok(ParagraphSpan {
        terms,
        span,
    })
}

/// Parse a complete paragraph that may span multiple lines
pub fn parse_paragraph(state: &mut ParseState) -> Result<ParagraphSpan, NoteError> {
    let start = state.mark_position();
    let mut terms = Vec::new();
    
    loop {
        // Skip whitespace but preserve structure
        state.skip_whitespace();
        
        // Check for paragraph boundary
        if state.is_at_end() || is_paragraph_boundary(state) {
            break;
        }
        
        // Parse line content
        if let Ok(paragraph_line) = ParagraphSpan::parse(state) {
            terms.extend(paragraph_line.terms);
        } else {
            break;
        }
        
        // Handle line endings
        if state.peek() == Some('\n') {
            state.advance();
            // Check for double newline (paragraph break)
            if state.peek() == Some('\n') {
                break;
            }
        }
    }
    
    if terms.is_empty() {
        return Err(NoteError::syntax_error(
            "Empty paragraph",
            start.offset..state.position.offset,
        ));
    }
    
    let end = state.position;
    let span = start.offset..end.offset;
    
    Ok(ParagraphSpan {
        terms,
        span,
    })
}

/// Check if current position indicates start of a block element
fn is_block_element_start(state: &ParseState) -> bool {
    if let Some(ch) = state.peek() {
        match ch {
            '#' => true,  // Heading
            '>' => true,  // Quote
            '-' | '*' | '+' => {
                // Check if it's a list item (followed by space)
                let rest = state.rest();
                rest.len() > 1 && rest.chars().nth(1) == Some(' ')
            }
            '`' => {
                // Check for code block
                state.starts_with("```")
            }
            _ if ch.is_ascii_digit() => {
                // Check for ordered list
                let rest = state.rest();
                rest.chars().skip(1).take_while(|c| c.is_ascii_digit()).count() > 0 &&
                rest.chars().skip_while(|c| c.is_ascii_digit()).next() == Some('.')
            }
            _ => false,
        }
    } else {
        false
    }
}

/// Check if current position indicates a paragraph boundary
fn is_paragraph_boundary(state: &ParseState) -> bool {
    // Empty line or block element start
    state.peek() == Some('\n') || is_block_element_start(state)
}
