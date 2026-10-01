mod display;
use std::{
    error::Error,
    fmt::{Debug, Display, Formatter},
    ops::Range,
};
#[cfg(feature = "url")]
use url::Url;

pub type Validation<T> = validatus::Validation<T, NoteError>;

pub struct NoteError {
    pub(crate) kind: Box<NoteErrorKind>,
}

#[derive(Debug)]
pub enum NoteErrorKind {
    IOError {
        message: String,
        source: std::io::Error,
    },
    Syntax {
        message: String,
        range: Range<usize>,
        #[cfg(feature = "url")]
        file: Option<Url>,
    },
    Custom {
        message: String,
    },
    Unknown {
        message: String,
    },
}

impl From<std::io::Error> for NoteError {
    fn from(e: std::io::Error) -> Self {
        Self { kind: Box::new(NoteErrorKind::IOError { message: "".to_string(), source: e }) }
    }
}

impl From<std::fmt::Error> for NoteError {
    fn from(value: std::fmt::Error) -> Self {
        todo!()
    }
}

impl From<()> for NoteError {
    fn from(_: ()) -> Self {
        todo!()
    }
}

impl NoteError {
    pub fn syntax_error<T: ToString>(message: T, range: Range<usize>) -> Self {
        Self { kind: Box::new(NoteErrorKind::Syntax { message: message.to_string(), range, file: None }) }
    }

    pub fn custom<T: ToString>(message: T) -> Self {
        Self { kind: Box::new(NoteErrorKind::Custom { message: message.to_string() }) }
    }
    pub fn with_url(&mut self, url: Url) {
        match self.kind.as_mut() {
            NoteErrorKind::Syntax { file, .. } => {
                *file = Some(url);
            }
            _ => {}
        }
    }
}
