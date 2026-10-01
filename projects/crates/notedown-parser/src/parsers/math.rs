use crate::helpers::{NotedownNode, ParseState};
use notedown_ast::ast::{InlineMathSpan, DisplayMathSpan, ParagraphSpan};
use notedown_error::NoteError;
use std::ops::Range;

/// Parse inline math: $formula$
impl NotedownNode for InlineMathSpan {
    type Target = Self;

    fn parse(state: &mut ParseState) -> Result<Self::Target, NoteError> {
        let start = state.mark_position();
        
        // Check for display math first ($$)
        if state.starts_with("$$") {
            return Err(NoteError::syntax_error("Use DisplayMathSpan for $$"));
        }
        
        // Consume opening $
        if !state.consume('$') {
            return Err(NoteError::syntax_error("Expected $"));
        }
        
        // Parse content until closing $
        let content_start = state.mark_position();
        let mut content = String::new();
        
        while !state.is_at_end() && state.peek() != Some('$') {
            if let Some(ch) = state.advance() {
                content.push(ch);
            }
        }
        
        if !state.consume('$') {
            return Err(NoteError::syntax_error("Unclosed inline math, expected $"));
        }
        
        let content_end = state.position;
        let end = state.position;
        let span = start.offset..end.offset;
        let content_span = content_start.offset..content_end.offset;
        
        Ok(InlineMathSpan {
            text: ParagraphSpan {
                terms: vec![], // Math content as plain text
                span: content_span,
            },
            span,
        })
    }
}

/// Parse display math: $$formula$$
impl NotedownNode for DisplayMathSpan {
    type Target = Self;

    fn parse(state: &mut ParseState) -> Result<Self::Target, NoteError> {
        let start = state.mark_position();
        
        // Consume opening $$
        if !state.starts_with("$$") {
            return Err(NoteError::syntax_error("Expected $$"));
        }
        state.advance_by("$$");
        
        // Parse content until closing $$
        let content_start = state.mark_position();
        let mut content = String::new();
        
        while !state.is_at_end() && !state.starts_with("$$") {
            if let Some(ch) = state.advance() {
                content.push(ch);
            }
        }
        
        if !state.starts_with("$$") {
            return Err(NoteError::syntax_error("Unclosed display math, expected $$"));
        }
        
        let content_end = state.position;
        state.advance_by("$$");
        
        let end = state.position;
        let span = start.offset..end.offset;
        let content_span = content_start.offset..content_end.offset;
        
        Ok(DisplayMathSpan {
            text: ParagraphSpan {
                terms: vec![], // Math content as plain text
                span: content_span,
            },
            span,
        })
    }
}



/// Check if current position starts math
pub fn is_math_start(state: &ParseState) -> bool {
    state.peek() == Some('$')
}