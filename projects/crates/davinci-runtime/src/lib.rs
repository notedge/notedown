#![deny(missing_docs)]
//! Runtime state, event normalization, reducers, and invalidation for DaVinci
//! scenes.
//!
//! This crate owns mutable interaction state. It never mutates a canonical
//! graphic spec, never lays out geometry, and never renders.
//!
//! The interaction pipeline is:
//!
//! ```text
//! raw input -> NormalizedEvent -> hit test -> Action -> reduce -> RuntimeState
//!            -> invalidate -> InvalidationSet
//! ```
//!
//! [`hit_test`] resolves a scene point to the topmost semantic id,
//! [`normalize`] turns host input into semantic [`Action`] values, [`reduce`]
//! applies one action to a [`RuntimeState`] without touching its input, and
//! [`invalidate`] reports which layers a change disturbs, and [`reduce_update`]
//! combines the state transition with that layer set.
//!
//! The invalidation layer is what keeps interaction incremental. Hover, focus,
//! press, and selection changes stay on [`InvalidationScope::Overlay`], zoom
//! and pan stay on [`InvalidationScope::Viewport`], playback stays on
//! [`InvalidationScope::Motion`], and only data or container changes reach the
//! expensive layout chain. No action reports every layer.
//!
//! Scenes stay immutable. Hover, focus, press, selection, viewport, parameters,
//! theme, and playback live only in [`RuntimeState`], so interaction can never
//! pollute document semantics. Conversely, runtime fields are never written
//! into `davinci_scene::SceneNode`, which only carries resolved geometry and
//! display attributes.

mod action;
mod event;
mod hit_test;
mod invalidation;
mod normalize;
mod reduce;
mod state;

pub use action::Action;
pub use event::NormalizedEvent;
pub use hit_test::{HitTarget, hit_test};
pub use invalidation::{Invalidation, InvalidationScope, InvalidationSet, invalidate};
pub use normalize::normalize;
pub use reduce::{RuntimeUpdate, reduce, reduce_update};
pub use state::{MAX_SCALE, MIN_SCALE, Playback, RuntimeState, Viewport};
