use super::*;

pub mod detailed;
pub mod item;
pub mod ordered;
pub mod orderless;
pub mod prefix;

/// List like nodes
/// Basically can be classified as ordered and orderless
#[derive(Clone, Eq, PartialEq, Hash)]
pub enum ListView {
    /// Ordered list
    Ordered(Box<OrderedList>),
    /// Orderless list
    Orderless(Box<OrderlessList>),
}

impl ListView {
    /// Returns the children nodes of this list
    #[inline]
    pub fn children(&self) -> &Vec<ListItem> {
        match self {
            Self::Ordered(v) => &v.children,
            Self::Orderless(v) => &v.children,
        }
    }
    /// Returns the mutable children nodes of this list
    #[inline]
    pub fn children_mut(&mut self) -> &mut Vec<ListItem> {
        match self {
            Self::Ordered(v) => &mut v.children,
            Self::Orderless(v) => &mut v.children,
        }
    }
    /// Returns the first prefix of this list
    #[inline]
    pub fn first_prefix(&self) -> QResult<&ListPrefixMark> {
        match self.children().first() {
            None => Err(QError::runtime_error("Not a valid list")),
            Some(s) => Ok(&s.prefix.value),
        }
    }
    /// Check if this list is ordered
    #[inline]
    pub fn is_ordered(&self) -> bool {
        matches!(self, Self::Ordered(_))
    }
}
