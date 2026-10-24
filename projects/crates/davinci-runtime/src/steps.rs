//! Incremental compile steps derived from an invalidation set.
//!
//! [`derive_steps`] turns layers into an ordered pass list that does not depend
//! on any renderer backend. Passes run from upstream data to downstream paint:
//! recompile data, recompute the viewport, remeasure text, relayout, restyle,
//! retarget motion, repaint overlays. A host may skip a pass only when it has a
//! stronger reusable artifact than this crate can prove.

use crate::{InvalidationScope, InvalidationSet};

/// One incremental pass a host can run independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pass {
    /// Repaint interaction overlays without touching scene geometry.
    RepaintOverlay,
    /// Reapply paint attributes and interaction styling.
    Restyle,
    /// Recompute the scene-to-viewport projection.
    RecomputeViewport,
    /// Remeasure text and other cached content extents.
    Remeasure,
    /// Recompute node placement and scene bounds.
    Relayout,
    /// Reread data snapshots and the scales derived from them.
    RecompileData,
    /// Retarget transitions and timeline state.
    RetargetMotion,
}

/// Derive the ordered passes a layer set implies.
///
/// The input is closed under the implication rules documented on
/// [`InvalidationSet::closure`], so `Data` also yields `Remeasure` and
/// `Relayout`, and `Measurement` yields `Relayout`. `Layout`, `Viewport`,
/// `Style`, `Motion`, and `Overlay` imply nothing beyond their own pass.
///
/// A hover therefore produces only `RepaintOverlay`: no data read, no
/// measurement, no layout, and no transition planning.
#[must_use]
pub fn derive_steps(invalidation: &InvalidationSet) -> Vec<Pass> {
    let closed = invalidation.closure();
    let mut steps = Vec::new();
    for (scope, pass) in [
        (InvalidationScope::Data, Pass::RecompileData),
        (InvalidationScope::Viewport, Pass::RecomputeViewport),
        (InvalidationScope::Measurement, Pass::Remeasure),
        (InvalidationScope::Layout, Pass::Relayout),
        (InvalidationScope::Style, Pass::Restyle),
        (InvalidationScope::Motion, Pass::RetargetMotion),
        (InvalidationScope::Overlay, Pass::RepaintOverlay),
    ] {
        if closed.contains(scope) {
            steps.push(pass);
        }
    }
    steps
}
