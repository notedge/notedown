use super::*;

pub enum MathStyle {
    Display,
    Inline,
}

pub struct MathNode {
    pub style: MathStyle,
    pub math: String,
    pub span: Range<u32>,
}
