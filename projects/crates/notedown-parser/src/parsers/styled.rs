use crate::helpers::{NotedownNode, ParseState};
use crate::parsers::paragraph::parse_paragraph_content;
use notedown_ast::ast::{
    FontBoldSpan, FontItalicSpan, FontBoldItalicSpan, 
    FontUnderlineSpan, FontDeleteSpan, ParagraphSpan
};
use notedown_error::NoteError;
use std::ops::Range;

/// Parse bold text: **text**
impl NotedownNode for FontBoldSpan {
    type Target = Self;

    fn parse(state: &mut ParseState) -> Result<Self::Target, NoteError> {
        let start = state.mark_position();
        
        // Consume opening **
        if !state.starts_with("**") {
            return Err(NoteError::syntax_error("Expected **", start.offset..start.offset));
        }
        state.advance_by("**");
        
        // Parse content until closing **
        let text = parse_paragraph_content(state, "**")?;

        if !state.starts_with("**") {
            return Err(NoteError::syntax_error(
                "Unclosed bold text, expected **",
                start.offset..state.position.offset,
            ));
        }

        state.advance_by("**");

        let end = state.position;
        let span = start.offset..end.offset;

        Ok(FontBoldSpan {
            text,
            span,
        })
    }
}

/// Parse italic text: *text*
impl NotedownNode for FontItalicSpan {
    type Target = Self;

    fn parse(state: &mut ParseState) -> Result<Self, NoteError> {
        let start = state.mark_position();
        state.expect("*")?;
        let text = parse_paragraph_content(state, "*")?;
        if !state.expect("*")? {
            return Err(NoteError::syntax_error(
                "Unclosed italic text, expected *",
                start.offset..state.position.offset,
            ));
        }
        Ok(FontItalicSpan {
            text,
            span: start.offset..state.position.offset,
        })
    }
}

/// Parse bold italic text: ***text***
impl NotedownNode for FontBoldItalicSpan {
    type Target = Self;

    fn parse(state: &mut ParseState) -> Result<Self::Target, NoteError> {
        let start = state.mark_position();
        
        // Consume opening ***
        if !state.starts_with("***") {
            return Err(NoteError::syntax_error("Expected ***", start.offset..start.offset));
        }
        state.advance_by("***");
        
        // Parse content until closing ***
        let text = parse_paragraph_content(state, "***")?;

        if !state.starts_with("***") {
            return Err(NoteError::syntax_error(
                "Unclosed bold italic text, expected ***",
                start.offset..state.position.offset,
            ));
        }

        state.advance_by("***");

        let end = state.position;
        let span = start.offset..end.offset;

        Ok(FontBoldItalicSpan {
            text,
            span,
        })
    }
}

/// Parse underline text: _text_
impl NotedownNode for FontUnderlineSpan {
    type Target = Self;

    fn parse(state: &mut ParseState) -> Result<Self::Target, NoteError> {
        let start = state.mark_position();
        
        // Consume opening _
        if !state.consume('_') {
            return Err(NoteError::syntax_error("Expected _", start.offset..start.offset));
        }
        
        // Parse content until closing _
        let text = parse_paragraph_content(state, "_")?;

        if !state.consume('_') {
            return Err(NoteError::syntax_error(
                "Unclosed underline text, expected _",
                start.offset..state.position.offset,
            ));
        }

        let end = state.position;
        let span = start.offset..end.offset;

        Ok(FontUnderlineSpan {
            text,
            span,
        })
    }
}

/// Parse delete text: ~~text~~
impl NotedownNode for FontDeleteSpan {
    type Target = Self;

    fn parse(state: &mut ParseState) -> Result<Self::Target, NoteError> {
        let start = state.mark_position();
        
        // Consume opening ~~
        if !state.starts_with("~~") {
            return Err(NoteError::syntax_error("Expected ~~", start.offset..start.offset));
        }
        state.advance_by("~~");
        
        // Parse content until closing ~~
        let text = parse_paragraph_content(state, "~~")?;

        if !state.starts_with("~~") {
            return Err(NoteError::syntax_error(
                "Unclosed delete text, expected ~~",
                start.offset..state.position.offset,
            ));
        }

        state.advance_by("~~");

        let end = state.position;
        let span = start.offset..end.offset;

        Ok(FontDeleteSpan {
            text,
            span,
        })
    }
}