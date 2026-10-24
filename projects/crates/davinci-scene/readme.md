# davinci-scene

A deterministic generic vector scene model and SVG writer for the DaVinci diagram subsystem inside the Notedown
workspace.

## Boundaries

- Depends only on `davinci-types`.
- No dependency on diagram semantics, `notedown-ir`, Oak, or VuTeX.
- No layout, text wrapping, interaction, animation, or data binding.
- Scene coordinates are explicit. A host layout engine places the canvas.

## Contents

- `Scene`, `SceneNode`, and the node kinds `Group`, `Path`, `Line`, `Rect`, `RoundedRect`, `Ellipse`, `Text`, `Image`,
  and `Marker`.
- Per-node `Transform`, axis-aligned `clip`, and computed bounds.
- Styles limited to fill, stroke, stroke width, opacity, font family, font size, font weight, and text anchor.
- `Scene::to_svg` with fixed attribute order and stable number formatting.

## Acceptance

`cargo test -p davinci-scene` runs bounds, transform, marker, text, and SVG golden assertions under `tests/`.
