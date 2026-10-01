use crate::helpers::{NotedownNode, ParseState};
use notedown_ast::ast::TextPlainNode;
use notedown_error::NoteError;
use std::ops::Range;

impl NotedownNode for TextPlainNode {
    type Target = TextPlainNode;

    fn parse(state: &mut ParseState) -> Result<Self::Target, NoteError> {
        let start = state.mark_position();
        let mut text = String::new();
        
        while let Some(ch) = state.peek() {
            // Stop at special characters that start other constructs
            match ch {
                '*' | '_' | '`' | '$' | '\\' | '[' | ']' | '\n' | '\r' => break,
                _ => {
                    text.push(ch);
                    state.advance();
                }
            }
        }

        if text.is_empty() {
            return Err(NoteError::syntax_error("Expected text content", start.offset..start.offset));
        }

        let end = state.position;
        let span = start.offset..end.offset;

        Ok(TextPlainNode {
            text,
            span,
        })
    }
}

/// Parse a single word (sequence of non-whitespace characters)
pub fn parse_word(state: &mut ParseState) -> Result<String, NoteError> {
    let mut word = String::new();
    
    while let Some(ch) = state.peek() {
        if ch.is_whitespace() {
            break;
        }
        word.push(ch);
        state.advance();
    }
    
    if word.is_empty() {
        Err(NoteError::syntax_error("Expected word"))
    } else {
        Ok(word)
    }
}

/// Parse text until a specific delimiter
pub fn parse_text_until(state: &mut ParseState, delimiter: &str) -> Result<String, NoteError> {
    let mut text = String::new();
    
    while !state.is_at_end() && !state.starts_with(delimiter) {
        if let Some(ch) = state.advance() {
            text.push(ch);
        }
    }
    
    Ok(text)
}

/// Parse text until any of the given characters
pub fn parse_text_until_any(state: &mut ParseState, chars: &[char]) -> String {
    let mut text = String::new();
    
    while !state.is_at_end() {
        if let Some(ch) = state.peek() {
            if chars.contains(&ch) {
                break;
            }
            text.push(ch);
            state.advance();
        } else {
            break;
        }
    }
    
    text
}