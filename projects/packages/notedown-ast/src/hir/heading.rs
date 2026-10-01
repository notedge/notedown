use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HeadingLevel {
    BookPart,
    Chapter,
    Section,
    Article,
    Header1,
    Header2,
    Header3,
    Header4,
    Header5,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HeadingNode {
    pub level: HeadingLevel,
    pub id: String,
    pub terms: ParagraphNode,
    pub span: Range<u32>,
}

impl From<usize> for HeadingLevel {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::Article,
            1 => Self::Header1,
            2 => Self::Header2,
            3 => Self::Header3,
            4 => Self::Header4,
            _ => Self::Header5,
        }
    }
}
