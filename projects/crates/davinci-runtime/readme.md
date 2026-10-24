# davinci-runtime

Runtime state and invalidation for DaVinci scenes.

The crate keeps mutable interaction state outside the canonical graphic model. It normalizes host input, resolves a
semantic target by hit testing a `davinci_scene::Scene`, reduces actions into a new `RuntimeState`, and reports which
compile scopes a change invalidates.

## Boundaries

- Depends on `davinci-types` and `davinci-scene` only.
- Holds no document semantics, no graphic compiler, and no renderer.
- Never mutates the canonical spec: interaction only changes runtime state.
- No animation evaluation; motion planning lives in `davinci-motion`.

## Acceptance

`cargo test -p davinci-runtime` runs state, reducer, hit testing, and invalidation assertions under `tests/`.
