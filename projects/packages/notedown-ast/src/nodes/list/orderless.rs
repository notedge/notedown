use super::*;

/// ## Orderless List
/// ```note
/// - part1
/// - part2
///   part2
/// - part3
///
/// - part4
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderlessList {
    /// list items
    pub children: Vec<ListItem>,
}
