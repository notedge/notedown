use super::*;

/// ## Two-way link
/// This means this link will create a reference on the opposite side
/// ```note
/// [[link]]
/// [[link > id]]
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TwoWayLink {
    link: String,
    id: Option<String>,
}
