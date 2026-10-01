use super::*;

/// Prefix marks of the list
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ListPrefixMark {
    /// Something wrong
    Unknown,
    /// ```note
    /// -
    /// ```
    Hyphen,
    /// ```note
    /// >
    /// ```
    Quote,
    /// ```note
    /// >+ Summary Open
    /// ```
    SummaryOpen,
    /// ```note
    /// >- Summary Open
    /// ```
    SummaryClosed,
    /// Single, serial number from the beginning
    /// ```note
    /// 1.
    /// 2.
    /// 3.
    /// ```
    Arabic,
    /// Complex, multi-layered, not serial number from the beginning
    /// ```note
    /// 4.4.
    /// 4.5.
    /// 4.6.
    /// ```
    ArabicNest {
        /// Prefix number list
        prefix_number: Vec<usize>,
        /// Last part of prefix number
        number: usize,
    },
    /// ```note
    /// I.
    /// II.
    /// ```
    RomanNumerals,
}

impl Default for ListPrefixMark {
    fn default() -> Self {
        Self::Hyphen
    }
}

impl ListPrefixMark {
    /// Parse [`ListPrefixMark`] mark from string
    pub fn parse(input: &str) -> Self {
        match input {
            s if s.starts_with(">") => Self::Quote,
            _ => Self::Unknown,
        }
    }
}

impl ListPrefixMark {
    /// Check if this marks quote
    #[inline]
    pub fn is_quote(&self) -> bool {
        matches!(self, Self::Quote)
    }
    /// Check if this marks ordered list
    #[inline]
    pub fn is_ordered(&self) -> bool {
        matches!(self, Self::Arabic | Self::ArabicNest { .. })
    }
    /// Check if this marks orderless list
    #[inline]
    pub fn is_orderless(&self) -> bool {
        matches!(self, Self::Hyphen)
    }
}
