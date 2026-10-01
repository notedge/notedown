use crate::nodes::*;

/// The Header node
///
/// ## Example
///
/// ```note
/// 
/// # level1
///
/// ### level3
///
/// \args(id = "custom-id")
/// ###### level6
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Header {
    /// The level of the header
    pub level: u8,
    /// Should hide the header in title of content
    pub hide_in_toc: bool,
    /// Force to set the header-id as string
    pub id: Option<String>,
    /// The content after `#` mark
    pub children: Vec<NotedownNode>,
}

impl Default for Header {
    fn default() -> Self {
        Self { level: 0, hide_in_toc: false, id: None, children: vec![] }
    }
}

impl Display for Header {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", "#".repeat(self.level as usize))?;
        for term in &self.children {
            write!(f, "{}", term)?
        }
        writeln!(f)?;
        Ok(())
    }
}

impl Header {
    /// Basic constructor
    #[inline]
    pub fn new(children: NotedownNodes, level: u8) -> Self {
        let mut new = Self::default();
        let id = children.slugify();
        new.children = children;
        new.id = Some(id);
        new.set_level(level);
        new
    }
    /// Level must between 1 - 6
    #[inline]
    pub fn set_level(&mut self, level: u8) -> &mut Self {
        let level = match level {
            n if n < 1 => 1,
            n if n > 6 => 6,
            n => n,
        };
        self.level = level;
        self
    }
    /// Id must slugify
    #[inline]
    pub fn set_id(&mut self, id: String) -> &mut Self {
        self.id = Some(id.slugify());
        self
    }
}

impl NotedownKind {
    /// Construct a header node
    #[inline]
    pub fn header(children: NotedownNodes, level: u8, span: &Span, file: &FileID) -> NotedownNode {
        Header::new(children, level).into_node(span, file)
    }
}
