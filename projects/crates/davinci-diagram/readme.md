# davinci-diagram

Lower diagram semantics into `davinci-scene` values. The first slice covers Oak Mermaid flowcharts.

## Boundaries

- Depends on `davinci-scene` and `davinci-types`, never on `notedown-ir` or VuTeX.
- No animation, interaction state, data binding, axes, statistical transforms, or responsive chart semantics.
- Layout is deliberately simple and deterministic: declaration-ordered layers with centered cross-axis placement.
- Every construct that cannot be represented raises a `DiagramError` or records a loss diagnostic. Nothing is silently
  dropped.

## Oak Mermaid status

The pinned Oak revision `351b173259156f5bf0652daf6aa797378758a971` ships `oak-mermaid` with AST types and a parser
entry, but its builder does not lower green tree nodes into `MermaidRoot`: the lexer emits error tokens and the builder
returns an empty diagram list. `parse_mermaid` therefore reports an explicit `davinci.diagram.ast-unavailable`
diagnostic instead of pretending to parse. The flowchart lowering itself is real and is tested against the typed
`oak_mermaid::ast::Flowchart` model that a working frontend produces.

## Supported lowering

- Flowchart nodes with `Box`, `Round`, `Stadium`, `Subroutine`, `Cylinder`, `Circle`, and `Rhombus` shapes.
- Edges with arrow, plain, dotted, and thick styles, plus edge labels.
- Directions `TD`/`TB`, `BT`, `LR`, and `RL`.
- Deterministic layered placement, including explicit cycle diagnostics.

Subgraphs are not represented because the pinned `oak-mermaid` AST has no grouping construct. Sequence and class
diagrams are rejected explicitly.

## Acceptance

`cargo test -p davinci-diagram` runs parser, lowering, layout, and AST-to-SVG cases under `tests/`.
