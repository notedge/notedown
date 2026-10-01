use crate::helpers::{NotedownNode, ParseState};
use notedown_ast::ast::{HeadingLevel, HeadingSpan, ParagraphTerm};
use notedown_ast::ast::TextPlainNode;
use notedown_error::NoteError;
use std::ops::Range;

impl NotedownNode for HeadingSpan {
    type Target = Self;

    fn parse(state: &mut ParseState) -> Result<Self::Target, NoteError> {
        let start = state.mark_position();
        
        // Parse heading level (count # characters)
        let mut level = 0;
        while state.peek() == Some('#') {
            level += 1;
            state.advance();
            if level > 6 {
                return Err(NoteError::syntax_error("Heading level cannot exceed 6"));
            }
        }
        
        if level == 0 {
            return Err(NoteError::syntax_error("Expected # for heading"));
        }
        
        // Skip whitespace after #
        state.skip_whitespace_preserve_newlines();
        
        // Parse heading text until end of line
        let text_start = state.mark_position();
        let mut text_content = String::new();
        
        while let Some(ch) = state.peek() {
            if ch == '\n' || ch == '\r' {
                break;
            }
            text_content.push(ch);
            state.advance();
        }
        
        let text_end = state.position;
        let text_span = text_start.offset..text_end.offset;
        
        // Create paragraph span for the heading text
        let text = ParagraphSpan {
            terms: if text_content.is_empty() {
                vec![]
            } else {
                vec![ParagraphTerm::Text(TextPlainNode {
                    text: text_content,
                    span: text_span.clone(),
                })]
            },
            span: text_span,
        };
        
        let end = state.position;
        let span = start.offset..end.offset;
        
        Ok(HeadingSpan {
            level,
            text,
            span,
        })
    }
}

/// Parse heading at any level (1-6)
pub fn parse_heading(state: &mut ParseState) -> Result<HeadingSpan, NoteError> {
    HeadingSpan::parse(state)
}