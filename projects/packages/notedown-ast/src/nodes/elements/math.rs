use super::*;

/// Supported math modes
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MathKind {
    /// Math surround by `$` inline
    Inline,
    /// Math surround by `$$` inline
    Display,
    /// Math surround by `$` blocked
    BlockInline,
    /// Math surround by `$$` blocked
    BlockDisplay,
}

/// Supported math backends
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MathBackend {
    ///
    LaTeX = 0,
    ///
    AsciiMath,
    ///
    MathML,
}

///
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct MathNode {
    ///
    pub kind: MathKind,
    ///
    pub raw: String,
    ///
    pub format: MathBackend,
}

impl Default for MathBackend {
    fn default() -> Self {
        Self::LaTeX
    }
}

impl Default for MathNode {
    fn default() -> Self {
        Self { kind: MathKind::BlockDisplay, raw: String::new(), format: Default::default() }
    }
}

impl Display for MathNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.surround(f)
    }
}

impl MathKind {
    /// Surround start
    pub fn surround_begin(&self) -> &'static str {
        match self {
            Self::Inline => "$",
            Self::Display => "$$",
            Self::BlockInline => "\n\n$",
            Self::BlockDisplay => "\n\n$$",
        }
    }
    /// Surround end
    pub fn surround_end(&self) -> &'static str {
        match self {
            Self::Inline => "$",
            Self::Display => "$$",
            Self::BlockInline => "$\n\n",
            Self::BlockDisplay => "$$\n\n",
        }
    }
}

impl MathNode {
    /// surrounded
    pub fn surround(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.kind.surround_begin())?;
        f.write_str(&self.raw)?;
        f.write_str(self.kind.surround_end())?;
        Ok(())
    }
}

impl MathNode {
    /// Parse given format string
    /// do nothing if parse failed
    pub fn set_format(&mut self, s: &str) -> &mut Self {
        MathBackend::new(s).map(|f| self.format = f);
        self
    }
}

impl MathBackend {
    /// Parse math backend form string
    pub fn new(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "tex" | "latex" => Some(Self::LaTeX),
            "ascii" => Some(Self::AsciiMath),
            _ => None,
        }
    }
}
