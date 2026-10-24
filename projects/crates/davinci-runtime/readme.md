# davinci-runtime

Runtime state and invalidation for DaVinci scenes.

The crate keeps mutable interaction state outside the canonical graphic model. It normalizes host input, resolves a
semantic target by hit testing a `davinci_scene::DisplayScene`, reduces actions into a new `RuntimeState`, and reports which
layers a change invalidates.

`RuntimeState` carries hover, focus, press, selection, viewport zoom and pan, named parameters, the selected theme,
container size, playback time, and the playing flag. The canonical scene is never written.

## Invalidation layers

`reduce_update` returns the next state plus an `InvalidationSet`. A layer is one kind of derived work:

| Layer | Triggered by |
|-------|--------------|
| `Overlay` | hover, focus, press, release, selection changes |
| `Style` | theme or style preset changes |
| `Viewport` | zoom and pan |
| `Measurement` | container resize, or implied by data |
| `Layout` | container resize, or implied by data and measurement |
| `Data` | parameter, filter, and brush changes |
| `Motion` | play, pause, seek, and time advance |

Implication is explicit, not blanket propagation: `Data` implies `Measurement`, and `Measurement` implies `Layout`.
`Layout` implies nothing, and `Viewport`, `Style`, `Motion`, and `Overlay` imply nothing. Pointer actions therefore stay on
`Overlay`, zoom and pan stay on `Viewport`, and playback stays on `Motion`.

`reduce_update` also returns the ids whose pointer or selection membership changed. No-op actions report an empty set, so
hovering the already hovered id costs nothing.

## Node narrowing

`invalidated_nodes` narrows a set against a `DisplayScene`, and `invalidated_semantic_nodes` does the same against a
`SemanticScene`. Overlay invalidation resolves to targeted ids, nodes sharing the target node's source element, and
highlight ids prefixed by `target/` or `target:`. Style reaches every identified node and measurement only text nodes.
Data reaches data roles and nodes with a source reference.

Narrowing is honest about its limits. `Layout`, `Viewport`, and `Motion` are reported in `whole_layers` because placement
depends on global bounds, projection is global, and naming motion nodes needs a transition plan. A node without an id also
forces its layer whole. `recompute` and `retained` are content claims only, and `retained` never claims placement reuse.

## Incremental passes

`derive_steps` turns a layer set into an ordered, backend-neutral pass list: recompile data, recompute viewport,
remeasure, relayout, restyle, retarget motion, repaint overlay. A hover produces only `RepaintOverlay`, so no data read,
measurement, layout, or transition planning happens for pointer movement.

## Boundaries

- Depends on `davinci-types` and `davinci-scene` only.
- Holds no document semantics, no graphic compiler, and no renderer.
- Never mutates the canonical spec: interaction only changes runtime state.
- No animation evaluation; motion planning lives in `davinci-motion`.

## Acceptance

`cargo test -p davinci-runtime` runs state, reducer, hit testing, invalidation, node narrowing, and pass orchestration
assertions under `tests/`.
