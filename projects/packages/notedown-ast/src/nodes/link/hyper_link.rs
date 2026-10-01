use super::*;

/// # HyperLink
#[derive(Clone, Default, Eq, PartialEq)]
pub struct HyperLink {
    /// ## Bare Link
    /// A link without the `[ ]`
    pub is_bare: bool,
    /// ## Real Link
    /// A normal link without any description
    /// ```note
    /// [path-of-link]
    /// ```
    pub src: String,
    /// ## Description
    /// A normal link without any description
    /// ```note
    /// [alt-ast: link]
    /// ```
    pub text: Option<String>,
    /// Click this link to begin downloading
    pub download: Option<String>,
    /// Click this link to jumping to target
    pub target: HyperLinkTarget,
    /// Additional information for the link
    pub options: Option<CommandArguments>,
}

/// HyperLink target type
/// [HTML/Element/a#attr-target](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/a#attr-target)
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum HyperLinkTarget {
    /// the current browsing context
    This,
    /// usually a new tab, but users can configure browsers to open a new window
    /// instead.
    Blank,
    /// the parent browsing context of the current one. If no parent, behaves as
    /// _self.
    Parent,
    /// the topmost browsing context (the "highest" context that’s an ancestor
    /// of the current one). If no ancestors, behaves as _self.
    Top,
}

impl Default for HyperLinkTarget {
    fn default() -> Self {
        Self::This
    }
}

impl HyperLink {
    /// Set description ast for the link
    #[inline]
    pub fn with_text(mut self, msg: impl Into<String>) -> Self {
        self.text = Some(msg.into());
        self
    }
    /// Parse arguments from arguments
    pub fn parse_options(mut self) -> Self {
        let args = match &mut self.options {
            None => return self,
            Some(s) => s,
        };
        args.optional.get_value("ast").map(|f| self.with_text(f));
        return self;
    }
}
