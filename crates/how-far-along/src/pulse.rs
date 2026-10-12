//! One lifecycle owner and shareable, non-owning pulse views.
use crate::{
    Child, ChildPulse, Execution, NodeId, Observer, Outcome, Phase, PhaseSpec, PlanError, Pulse,
    Report, Reporter, SharedPulse, Stop, StopReason, Total, sync::OwnerCell,
};
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::{
    fmt,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

/// Owns the root of a tracked operation. Workers borrow it or use `share()`.
/// Finish after joining workers. Dropping this owner freezes abandonment even
/// when non-owning shared views remain alive.
#[must_use = "finish the root after its work joins; dropping it unfinished records Abandoned"]
pub struct PulseTree {
    node: TreePulse<Arc<dyn Stop>>,
}
impl PulseTree {
    /// Track a phase with a shared cancellation policy.
    pub fn new(phase: Phase, stop: impl Stop + 'static) -> Self {
        Self {
            node: TreePulse::new(phase, Arc::new(stop) as Arc<dyn Stop>),
        }
    }
    /// Observe the root, including after its owner finishes.
    pub fn observer(&self) -> Observer {
        self.node.observer()
    }
    /// The root's stable identity.
    pub fn id(&self) -> NodeId {
        self.node.state.observer.id()
    }
    /// Finish the root after workers and children join. On error it is abandoned.
    pub fn finish(self, outcome: Outcome) -> Result<(), PlanError> {
        self.node.finish(outcome)
    }
}
impl fmt::Debug for PulseTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PulseTree")
            .field("root", &self.id())
            .finish()
    }
}
const UNPLANNED: u8 = 0;
const COUNTING: u8 = 1;
const SPLIT: u8 = 2;
const FINISHED: u8 = 3;

/// Checkpoint behavior is selected by the owner type, not by a Cargo feature
/// or a per-node Option. Cloning a policy carries it into children and views.
pub(crate) trait Policy: Clone + Send + Sync + 'static {
    #[track_caller]
    fn check(&self, phase: &Observer) -> Result<(), StopReason>;
    fn may_stop(&self) -> bool;
}
impl Policy for Arc<dyn Stop> {
    #[track_caller]
    fn check(&self, _phase: &Observer) -> Result<(), StopReason> {
        Stop::check(self)
    }
    fn may_stop(&self) -> bool {
        Stop::may_stop(self)
    }
}

// This state is shared. The TreePulse token, not the last Arc, owns completion.
struct State<S: Policy> {
    owner: OwnerCell<Phase>,
    reporter: Reporter,
    observer: Observer,
    stop: S,
    activity: AtomicU8,
    closed: AtomicBool,
}
pub(crate) struct TreePulse<S: Policy> {
    state: Arc<State<S>>,
}
#[derive(Clone)]
struct View<S: Policy> {
    state: Arc<State<S>>,
}
impl<S: Policy> TreePulse<S> {
    pub(crate) fn observer(&self) -> Observer {
        self.state.observer.clone()
    }
    pub(crate) fn finish(self, outcome: Outcome) -> Result<(), PlanError> {
        self.state.finish(outcome)
    }
    pub(crate) fn new(phase: Phase, stop: S) -> Self {
        Self {
            state: Arc::new(State {
                reporter: phase.deferred_reporter(),
                observer: phase.observer(),
                owner: OwnerCell::new(phase),
                stop,
                activity: AtomicU8::new(UNPLANNED),
                closed: AtomicBool::new(false),
            }),
        }
    }
}
impl<S: Policy> State<S> {
    fn complete(&self, outcome: Outcome) -> Result<(), PlanError> {
        self.with_owner(|phase| phase.record_outcome(outcome))??;
        self.activity.store(FINISHED, Ordering::Release);
        Ok(())
    }
    fn start(&self) -> Result<(), PlanError> {
        self.with_owner(Phase::start)?
    }
    fn set_total(&self, total: Total) -> Result<(), PlanError> {
        self.with_owner(|p| p.set_total(total))?
    }
    fn share(self: &Arc<Self>) -> Result<SharedPulse, PlanError> {
        Ok(SharedPulse::new(View {
            state: Arc::clone(self),
        }))
    }

    /// Administer the phase. Concurrent administrators, including shared
    /// views, run one at a time; a closed owner reports `Finished`.
    fn with_owner<R>(&self, f: impl FnOnce(&mut Phase) -> R) -> Result<R, PlanError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(PlanError::Finished);
        }
        self.owner.with(f).ok_or(PlanError::Finished)
    }
    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.activity.store(FINISHED, Ordering::Release);
        // Waits only for an administrative call already under way; an
        // unfinished phase records Abandoned as it drops, outside the lock.
        drop(self.owner.take());
    }
    fn finish(&self, outcome: Outcome) -> Result<(), PlanError> {
        let result = self.with_owner(|phase| phase.finish_with(outcome))?;
        if result.is_ok() {
            self.activity.store(FINISHED, Ordering::Release);
        }
        result
    }
    fn split(
        &self,
        execution: Execution,
        parts: &[PhaseSpec<'_>],
    ) -> Result<Vec<Child<'static>>, PlanError> {
        PhaseSpec::validate_split(parts)?;
        self.activity
            .compare_exchange(UNPLANNED, SPLIT, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|s| {
                if s == FINISHED {
                    PlanError::Finished
                } else {
                    PlanError::AlreadyInUse
                }
            })?;
        let mut children = Vec::with_capacity(parts.len());
        let split = self.with_owner(|phase| {
            phase.split_each(execution, parts, &mut |phase| {
                children.push(Child::new(TreePulse::new(phase, self.stop.clone())));
            })
        });
        match split {
            Ok(Ok(())) => Ok(children),
            Ok(Err(error)) | Err(error) => {
                // A concurrent owner drop must never resurrect planning.
                let _ = self.activity.compare_exchange(
                    SPLIT,
                    UNPLANNED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                Err(error)
            }
        }
    }
}
impl<S: Policy> Stop for State<S> {
    #[track_caller]
    fn check(&self) -> Result<(), StopReason> {
        self.stop.check(&self.observer)
    }
    fn may_stop(&self) -> bool {
        self.stop.may_stop()
    }
}
impl<S: Policy> Report for State<S> {
    #[track_caller]
    fn advance(&self, n: u64) {
        if n == 0 {
            return;
        }
        // During a split the reporter decides: it ignores a phase that became
        // a branch, and counts work for one whose split failed.
        if self.activity.load(Ordering::Acquire) == COUNTING
            || matches!(
                self.activity.compare_exchange(
                    UNPLANNED,
                    COUNTING,
                    Ordering::AcqRel,
                    Ordering::Acquire
                ),
                Ok(_) | Err(COUNTING) | Err(SPLIT)
            )
        {
            self.reporter.advance(n);
        }
    }
    fn may_report(&self) -> bool {
        self.activity.load(Ordering::Relaxed) != FINISHED && self.reporter.may_report()
    }
}
impl<S: Policy> Drop for TreePulse<S> {
    fn drop(&mut self) {
        self.state.close();
    }
}
impl<S: Policy> ChildPulse for TreePulse<S> {
    fn complete_inferred(self: Box<Self>, outcome: Outcome) {
        let _ = self
            .state
            .with_owner(|phase| phase.complete_inferred(outcome));
    }
    fn complete_as(self: Box<Self>, outcome: Outcome) {
        let _ = self.state.complete(outcome);
    }
    fn finish(self: Box<Self>, outcome: Outcome) -> Result<(), PlanError> {
        self.state.finish(outcome)
    }
}

impl<S: Policy> how_far::Complete for TreePulse<S> {
    fn complete_as(self, outcome: Outcome) {
        let _ = self.state.complete(outcome);
    }
}

impl how_far::Complete for PulseTree {
    fn complete_as(self, outcome: Outcome) {
        let _ = self.node.state.complete(outcome);
    }
}
/// Forward the pulse traits to `self.$field`, a [`State`] or a pulse.
macro_rules! pulse_view {
    ([$($generic:ident : $bound:path)?] $ty:ty, $field:ident $(, #[$hint:meta])?) => {
        impl$(<$generic: $bound>)? Stop for $ty {
            $(#[$hint])?
            #[track_caller]
            fn check(&self) -> Result<(), StopReason> {
                self.$field.check()
            }
            fn may_stop(&self) -> bool {
                self.$field.may_stop()
            }
        }
        impl$(<$generic: $bound>)? Report for $ty {
            $(#[$hint])?
            #[track_caller]
            fn advance(&self, n: u64) {
                self.$field.advance(n)
            }
            fn may_report(&self) -> bool {
                self.$field.may_report()
            }
        }
        impl$(<$generic: $bound>)? Pulse for $ty {
            fn split(
                &self,
                e: Execution,
                parts: &[PhaseSpec<'_>],
            ) -> Result<Vec<Child<'_>>, PlanError> {
                self.$field.split(e, parts)
            }
            fn start(&self) -> Result<(), PlanError> {
                self.$field.start()
            }
            fn set_total(&self, total: Total) -> Result<(), PlanError> {
                self.$field.set_total(total)
            }
            fn share(&self) -> Result<SharedPulse, PlanError> {
                self.$field.share()
            }
        }
    };
}
pulse_view!([S: Policy] TreePulse<S>, state, #[inline]);
pulse_view!([S: Policy] View<S>, state, #[inline]);
pulse_view!([] PulseTree, node);
