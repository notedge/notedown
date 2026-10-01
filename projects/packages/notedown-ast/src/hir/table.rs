/// | title | title |
/// | align | align |
/// | text | text |
pub struct TableNode {
    pub title: Vec<String>,
    pub align: Vec<TableAlignMode>,
    pub content: Vec<Vec<String>>,
}

pub enum TableAlignMode {
    Left,
    Center,
    Right,
}

impl TableNode {
    pub fn fill_columns(&mut self) {}
    pub fn count_columns(&self) -> usize {
        let mut max = self.title.len();
        // max = max.max(self.align.len());
        for row in self.content.iter() {
            max = max.max(row.len());
        }
        max
    }
}
