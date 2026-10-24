# davinci-types

Shared geometry, color, transform, and diagnostic primitives for the DaVinci diagram subsystem inside the Notedown
workspace.

## Boundaries

- No dependency on `notedown-ir`, Oak, VuTeX, or a renderer.
- `davinci-scene` and `davinci-diagram` build on these primitives.
- Diagnostics reuse the workspace `boost-diagnostic` model with stable `davinci.*` codes.

## Contents

- `Point`, `Size`, `Rect`, `Bounds`
- `Transform` (affine translate / scale / rotate)
- `Color` (RGBA, directly usable in CSS and SVG)
- `diagnostic`, `origin`, `codes`, and re-exported diagnostic records

## Acceptance

`cargo test -p davinci-types` runs the integration tests under `tests/`.
