# davinci-motion

Transition planning between two DaVinci display scenes.

Correspondence is derived from stable scene ids, never from paint order. A plan records which nodes enter, update, or
exit, the channels that animate, timing, easing, and the interruption policy. Sampling a plan at time `t` returns a
display scene that a renderer can draw.

## Boundaries

- Depends on `davinci-types` and `davinci-scene` only.
- No data loading, layout, or layout invalidation.
- No renderer, no host clock, and no event handling; the host advances time.
- Static exports pick an explicit sample time instead of running a timeline.

## Acceptance

`cargo test -p davinci-motion` runs correspondence, interpolation, and interruption assertions under `tests/`.
