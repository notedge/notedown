use super::*;
use diagnostic_quick::error_3rd::NodeLocation;

/// ```md
/// \cmd[][](): args
/// ```
/// ```md
/// \cmd(
///     arg = 1
/// )
/// ```
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NormalCommand {
    /// The name of the atomics
    ///
    /// ## Example
    ///
    /// ```note
    /// \cmd
    /// ```
    pub cmd: String,
    /// The standard argument of the atomics
    ///
    /// ## Example
    ///
    /// ```note
    /// \cmd(arguments, options)
    /// ```
    pub options: CommandArguments,
    /// The pattern argument of the atomics
    ///
    /// ## Example
    ///
    /// ```note
    /// \cmd[p1][p2]
    /// ```
    pub pattern: Vec<NodeLocation<String>>,
    /// The body argument of the atomics
    ///
    /// ## Example
    ///
    /// ```note
    /// \cmd: body
    /// ```
    pub body: NodeLocation<String>,
}
