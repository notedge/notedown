use super::*;

/// # Ordered List
/// ```note
/// 1. part1
/// 2. part2
///    part2
/// 3. part3
///
/// 4. part4
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderedList {
    /// First element number of the list
    pub first_order: usize,
    /// Children nodes of the list
    pub children: Vec<ListItem>,
}

impl ListView {
    /// Constructor of [`OrderedList`]
    #[inline]
    pub fn ordered_list(children: Vec<ListItem>) -> Self {
        let list = OrderedList { first_order: 0, children };
        Self::Ordered(Box::new(list))
    }
}
