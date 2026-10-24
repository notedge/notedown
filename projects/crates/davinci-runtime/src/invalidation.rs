//! Dependency invalidation derived from semantic actions.
//!
//! An action never rewrites the canonical graphic. It changes
//! [`RuntimeState`](crate::RuntimeState) and reports which derived layers must
//! be recomputed. Layers are deliberately coarse: an [`InvalidationSet`] states
//! *what kind of work* is stale, while narrowing the set to actual scene nodes
//! is a separate step resolved against scene identity.

use std::collections::BTreeSet;

use crate::Action;

/// A layer of derived work that an action can disturb.
///
/// Declarations are ordered so [`BTreeSet`] iteration and diagnostics stay
/// deterministic across runs. The order is not a cost ranking: a host decides
/// on its own which layers it can rebuild independently.
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

/// An unordered set of invalidated layers.
///
/// The set supports multiple simultaneously dirty layers, union, and
/// membership queries. It is the unit hosts use to decide which incremental
/// passes to run instead of recompiling the whole tree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InvalidationSet {
    scopes: BTreeSet<InvalidationScope>,
}

impl InvalidationSet {
    /// Create an empty set.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Create a set from any sequence of layers.
    pub fn of(scopes: impl IntoIterator<Item = InvalidationScope>) -> Self {
        Self { scopes: scopes.into_iter().collect() }
    }

    /// Add a layer. Returns `true` when it was not already present.
    pub fn insert(&mut self, scope: InvalidationScope) -> bool {
        self.scopes.insert(scope)
    }

    /// Returns `true` when the layer is dirty.
    #[must_use]
    pub fn contains(&self, scope: InvalidationScope) -> bool {
        self.scopes.contains(&scope)
    }

    /// Returns `true` when no layer is dirty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.scopes.is_empty()
    }

    /// Number of dirty layers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.scopes.len()
    }

    /// Iterate the dirty layers in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = InvalidationScope> + '_ {
        self.scopes.iter().copied()
    }

    /// Combine two sets without mutating either input.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        let mut merged = self.clone();
        merged.union_with(other);
        merged
    }

    /// Add every layer of `other` to this set.
    pub fn union_with(&mut self, other: &Self) {
        self.scopes.extend(other.scopes.iter().copied());
    }

    /// Recompute layers implied by explicitly dirty layers.
    ///
    /// The dependency chain is measured rather than assumed:
    ///
    /// - `Data` implies `Measurement` because new values can change text that
    ///   must be measured.
    /// - `Measurement` implies `Layout` because placement consumes measured
    ///   text extents.
    /// - `Layout` implies nothing. Moving already measured text does not
    ///   require measuring it again.
    /// - `Viewport`, `Style`, `Motion`, and `Overlay` imply nothing. A host
    ///   that changes font metrics through a style change must dirty
    ///   `Measurement` explicitly.
    #[must_use]
    pub fn closure(&self) -> Self {
        let mut closed = self.clone();
        if closed.contains(InvalidationScope::Data) {
            closed.insert(InvalidationScope::Measurement);
        }
        if closed.contains(InvalidationScope::Measurement) {
            closed.insert(InvalidationScope::Layout);
        }
        closed
    }
}

impl FromIterator<InvalidationScope> for InvalidationSet {
    fn from_iter<T: IntoIterator<Item = InvalidationScope>>(iter: T) -> Self {
        Self { scopes: iter.into_iter().collect() }
    }
}

/// The invalidation result of one action.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Invalidation {
    /// Layers that must be rebuilt.
    pub scopes: InvalidationSet,
    /// Semantic ids affected inside those layers.
    ///
    /// An empty set means the layer as a whole is dirty but the action alone
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

    /// Returns `true` when the layer is dirty.
    #[must_use]
    pub fn contains(&self, scope: InvalidationScope) -> bool {
        self.scopes.contains(scope)
    }
}

/// Report the compile layers an action invalidates.
///
/// The returned set is already closed under the implication rules documented on
/// [`InvalidationSet::closure`]. Direct and returned layers per action:
///
/// | Action | Direct | Returned |
/// |--------|--------|----------|
/// | `Hover`, `Focus`, `Press`, `Release`, `Select`, `ClearSelection` | overlay | overlay |
/// | `Zoom`, `Pan` | viewport | viewport |
/// | `Resize` | measurement, layout | measurement, layout |
/// | `SetParameter` | data, motion | data, measurement, layout, motion |
/// | `SetTheme` | style | style |
/// | `Play`, `Pause`, `Seek`, `Advance` | motion | motion |
///
/// Pointer and selection changes never reach data, layout, measurement,
/// viewport, or motion: repainting the affected overlay is enough. None of
/// these actions changes geometry, so they never append `Measurement`.
/// `Zoom` and `Pan` change only the projection, and scene coordinates are
/// independent of it, so viewport changes never relayout. Playback actions
/// retarget motion only, so advancing time never rereads data or relayouts.
#[must_use]
pub fn invalidate(action: &Action) -> Invalidation {
    let mut invalidation = Invalidation::default();
    match action {
        Action::Hover(target) => {
            invalidation.scopes.insert(InvalidationScope::Overlay);
            extend_target(&mut invalidation, target);
        }
        Action::Focus(target) => {
            invalidation.scopes.insert(InvalidationScope::Overlay);
            extend_target(&mut invalidation, target);
        }
        Action::Press(target) => {
            invalidation.scopes.insert(InvalidationScope::Overlay);
            extend_target(&mut invalidation, target);
        }
        Action::Release => {
            invalidation.scopes.insert(InvalidationScope::Overlay);
        }
        Action::Select { target, .. } => {
            invalidation.scopes.insert(InvalidationScope::Overlay);
            invalidation.targets.insert(target.clone());
        }
        Action::ClearSelection => {
            invalidation.scopes.insert(InvalidationScope::Overlay);
        }
        Action::Zoom { .. } | Action::Pan { .. } => {
            invalidation.scopes.insert(InvalidationScope::Viewport);
        }
        Action::Resize { .. } => {
            invalidation.scopes.insert(InvalidationScope::Measurement);
            invalidation.scopes.insert(InvalidationScope::Layout);
        }
        Action::SetParameter { .. } => {
            invalidation.scopes.insert(InvalidationScope::Data);
            invalidation.scopes.insert(InvalidationScope::Motion);
        }
        Action::SetTheme(_) => {
            invalidation.scopes.insert(InvalidationScope::Style);
        }
        Action::Play | Action::Pause | Action::Seek(_) | Action::Advance(_) => {
            invalidation.scopes.insert(InvalidationScope::Motion);
        }
    }
    invalidation.scopes = invalidation.scopes.closure();
    invalidation
}

/// Record an optional semantic target.
fn extend_target(invalidation: &mut Invalidation, target: &Option<String>) {
    if let Some(target) = target {
        invalidation.targets.insert(target.clone());
    }
}
