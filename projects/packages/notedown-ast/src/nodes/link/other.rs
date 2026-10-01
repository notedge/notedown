/// # EmailLink
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct EmailLink {
    /// Whether this email link is bare
    pub is_bare: bool,
    /// Name of the email link
    pub text: String,
}
