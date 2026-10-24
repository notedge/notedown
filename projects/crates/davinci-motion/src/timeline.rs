//! Host-driven timeline state over a transition plan.

use davinci_scene::DisplayScene;

use crate::{
    plan::{TransitionPlan, plan_transition},
    sample::sample,
};

/// Host-advanced transition state.
///
/// The timeline holds the current plan and both scenes plus the elapsed time.
/// It never reads a clock: the host adds deltas or asks for an arbitrary sample
/// time, which makes static export and interactive playback share one path.
#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    /// Plan currently being sampled.
    pub plan: TransitionPlan,
    /// Scene supplying exiting subtrees.
    pub previous: DisplayScene,
    /// Scene supplying the output tree and update targets.
    pub next: DisplayScene,
    /// Elapsed seconds since the current plan started, clamped to the duration.
    pub elapsed: f64,
}

impl Timeline {
    /// Start a timeline at time zero.
    pub fn new(previous: DisplayScene, plan: TransitionPlan, next: DisplayScene) -> Self {
        Self { plan, previous, next, elapsed: 0.0 }
    }

    /// Advance the timeline by `delta` seconds and return the sampled scene.
    ///
    /// Negative deltas move backwards. The elapsed time is clamped to
    /// `0.0..=duration`, so overshooting lands exactly on the final scene.
    pub fn advance(&mut self, delta: f64) -> DisplayScene {
        let duration = self.duration();
        self.elapsed = (self.elapsed + delta).clamp(0.0, duration);
        self.scene_at(self.elapsed)
    }

    /// Sample the current plan at `time` without changing elapsed time.
    pub fn scene_at(&self, time: f64) -> DisplayScene {
        sample(&self.plan, &self.previous, &self.next, time)
    }

    /// Replace the in-flight transition according to the new plan's
    /// [`InterruptionPolicy`](crate::InterruptionPolicy) and return the scene
    /// to draw next.
    ///
    /// `next` is the target scene of the new transition and `current` is the
    /// frame that was last drawn.
    ///
    /// - `CancelAndRestart` swaps in the supplied plan as-is and resets elapsed
    ///   time to zero, so update channels restart from the plan's recorded
    ///   start values. The supplied plan does not carry its source scene, so
    ///   `previous` becomes `current`; exiting subtrees are therefore taken
    ///   from the frame that was visible.
    /// - `ContinueFromCurrent` rebuilds the plan from `current` to `next` using
    ///   the supplied intent, sets `previous` to `current`, and resets elapsed
    ///   time to zero. The returned scene is exactly the current frame, so the
    ///   new transition starts where the old one was interrupted. Any tracks in
    ///   the supplied plan are replaced by the rebuilt ones.
    /// - `JumpToEnd` installs the supplied plan, moves elapsed time to the
    ///   duration, and returns the final next scene.
    pub fn interrupt(&mut self, plan: TransitionPlan, next: DisplayScene, current: DisplayScene) -> DisplayScene {
        match plan.intent.interruption {
            crate::InterruptionPolicy::CancelAndRestart => {
                self.plan = plan;
                self.previous = current;
                self.next = next;
                self.elapsed = 0.0;
                self.scene_at(0.0)
            }
            crate::InterruptionPolicy::ContinueFromCurrent => {
                let rebuilt = plan_transition(&current, &next, &plan.intent);
                self.plan = rebuilt;
                self.previous = current;
                self.next = next;
                self.elapsed = 0.0;
                self.scene_at(0.0)
            }
            crate::InterruptionPolicy::JumpToEnd => {
                self.plan = plan;
                self.previous = current;
                self.next = next;
                self.elapsed = self.duration();
                self.next.clone()
            }
        }
    }

    /// Returns `true` when no frame can change anymore.
    pub fn is_complete(&self) -> bool {
        self.elapsed >= self.duration()
    }

    fn duration(&self) -> f64 {
        self.plan.intent.duration.max(0.0)
    }
}
