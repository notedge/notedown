use super::*;

impl Display for QuoteBlock {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "< quote>")?;
        writeln!(f, "</quote>")
    }
}
