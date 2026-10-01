use super::*;
use notedown_error::NoteError;

/// `\w+:(\/?\/?)[^\s]+`
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UriNode {
    pub scheme: IdentifierNode,
    pub body: TextPlainNode,
    pub span: Range<u32>,
}

impl UriNode {
    pub fn as_url(&self) -> Result<Url, NoteError> {
        let maybe = format!("{}:{}", self.scheme.name, self.body.text);
        Ok(Url::parse(&maybe)?)
    }
}
