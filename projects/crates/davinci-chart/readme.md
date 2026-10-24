# davinci-chart

Compile chart semantics into generic DaVinci vector scenes. The first slice covers Cartesian charts with six marks.

## Stages

The crate enforces the DaVinci layering contract as separate modules:

```text
DataSnapshot -> ChartSpec -> ChartCompilePlan -> SemanticScene -> DisplayScene
   data.rs       spec.rs        compile.rs         render.rs     davinci-scene resolver
                               scale.rs
                             statistic.rs
```

1. `data::DataSnapshot` is the source-independent observation store: named columns with explicit `FieldKind`
   declarations and row values. It validates itself before anything else consumes it.
2. `spec::ChartSpec` is the source-independent chart meaning: declared fields, one mark, field-to-channel bindings with
   optional aggregates, per-channel scale declarations, one coordinate system, and guides. Fields are bound by name,
   never by position.
3. `scale` and `statistic` are pure functions. Scale training turns a channel domain into a mapping. Statistics
   collapse observations before scales are trained.
4. `plan::ChartCompilePlan` is the deterministic compile result: canvas size, plot rectangle, trained scales, final
   mark geometry, axis ticks, and diagnostics.
5. `davinci_scene::SemanticScene` is the unresolved domain tree produced by `render::render` from a plan. It keeps
   mark instance ids, semantic roles, source references, and hit regions.
6. `davinci_scene::DisplayScene` is the resolved display tree produced by `SceneResolver`. It is the only input to
   `to_svg` or another renderer.

`lower` chains compilation and rendering into a `LoweredChart`.

Chart and diagram are parallel domains. Both depend on `davinci-types` and `davinci-scene`, and neither owns the
other's semantic model.

## Supported surface

- Data: `Quantitative`, `Temporal`, `Nominal`, and `Ordinal` columns. Values are `Null`, `Number`, `Text`, or
  `Timestamp`. Columnar and row-oriented data both reduce to the same snapshot.
- Marks: `Point`, `Bar`, `Line`, `Area`, `Rule`, and `Text`.
- Channels: `x`, `y`, `color`, `size`, and `shape`.
- Scales: `linear` with an inferred or explicit domain, `band` with padding, `point` with padding, and `nominal`
  for color and marker shape. Size channels always train a linear scale.
- Statistics: `count`, `sum`, `mean`, `min`, and `max`. At most one aggregated channel is supported, and aggregation
  runs at compile time, never during rendering.
- Guides: `x` and `y` axes. A guide list may be empty.

## Prohibited in a plan and a scene

- No `xField`, `yField`, `groupBy`, `stat`, `scale`, `svgPath`, `canvasCommand`, SVG string, or backend instruction.
  A plan may hold trained scales because that is compile output. A scene never does.
- No statistics, scale training, text measurement, or layout inside `render`; every coordinate comes from the plan.
- No document semantics, `notedown-ir` dependency, animation, interaction state, data binding, or responsive chart
  semantics.
- No silent loss. Fatal failures return a `ChartError` with a `davinci.chart.*` diagnostic; non-fatal loss and
  approximation are recorded in `ChartCompilePlan::diagnostics` and copied into `LoweredChart::diagnostics`.

## Determinism and degradation rules

`compile` is pure: no randomness, no hash-map iteration order, and rows and categories stay in first-appearance order.
Documented degradations are recorded as diagnostics:

- Missing values warn and affected rows are skipped. An aggregated measure keeps its group and reports the dropped
  group when no numeric value remains.
- A linear domain with one distinct value is expanded symmetrically and reported, so mapping never divides by zero or
  returns `NaN`.
- A quantitative `color` or `shape` channel is treated as a category.
- Legend guides and axis guides on non-position channels are reported, not drawn.
- A bar mark needs a band scale on one axis and a linear scale on the other.

## Diagnostics

Structural failures return `ChartError` with one of these codes:

`davinci.chart.empty-data`, `davinci.chart.ragged-column`, `davinci.chart.type-mismatch`, `davinci.chart.missing-value`,
`davinci.chart.non-finite-value`, `davinci.chart.duplicate-field`, `davinci.chart.unknown-field`,
`davinci.chart.invalid-spec`, `davinci.chart.invalid-scale`, `davinci.chart.empty-domain`,
`davinci.chart.degenerate-domain`, and `davinci.chart.approximated`.

## Out of scope in this slice

- Facets, layers, and multi-view composition.
- Logarithmic, power, and time scales.
- Polar, geographic, and map coordinates.
- Legends, annotations, titles, and tooltips.
- Stacking, dodging, and other positional adjustments.
- Interaction state and animation.

## Acceptance

`cargo test -p davinci-chart` runs data, spec, scale, compile, and render tests under `tests/`.
