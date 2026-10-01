//! Parser combinators for Notedown language

use crate::helpers::{NotedownNode, ParseState};
use notedown_ast::ast::*;
use notedown_error::NoteError;

pub mod code;
pub mod command;
pub mod delimiter;
pub mod header;
pub mod link;
pub mod list;
pub mod math;
pub mod paragraph;
pub mod quote;
pub mod styled;
pub mod table;
pub mod text;
pub mod value;

// Re-export parsers

impl NotedownNode for NotedownAST {
    type Target = Self;

    fn parse(state: &mut ParseState) -> Result<Self::Target, NoteError> {
        let mut terms = Vec::new();
        
        while !state.is_at_end() {
            state.skip_ignored();
            
            if state.is_at_end() {
                break;
            }
            
            let term = NotedownTerm::parse(state)?;
            terms.push(term);
        }
        
        Ok(NotedownAST {
            terms,
            path: None,
        })
    }
}

impl NotedownNode for NotedownTerm {
    type Target = Self;

    fn parse(state: &mut ParseState) -> Result<Self::Target, NoteError> {
        use crate::parsers::*;

        state.skip_ignored();

        // Try to parse different types of terms
        if state.peek() == Some('#') {
            let heading = HeadingSpan::parse(state)?;
            return Ok(NotedownTerm::Heading(Box::new(heading)));
        }

        if code::is_code_block_start(state) {
            // let (language, code_content) = code::parse_code_block(state)?;
            // For now, we'll create a simple text representation
            // This might need adjustment based on the actual AST structure
            return Err(NoteError::syntax_error(
                "Code blocks not yet supported in AST",
                state.position..state.position,
            ));
        }

        // Try to parse as paragraph
        let paragraph = ParagraphSpan::parse(state)?;
        Ok(NotedownTerm::Paragraph(Box::new(paragraph)))
    }
}
