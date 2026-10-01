/// TagReference
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TagReference {
    /// ## Tag Block
    /// ```note
    /// [^tag]: ast ast ast
    /// [^tag]:
    ///     ast ast ast
    ///     ast ast ast
    /// ```
    pub inline: bool,
    /// ## Tag Reference
    /// Quote a number, note that the name is irrelevant, and a number will always be generated in sequence at the end
    /// ```note
    /// ast [^tag] ast ast ast
    /// ```
    pub tag: String,
    /// ## Tag Inline
    /// Quote a number while defining
    /// ```note
    /// ast [^tag][ast ast ast] ast
    /// ```
    pub text: Vec<String>,
}
