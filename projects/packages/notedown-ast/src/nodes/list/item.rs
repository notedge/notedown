use super::*;

/// A list item contains the prefix and rest ast nodes
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListItem {
    /// Prefix of the list item
    pub prefix: ListPrefixMark,
    /// Rest parts the list item
    pub rest: NotedownNodes,
}

impl From<NotedownNodes> for ListItem {
    fn from(node: NotedownNodes) -> Self {
        Self { prefix: Default::default(), rest: node }
    }
}

impl ListItem {}
