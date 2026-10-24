//! Motion intent, stable correspondence, and transition planning.

use std::collections::{BTreeSet, HashSet};

use davinci_scene::{Scene, Transform};

use crate::{
    easing::Easing,
    internal::{first_nodes, node_opacity, ordered_ids},
};

/// A channel that motion can animate between two matching nodes.
///
/// Channels also define their own iteration order, so a plan always lists all
/// transform tracks before all opacity tracks for the same node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Channel {
    /// The node's local affine transform.
    Transform,
    /// The node's own opacity, as stored by its variant or style.
    Opacity,
}

/// How a node that only exists in the next scene appears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnterPolicy {
    /// Appear at full target state from the first sampled frame.
    Snap,
    /// Fade the node's own opacity from `0.0` to its target over the duration.
    Fade,
}

/// How a node that only exists in the previous scene disappears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitPolicy {
    /// Disappear at the first sampled frame.
    Snap,
    /// Keep drawing the previous subtree and fade it to `0.0` at the duration.
    Fade,
    /// Keep drawing the previous subtree unchanged for every sampled time.
    Keep,
}

/// What an in-flight timeline does when a new transition interrupts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterruptionPolicy {
    /// Drop the old progress and restart the supplied plan from time zero.
    CancelAndRestart,
    /// Treat the current sampled frame as the new start and continue to the new
    /// target.
    ContinueFromCurrent,
    /// Skip intermediate frames and jump directly to the new final scene.
    JumpToEnd,
}

/// Timing, easing, animated channels, and enter/exit/interruption policy.
///
/// Durations are in seconds. A non-positive duration means "no animation": the
/// sample function returns the next scene directly.
#[derive(Clone, Debug, PartialEq)]
pub struct MotionIntent {
    /// Length of the transition in seconds.
    pub duration: f64,
    /// Easing applied to normalized progress.
    pub ease: Easing,
    /// Channels animated for nodes that exist in both scenes.
    pub channels: BTreeSet<Channel>,
    /// Treatment of nodes that only exist in the next scene.
    pub enter: EnterPolicy,
    /// Treatment of nodes that only exist in the previous scene.
    pub exit: ExitPolicy,
    /// Treatment of a timeline that is interrupted by a new plan.
    pub interruption: InterruptionPolicy,
}

impl Default for MotionIntent {
    /// A practical default: 0.3 s, `EaseInOut`, transform and opacity, fade in
    /// and out, and restart on interruption.
    fn default() -> Self {
        Self {
            duration: 0.3,
            ease: Easing::EaseInOut,
            channels: BTreeSet::from([Channel::Transform, Channel::Opacity]),
            enter: EnterPolicy::Fade,
            exit: ExitPolicy::Fade,
            interruption: InterruptionPolicy::CancelAndRestart,
        }
    }
}

/// The role a stable id plays between two scenes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CorrespondenceKind {
    /// The id exists in both scenes.
    Update,
    /// The id exists only in the next scene.
    Enter,
    /// The id exists only in the previous scene.
    Exit,
}

/// A stable id and the role it plays in a transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Correspondence {
    /// The node id, matched across the whole tree.
    pub id: String,
    /// Whether the node updates, enters, or exits.
    pub kind: CorrespondenceKind,
}

/// One channel value sampled from a scene node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChannelValue {
    /// A local affine transform.
    Transform(Transform),
    /// A node opacity in the inclusive range `0.0..=1.0`.
    Opacity(f64),
}

/// An animated channel for one node id, from a start value to an end value.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    /// Node id this track applies to.
    pub id: String,
    /// Animated channel.
    pub channel: Channel,
    /// Value at the start of the transition.
    pub from: ChannelValue,
    /// Value at the end of the transition.
    pub to: ChannelValue,
}

/// A transition from a previous scene to a next scene.
///
/// The plan stores correspondence, the animated tracks that sampling reads, and
/// the intent that supplied timing and policies.
#[derive(Clone, Debug, PartialEq)]
pub struct TransitionPlan {
    /// Stable ids in `Update`, then `Enter`, then `Exit` order.
    pub correspondences: Vec<Correspondence>,
    /// Tracks for `Update` nodes only, ordered by correspondence and then by
    /// channel. Unchanged channels omit their track.
    pub tracks: Vec<Track>,
    /// Intent the plan was built from.
    pub intent: MotionIntent,
}

/// Match nodes between two scenes by stable id.
///
/// The whole tree is walked, including group children. Only
/// [`SceneNode::id`](davinci_scene::SceneNode::id) participates: nodes without
/// an id are never matched, and duplicates keep the first occurrence in
/// pre-order document order.
///
/// The result is deterministic: every `Update` (in next-scene order) precedes
/// every `Enter` (next-only, in next-scene order), which precedes every `Exit`
/// (previous-only, in previous-scene order).
pub fn match_scenes(previous: &Scene, next: &Scene) -> Vec<Correspondence> {
    let previous_ids = ordered_ids(previous);
    let next_ids = ordered_ids(next);
    let previous_set: HashSet<&str> = previous_ids.iter().copied().collect();
    let next_set: HashSet<&str> = next_ids.iter().copied().collect();

    let mut correspondences = Vec::with_capacity(previous_ids.len() + next_ids.len());
    for id in &next_ids {
        if previous_set.contains(id) {
            correspondences.push(Correspondence { id: (*id).to_owned(), kind: CorrespondenceKind::Update });
        }
    }
    for id in &next_ids {
        if !previous_set.contains(id) {
            correspondences.push(Correspondence { id: (*id).to_owned(), kind: CorrespondenceKind::Enter });
        }
    }
    for id in &previous_ids {
        if !next_set.contains(id) {
            correspondences.push(Correspondence { id: (*id).to_owned(), kind: CorrespondenceKind::Exit });
        }
    }
    correspondences
}

/// Build a transition plan for two scenes and an intent.
///
/// Tracks are generated only for `Update` nodes and only for channels listed in
/// `intent.channels` whose value actually changes; a track's `from` and `to`
/// come from the first occurrence of the id in the previous and next scenes.
///
/// `Enter` and `Exit` nodes never get tracks. Their appearance and
/// disappearance are entirely decided by [`sample`](crate::sample) from
/// `intent.enter` and `intent.exit`, which keeps the policy in one place.
pub fn plan_transition(previous: &Scene, next: &Scene, intent: &MotionIntent) -> TransitionPlan {
    let correspondences = match_scenes(previous, next);
    let previous_nodes = first_nodes(previous);
    let next_nodes = first_nodes(next);
    let mut tracks = Vec::new();

    for correspondence in correspondences.iter().filter(|item| item.kind == CorrespondenceKind::Update) {
        let Some(previous_node) = previous_nodes.get(correspondence.id.as_str())
        else {
            continue;
        };
        let Some(next_node) = next_nodes.get(correspondence.id.as_str())
        else {
            continue;
        };

        for channel in &intent.channels {
            match channel {
                Channel::Transform => {
                    if previous_node.transform != next_node.transform {
                        tracks.push(Track {
                            id: correspondence.id.clone(),
                            channel: Channel::Transform,
                            from: ChannelValue::Transform(previous_node.transform),
                            to: ChannelValue::Transform(next_node.transform),
                        });
                    }
                }
                Channel::Opacity => {
                    let from = node_opacity(previous_node);
                    let to = node_opacity(next_node);
                    if from != to {
                        tracks.push(Track {
                            id: correspondence.id.clone(),
                            channel: Channel::Opacity,
                            from: ChannelValue::Opacity(from),
                            to: ChannelValue::Opacity(to),
                        });
                    }
                }
            }
        }
    }

    TransitionPlan { correspondences, tracks, intent: intent.clone() }
}
