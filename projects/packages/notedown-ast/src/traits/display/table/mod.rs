use super::*;

impl Debug for TableView {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SimpleTable(v) => Debug::fmt(v, f),
        }
    }
}

impl Display for TableView {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SimpleTable(v) => Display::fmt(v, f),
        }
    }
}

impl Debug for SimpleTable {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let w = &mut f.debug_struct("Table");
        w.field("column", &self.column);
        w.finish()
    }
}

impl Display for SimpleTable {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let _ = f;
        todo!()
    }
}
