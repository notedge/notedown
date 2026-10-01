use super::*;

/// ````md
/// ```cmd(arg=1)
/// body ast
/// ```
/// ````
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EscapedCommand {
    pub cmd: String,
    pub level: u8,
    pub start: usize,
    pub end: usize,
    // pub options: CommandArguments,
    // pub pattern: Vec<NodeLocation<String>>,
    // pub body: NodeLocation<String>,
}
