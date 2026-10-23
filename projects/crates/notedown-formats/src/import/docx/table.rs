use notedown_ir::{Block, DocumentGraph, Inline, TableCell, TableRow};

/// Mutable table parse state for `w:tbl`/`w:tr`/`w:tc` walks.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub(crate) struct TableState {
    pub rows: Vec<Vec<TableCell>>,
    pub current_row: Vec<TableCell>,
    pub current_cell: Vec<Inline>,
}

pub(crate) fn append_cell_paragraph(cell: &mut Vec<Inline>, content: &[Inline]) {
    if content.is_empty() {
        return;
    }
    if !cell.is_empty() {
        cell.push(Inline::Text { text: "\n".into() });
    }
    cell.extend(content.iter().cloned());
}

pub(crate) fn push_table_block(graph: &mut DocumentGraph, table: TableState) {
    if table.rows.is_empty() {
        return;
    }
    let rows = table
        .rows
        .into_iter()
        .enumerate()
        .map(|(index, cells)| TableRow { is_header: index == 0, cells })
        .collect();
    graph.push_block(Block::Table { caption: None, rows });
}
