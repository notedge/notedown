# davinci-diagram

Compile diagram semantics into generic DaVinci vector scenes. The current slice covers Oak Mermaid flowcharts and
sequence diagrams.

## Four stages

The crate enforces the DaVinci layering contract as separate modules:

```text
DiagramSpec -> DiagramLayoutPlan -> SemanticScene -> DisplayScene
  spec.rs          plan.rs          render.rs        davinci-scene resolver
                compile.rs
```

1. `spec::DiagramSpec` is the source-independent diagram meaning: direction, nodes, edges, ports, clusters, and
   ordering constraints. A source frontend lowers into this model.
2. `plan::DiagramLayoutPlan` is the deterministic compile result: canvas size, placed node rectangles, orthogonal edge
   routes, cluster frames, and diagnostics. A plan carries geometry only.
3. `davinci_scene::SemanticScene` is the unresolved domain tree produced by `render::render` from a plan. It keeps
   ids, semantic roles, source references, hit regions, and unresolved local transforms.
4. `davinci_scene::DisplayScene` is the resolved display tree produced by `SceneResolver`. It is the only input to
   `to_svg` or another renderer.

The Mermaid frontend in `parse.rs` produces a `DiagramSpec`. It never assembles SVG, chooses coordinates, or creates
scene nodes.

## Prohibited in a plan and a scene

- No `svgPath`, `canvasCommand`, SVG string, drawing command, or backend instruction.
- No `xField`, `yField`, `groupBy`, `stat`, `scale`, `nodeId`, or `edgeKind` fields.
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

`oak-mermaid` lowers `graph`/`flowchart` and `sequenceDiagram` sections into `MermaidRoot`. State, ER, and similar
headers still produce no diagram, so `parse_mermaid` reports an explicit `davinci.diagram.ast-unavailable` diagnostic
instead of pretending to parse. `classDiagram` parses but is not lowered yet.

## Supported lowering

- Flowchart nodes with `Box`, `Round`, `Stadium`, `Subroutine`, `Cylinder`, `Circle`, and `Rhombus` shapes.
- Edges with arrow, plain, dotted, dotted-line, and thick styles, plus edge labels.
- Directions `TD`/`TB`, `BT`, `LR`, and `RL`.
- `subgraph` blocks become clusters. A node joins every enclosing block, so an outer frame still covers nested members.
  Nested blocks are flattened into sibling clusters and the lost containment is reported as a warning.
- Deterministic layered placement, including explicit cycle diagnostics.
- `sequenceDiagram` participants and `actor` aliases, `A->>B`, `A-->>B`, `A->B`, and `A-->B` messages, and implicit
  participants that appear only as message endpoints.
- Sequence layout is time-ordered: participant header boxes are measured from their labels, messages occupy one row
  each, the canvas covers all columns and rows plus the margin, and lifelines are decoration from each header to the
  canvas bottom. Dotted messages are drawn solid with an approximation diagnostic. Self messages and undeclared
  participants in a hand-built spec are skipped with diagnostics.
- Sequence statements outside that subset (`Note`, `loop`, `alt`, `activate`, `autonumber`, and similar) fail with
  `davinci.diagram.unsupported` before compilation, so the host keeps the authored source visible instead of dropping
  the diagram silently.
- `mermaid_to_spec` stays the flowchart graph spec entry point. Sequence diagrams lower through `mermaid_to_scene`.

## Acceptance

`cargo test -p davinci-diagram` runs spec, layout, render, and Mermaid tests under `tests/`.
