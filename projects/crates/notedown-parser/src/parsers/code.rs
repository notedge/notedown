use crate::helpers::{NotedownNode, ParseState};
use notedown_ast::ast::CodeInlineSpan;
use notedown_error::NoteError;
use std::ops::Range;

/// Parse inline code: `code`
impl NotedownNode for CodeInlineSpan {
    type Target = Self;

    fn parse(state: &mut ParseState) -> Result<Self::Target, NoteError> {
        let start = state.mark_position();
        
        // Count opening backticks to determine level
        let mut level = 0;
        while state.peek() == Some('`') {
            level += 1;
            state.advance();
        }
        
        if level == 0 {
            return Err(NoteError::syntax_error("Expected `"));
        }
        
        // Parse content until matching number of closing backticks
        let mut content = String::new();
        
        while !state.is_at_end() {
            // Check for closing backticks
            let mut closing_level = 0;
            let checkpoint = state.position;
            
            while state.peek() == Some('`') {
                closing_level += 1;
                state.advance();
            }
            
            if closing_level == level {
                // Found matching closing backticks
                break;
            } else if closing_level > 0 {
                // Found some backticks but not matching, restore and include in content
                state.position = checkpoint;
                for _ in 0..closing_level {
                    content.push('`');
                    state.advance();
                }
            } else {
                // Regular character
                if let Some(ch) = state.advance() {
                    content.push(ch);
                }
            }
        }
        
        let end = state.position;
        let span = start.offset..end.offset;
        
        Ok(CodeInlineSpan {
            level,
            code: content,
            span,
        })
    }
}

/// Parse code block: ```lang\ncode\n```
pub fn parse_code_block(state: &mut ParseState) -> Result<(String, String), NoteError> {
    // Consume opening ```
    if !state.starts_with("```") {
        return Err(NoteError::syntax_error("Expected ```"));
    }
    state.advance_by("```");
    
    // Parse language identifier (optional)
    let mut language = String::new();
    while let Some(ch) = state.peek() {
        if ch == '\n' || ch == '\r' {
            break;
        }
        if ch.is_whitespace() {
            state.advance();
            break;
        }
        language.push(ch);
        state.advance();
    }
    
    // Skip to next line
    state.skip_whitespace_preserve_newlines();
    if state.peek() == Some('\n') {
        state.advance();
    }
    
    // Parse code content until closing ```
    let mut code = String::new();
    
    while !state.is_at_end() {
        if state.starts_with("```") {
            state.advance_by("```");
            break;
        }
        
        if let Some(ch) = state.advance() {
            code.push(ch);
        }
    }
    
    Ok((language, code))
}

/// Check if current position starts inline code
pub fn is_inline_code_start(state: &ParseState) -> bool {
    state.peek() == Some('`')
}