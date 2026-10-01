use crate::nodes::*;

/// Table link nodes
#[derive(Clone, Eq, PartialEq)]
pub enum TableView {
    /// Simple markdown table
    SimpleTable(Box<SimpleTable>),
}

///
#[derive(Clone, Eq, PartialEq)]
pub struct SimpleTable {
    ///
    pub head: Vec<NotedownNode>,
    ///
    pub align: Vec<u8>,
    ///
    pub terms: Vec<Vec<NotedownNode>>,
    ///
    pub column: usize,
}
