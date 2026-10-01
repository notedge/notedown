use super::*;
use std::hash::{Hash, Hasher};

impl Debug for IgnoreNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WS(s) => Debug::fmt(s, f),
            Self::NL(s) => Debug::fmt(s, f),
        }
    }
}

impl From<TextSpaceNode> for IgnoreNode {
    fn from(value: TextSpaceNode) -> Self {
        Self::WS(value)
    }
}

impl Display for TextSpaceNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for _ in 0..self.width {
            f.write_char(' ')?
        }
        Ok(())
    }
}

impl Debug for NewlineSpan {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "NewlineNode({:?})", self.span)
    }
}

impl Debug for AlignNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AlignNode").field(&self.span).finish()
    }
}

impl Display for NewlineSpan {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_char('\n')
    }
}

impl From<NewlineSpan> for IgnoreNode {
    fn from(value: NewlineSpan) -> Self {
        Self::NL(value)
    }
}
impl Eq for HSpaceNode {}

impl PartialEq for HSpaceNode {
    fn eq(&self, other: &Self) -> bool {
        self.width.eq(&other.width) && self.span.start.eq(&other.span.start) && self.span.end.eq(&other.span.end)
    }
}

impl Hash for HSpaceNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.width.to_bits().hash(state);
        self.span.start.hash(state);
        self.span.end.hash(state);
    }
}

impl Eq for VSpaceNode {}

impl PartialEq for VSpaceNode {
    fn eq(&self, other: &Self) -> bool {
        self.height.eq(&other.height) && self.span.start.eq(&other.span.start) && self.span.end.eq(&other.span.end)
    }
}

impl Hash for VSpaceNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.height.to_bits().hash(state);
        self.span.start.hash(state);
        self.span.end.hash(state);
    }
}
