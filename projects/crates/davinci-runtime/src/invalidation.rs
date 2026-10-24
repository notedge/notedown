//! Compile-scope invalidation derived from semantic actions.

use std::collections::BTreeSet;

use crate::Action;

/// A layer of derived work that an action can disturb.
///
/// Declarations are ordered so [`BTreeSet`] iteration and diagnostics stay
/// deterministic across runs. The order is not a cost ranking: a host decides
/// on its own which scopes it can rebuild independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InvalidationScope {
    /// Painted interaction overlays such as hover halos, selection rings, and
    /// handles that are drawn above the scene.
    Overlay,
    /// Paint attributes and interaction styling of scene nodes.
    Style,
    /// The viewport projection, including zoom and translation.
    Viewport,
    /// Text measurement and other cached content extents.
    Measurement,
    /// Node placement, scene bounds, and any geometry cache.
    Layout,
    /// Data snapshots and the scales derived from them.
    Data,
    /// Time-based animation and transition state.
    Motion,
}

/// The invalidation result of one action.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Invalidation {
    /// Layers that must be rebuilt.
    pub scopes: BTreeSet<InvalidationScope>,
    /// Semantic ids affected inside those layers.
    ///
    /// An empty set means the scope as a whole is dirty but the action alone
    /// cannot name a semantic target, for example [`Action::Release`], which
    /// does not carry the previously pressed id.
    pub targets: BTreeSet<String>,
}

impl Invalidation {
    /// Returns `true` when no layer has to be rebuilt.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.scopes.is_empty()
    }

    /// Returns `true` when the scope is dirty.
    #[must_use]
    pub fn contains(&self, scope: InvalidationScope) -> bool {
        self.scopes.contains(&scope)
    }
}

/// Report the compile scopes an action invalidates.
///
/// Pointer state changes stay on the style and overlay layers: hovering,
/// focusing, pressing, and selecting never relayout or reread data. Only
/// geometry-affecting actions reach the expensive scopes.
///
/// | Action | Scopes | Targets |
/// |--------|--------|---------|
/// | `Hover` | style, overlay | the hovered id |
/// | `Focus` | style | the focused id |
/// | `Press` / `Release` | style, overlay | the pressed id when known |
/// | `Select` / `ClearSelection` | style, overlay | the selected id when known |
/// | `Zoom` / `Pan` | viewport | none |
/// | `Resize` | layout, measurement | none |
/// | `SetParameter` | data, layout, motion | none |
/// | `Play` / `Pause` / `Seek` / `Advance` | motion | none |
#[must_use]
pub fn invalidate(action: &Action) -> Invalidation {
    let mut invalidation = Invalidation::default();
    match action {
        Action::Hover(target) => {
            invalidation.scopes.extend([InvalidationScope::Style, InvalidationScope::Overlay]);
            extend_target(&mut invalidation, target);
        }
        Action::Focus(target) => {
            invalidation.scopes.insert(InvalidationScope::Style);
            extend_target(&mut invalidation, target);
        }
        Action::Press(target) => {
            invalidation.scopes.extend([InvalidationScope::Style, InvalidationScope::Overlay]);
            extend_target(&mut invalidation, target);
        }
        Action::Release => {
            invalidation.scopes.extend([InvalidationScope::Style, InvalidationScope::Overlay]);
        }
        Action::Select { target, .. } => {
            invalidation.scopes.extend([InvalidationScope::Style, InvalidationScope::Overlay]);
            invalidation.targets.insert(target.clone());
        }
        Action::ClearSelection => {
            invalidation.scopes.extend([InvalidationScope::Style, InvalidationScope::Overlay]);
        }
        Action::Zoom { .. } | Action::Pan { .. } => {
            invalidation.scopes.insert(InvalidationScope::Viewport);
        }
        Action::Resize { .. } => {
            invalidation.scopes.extend([InvalidationScope::Layout, InvalidationScope::Measurement]);
        }
        Action::SetParameter { .. } => {
            invalidation.scopes.extend([InvalidationScope::Data, InvalidationScope::Layout, InvalidationScope::Motion]);
        }
        Action::Play | Action::Pause | Action::Seek(_) | Action::Advance(_) => {
            invalidation.scopes.insert(InvalidationScope::Motion);
        }
    }
    invalidation
}

/// Record an optional semantic target.
fn extend_target(invalidation: &mut Invalidation, target: &Option<String>) {
    if let Some(target) = target {
        invalidation.targets.insert(target.clone());
    }
}
