//! Oak Mermaid parser entry and source-level pipeline.

use oak_core::{Builder, SourceText, parser::session::ParseSession};
use oak_mermaid::{MermaidBuilder, MermaidLanguage, ast::MermaidRoot};

use crate::{
    error::DiagramError,
    flowchart::{LoweredDiagram, lower_mermaid},
};

/// Parse Mermaid source through the `oak-mermaid` frontend.
///
/// The frontend lowers flowchart sections into `MermaidRoot`. Diagrams outside
/// that subset, or source without any diagram header, produce an empty root.
/// This function rejects an empty AST with an explicit
/// `davinci.diagram.ast-unavailable` diagnostic instead of reporting a
/// successful parse with no semantics.
pub fn parse_mermaid(source: &str) -> Result<MermaidRoot, DiagramError> {
    let text = SourceText::new(source);
    let language = MermaidLanguage::default();
    let builder = MermaidBuilder::new(&language);
    let mut session = ParseSession::<MermaidLanguage>::default();
    let output = builder.build(&text, &[], &mut session);
    let oak_diagnostics = output.diagnostics.len();

    match output.result {
        Ok(root) if root.diagrams.is_empty() => Err(DiagramError::ast_unavailable(format!(
            "`oak-mermaid` accepted {} bytes with {oak_diagnostics} Oak parser diagnostics but produced no diagrams, only flowchart sections inside a `graph`/`flowchart` header are lowered",
            source.len()
        ))),
        Ok(root) => Ok(root),
        Err(error) => Err(DiagramError::parse_failed(format!("`oak-mermaid` failed to build an AST: {error}"))),
    }
}

/// Parse Mermaid source and lower the resulting AST into a DaVinci scene.
pub fn mermaid_to_scene(source: &str) -> Result<LoweredDiagram, DiagramError> {
    let root = parse_mermaid(source)?;
    lower_mermaid(&root)
}
