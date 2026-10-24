# davinci-diagram

Lower diagram semantics into `davinci-scene` values. The first slice covers Oak Mermaid flowcharts.

## Boundaries

- Depends on `davinci-scene` and `davinci-types`, never on `notedown-ir` or VuTeX.
- No animation, interaction state, data binding, axes, statistical transforms, or responsive chart semantics.
- Layout is deliberately simple and deterministic: declaration-ordered layers with centered cross-axis placement.
- Every construct that cannot be represented raises a `DiagramError` or records a loss diagnostic. Nothing is silently
  dropped.

## Oak Mermaid status

`oak-mermaid` lowers flowchart sections introduced by a `graph`/`flowchart` header into `MermaidRoot`. Diagram
families outside that subset (sequence, class, state, and similar headers) are skipped by the frontend, so
`parse_mermaid` reports an explicit `davinci.diagram.ast-unavailable` diagnostic for a root without diagrams
instead of pretending to parse.

## Supported lowering

- Flowchart nodes with `Box`, `Round`, `Stadium`, `Subroutine`, `Cylinder`, `Circle`, and `Rhombus` shapes.
- Edges with arrow, plain, dotted, and thick styles, plus edge labels.
- Directions `TD`/`TB`, `BT`, `LR`, and `RL`.
- Deterministic layered placement, including explicit cycle diagnostics.

Subgraphs are not represented because the pinned `oak-mermaid` AST has no grouping construct. Sequence and class
diagrams are rejected explicitly.

## Acceptance

`cargo test -p davinci-diagram` runs parser, lowering, layout, and AST-to-SVG cases under `tests/`.
