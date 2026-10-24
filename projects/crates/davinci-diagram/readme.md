# davinci-diagram

Compile diagram semantics into generic DaVinci vector scenes. The first slice covers Oak Mermaid flowcharts.

## Three stages

The crate enforces the DaVinci layering contract as separate modules:

```text
DiagramSpec -> DiagramLayoutPlan -> Scene
  spec.rs          plan.rs          render.rs
                compile.rs
```

1. `spec::DiagramSpec` is the source-independent diagram meaning: direction, nodes, edges, ports, clusters, and
   ordering constraints. A source frontend lowers into this model.
2. `plan::DiagramLayoutPlan` is the deterministic compile result: canvas size, placed node rectangles, orthogonal edge
   routes, cluster frames, and diagnostics. A plan carries geometry only.
3. `davinci_scene::Scene` is the display scene produced by `render::render` from a plan.

The Mermaid frontend in `parse.rs` produces a `DiagramSpec`. It never assembles SVG, chooses coordinates, or creates
scene nodes.

## Prohibited in a plan and a scene

- No `svgPath`, `canvasCommand`, SVG string, drawing command, or backend instruction.
- No scales, axes, or statistical transforms.
- No layout inside `render`; every coordinate comes from the plan.
- No document semantics, `notedown-ir` dependency, animation, interaction state, data binding, or responsive chart
  semantics.
- No silent loss. Fatal failures return a `DiagramError` with a `davinci.*` diagnostic; non-fatal loss and
  approximation are recorded in `DiagramLayoutPlan::diagnostics` and copied into `LoweredDiagram::diagnostics`.

## Determinism and degradation rules

`compile` is pure: no randomness, no hash-map iteration order, and nodes stay in spec declaration order. Documented
degradations are recorded as diagnostics:

- A missing endpoint node skips the edge; a self edge is skipped.
- An unknown port falls back to the default anchor for the layout direction.
- Duplicate node ids keep the first definition and merge later port declarations.
- Unknown cluster references, missing cluster members, and empty clusters are reported; unusable members are ignored.
- Cycles are broken in declaration order with a deterministic layer assignment.
- Diamond shapes and dotted edges are approximated by the scene surface and reported.
- `Subroutine` nodes are drawn as plain rectangles and reported.

## Oak Mermaid status

`oak-mermaid` lowers flowchart sections introduced by a `graph`/`flowchart` header into `MermaidRoot`. Diagram families
outside that subset (sequence, class, state, and similar headers) are skipped by the frontend, so `parse_mermaid`
reports an explicit `davinci.diagram.ast-unavailable` diagnostic for a root without diagrams instead of pretending to
parse.

## Supported lowering

- Flowchart nodes with `Box`, `Round`, `Stadium`, `Subroutine`, `Cylinder`, `Circle`, and `Rhombus` shapes.
- Edges with arrow, plain, dotted, and thick styles, plus edge labels.
- Directions `TD`/`TB`, `BT`, `LR`, and `RL`.
- `subgraph` blocks become clusters. A node joins every enclosing block, so an outer frame still covers nested members.
  Nested blocks are flattened into sibling clusters and the lost containment is reported as a warning.
- Deterministic layered placement, including explicit cycle diagnostics.

Sequence and class diagrams are rejected explicitly.

## Acceptance

`cargo test -p davinci-diagram` runs spec, layout, render, and Mermaid tests under `tests/`.
