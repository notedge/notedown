use super::*;
use crate::NotedownNodes;

/// Marks position of [`XMLCommand`]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum XMLCommandMarks {
    /// This is a open-close xml
    OpenClose {
        /// `|` `<cmd`
        start: usize,
        /// `|` `>`
        middle: usize,
        /// `|` `</cmd>`
        end: usize,
    },
    /// This is a self-close xml
    SelfClose {
        /// <cmd
        start: usize,
        /// </cmd>
        end: usize,
    },
}

/// ## XMLCommand
/// a atomics on XML Syntax
/// ### OpenClose
/// ```md
/// <cmd[][] arg=1>body ast</cmd>
/// ```
/// ### SelfClose
/// ```md
/// <cmd[][] arg=1/>
/// ```
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct XMLCommand {
    /// Command name
    pub cmd: String,
    /// Command marks position
    pub kind: XMLCommandMarks,
    /// Command options
    pub options: CommandArguments,
    /// Command body
    pub body: NotedownNodes,
}

impl XMLCommandMarks {
    /// Start range of Command
    pub fn start_range(&self, name: &str) -> Range<usize> {
        match self {
            | Self::OpenClose { start, middle: _, end: _ } // \n
            | Self::SelfClose { start, end: _ } => {
                Range { start: *start, end: start + 1 + name.len() }
            }
        }
    }
    /// End range of Command
    pub fn end_range(&self, name: &str) -> Range<usize> {
        match self {
            | Self::OpenClose { start: _, middle: _, end } // \n
            | Self::SelfClose { start: _, end } => {
                Range { start: *end, end: end + 1 + name.len() }
            }
        }
    }
}

impl XMLCommand {
    /// Constructor of [`XMLCommand`]
    pub fn open_close(cmd: String, body: NotedownNodes, options: CommandArguments) -> Self {
        let kind = XMLCommandMarks::OpenClose { start: 0, middle: 0, end: 0 };
        Self { cmd, kind, options, body }
    }
    /// Constructor of [`XMLCommand`]
    pub fn self_close(cmd: String, options: CommandArguments) -> Self {
        let kind = XMLCommandMarks::SelfClose { start: 0, end: 0 };
        Self { cmd, kind, options, body: vec![] }
    }
}

impl XMLCommand {
    /// Edit event of atomics name
    pub fn modify_name(&mut self, new: String) -> (Range<usize>, Range<usize>) {
        let out = (self.kind.start_range(&self.cmd), self.kind.end_range(&self.cmd));
        self.cmd = new;
        return out;
    }
}
