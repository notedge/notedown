//! Oak Mermaid parser entry and source-level pipeline.

use oak_core::{Builder, SourceText, parser::session::ParseSession};
use oak_mermaid::{MermaidBuilder, MermaidLanguage, ast::MermaidRoot};

use crate::{
    error::DiagramError,
    flowchart::{LoweredDiagram, lower_mermaid},
};

/// Parse Mermaid source through the pinned `oak-mermaid` frontend.
///
/// The pinned Oak revision ships AST types and a parser entry, but its builder
/// does not lower green tree nodes into `MermaidRoot` and returns an empty
/// diagram list for every input. This function therefore rejects an empty AST
/// with an explicit `davinci.diagram.ast-unavailable` diagnostic instead of
/// reporting a successful parse with no semantics.
pub fn parse_mermaid(source: &str) -> Result<MermaidRoot, DiagramError> {
    let text = SourceText::new(source);
    let language = MermaidLanguage::default();
    let builder = MermaidBuilder::new(&language);
    let mut session = ParseSession::<MermaidLanguage>::default();
    let output = builder.build(&text, &[], &mut session);
    let oak_diagnostics = output.diagnostics.len();

    match output.result {
        Ok(root) if root.diagrams.is_empty() => Err(DiagramError::ast_unavailable(format!(
            "`oak-mermaid` at rev 351b173259156f5bf0652daf6aa797378758a971 accepted {} bytes with {oak_diagnostics} Oak parser diagnostics but produced no diagrams because its builder does not lower green nodes into `MermaidRoot`",
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
