#![deny(missing_docs)]
//! Stable correspondence and transition planning between DaVinci display
//! scenes.
//!
//! Motion consumes two already-laid-out scenes plus a [`MotionIntent`] and
//! never loads data, lays out geometry, renders, or owns a clock:
//!
//! ```text
//! previous scene + next scene + stable ids + motion intent
//!   -> correspondence -> transition plan -> sample at time t -> display scene
//! ```
//!
//! Correspondence is derived only from
//! [`SceneNode::id`](davinci_scene::SceneNode::id) values, matched across the
//! whole node tree including groups. Paint order, array indices, and geometry
//! never decide which node continues into which: reordering or filtering a
//! scene therefore cannot produce a node flying in from the wrong place.
//!
//! Every result is deterministic. The implementation walks scenes in document
//! order and uses hash maps only for lookups, so equal inputs always produce
//! equal output regardless of hash iteration order.
//!
//! # Known boundaries
//!
//! - Only `Transform` and `Opacity` are animated. Path geometry, rect extents,
//!   text content, and colors are snapped to the next scene, so shape morphing
//!   and color interpolation are intentionally out of scope.
//! - `Transform` interpolation is component-wise on the affine matrix. It does
//!   not decompose rotation or scale, and it never produces a different matrix
//!   than the documented component lerp.
//! - Nodes without an `id` never correspond. A node present only in the
//!   previous scene without an `id` is not drawn by [`sample`].
//! - Sampling builds its tree from the next scene; nodes that exit are
//!   reattached as documented on [`sample`].

mod easing;
mod internal;
mod plan;
mod sample;
mod timeline;

pub use easing::Easing;
pub use plan::{
    Channel, ChannelValue, Correspondence, CorrespondenceKind, EnterPolicy, ExitPolicy, InterruptionPolicy, MotionIntent, Track,
    TransitionPlan, match_scenes, plan_transition,
};
pub use sample::sample;
pub use timeline::Timeline;
