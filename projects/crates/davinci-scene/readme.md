# davinci-scene

A deterministic vector scene model and SVG writer for the DaVinci diagram subsystem inside the Notedown workspace.

## Boundaries

- Depends only on `davinci-types`.
- No dependency on diagram semantics, `notedown-ir`, Oak, or VuTeX.
- No layout, text wrapping, interaction animation, or data binding.
- Scene coordinates are explicit. A host layout engine places the canvas.

## Scene stages

- `SemanticScene`, `SemanticNode`, and `SemanticKind` form the unresolved domain identity tree. Nodes keep a stable
  `id`, `SemanticRole`, `SourceRef`, `hit_region`, image `ResourceRef`, an unresolved local `Transform`, an unresolved
  local `clip`, and the original group nesting.
- `SceneResolver::resolve` composes nested transforms into absolute node transforms, flattens nested groups into
  identity containers, inherits and intersects clips, and keeps paint order. Resolution is pure, deterministic, and
  performs no I/O.
- `DisplayScene`, `SceneNode`, and the node kinds `Group`, `Path`, `Line`, `Rect`, `RoundedRect`, `Ellipse`, `Text`,
  `Image`, and `Marker` are the resolved, backend-ready tree. Renderers consume only this stage.
- Per-node provenance metadata survives resolution: stable `id`, `SemanticRole`, `SourceRef`, `hit_region`, and
  `ResourceRef` for images. Metadata drives hit testing, accessibility, and motion correspondence and never changes
  geometry or paint order.
- Styles limited to fill, stroke, stroke width, opacity, font family, font size, font weight, and text anchor.
- `DisplayScene::to_svg` with fixed attribute order and stable number formatting. Semantic metadata is emitted as
  `data-role`, `data-source`, `data-source-range`, and `data-mime`.

## Acceptance

`cargo test -p davinci-scene` runs bounds, transform, marker, text, resolution, and SVG golden assertions under
`tests/`.
