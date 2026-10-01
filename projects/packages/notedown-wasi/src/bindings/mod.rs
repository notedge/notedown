use crate::{
    exports::notedown::core::types::{GuestUrl, NotedownError, SyntaxError, TextRange},
    NotedownHost,
};
use std::str::FromStr;
use url::ParseError;

impl crate::exports::notedown::core::types::Guest for NotedownHost {
    type Url = UrlNative;
}

pub struct UrlNative {
    repr: url::Url,
}

impl GuestUrl for UrlNative {}

impl FromStr for UrlNative {
    type Err = NotedownError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self { repr: url::Url::from_str(s)? })
    }
}

impl From<ParseError> for NotedownError {
    fn from(value: ParseError) -> Self {
        NotedownError::Syntax(SyntaxError { reason: value.to_string(), file: None, range: TextRange { head_offset: 0, tail_offset: 0 } })
    }
}
