#[cfg(feature = "html-ast")]
pub mod html;

use crate::hir::NotedownHIR;
use notedown_error::Validation;

/// Apply IR -> IR transformation
pub trait NoteTransformer {
    /// Optimize the given notedown AST
    fn transform(&mut self, ir: &NotedownHIR) -> Validation<NotedownHIR>;
}

/// Convert notedown to other formats
pub trait NoteGenerator {
    /// The output format
    type Output;
    /// Generate the output with the given notedown AST
    fn generate(&mut self, ir: &NotedownHIR) -> Validation<Self::Output>;
}

/// Component for rendering math formulas
pub trait MathEngine {
    /// Render the given math formula
    fn render(&mut self, raw: &str) -> Validation<NotedownHIR>;
}

/// Components for code highlighting
pub trait CodeEngine {
    /// Render the given code block
    fn render(&mut self, raw: &str) -> Validation<NotedownHIR>;
}
