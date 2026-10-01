use super::*;
/// ## Details List
/// ```note
/// >- summary part
/// >- sum
/// >  part2
/// >  part3
/// ```
///
/// ```html
/// <details open>
/// <summary>Want to ruin the surprise?</summary>
/// <br>
/// Well, you asked for it!
/// </details>
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetailedList {
    ///
    pub summary: ListItem,
    ///
    pub body: Vec<ListItem>,
}

impl DetailedList {
    /// open?
    pub fn is_open(&self) -> bool {
        matches!(self.summary.prefix.value, ListPrefixMark::SummaryOpen)
    }
}
