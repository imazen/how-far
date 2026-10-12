//! Best-effort observation of sequential work; errors belong to the work.
use crate::{AsStopReason, Child, Complete, Execution, Outcome, PhaseSpec, PlanError, Pulse};
use alloc::{collections::VecDeque, sync::Arc};

/// Sequential work returning the library's ordinary `Result<T, E>`.
///
/// Implement [`AsStopReason`] for your error to recognize cancellation.
/// `From<StopReason>` supports `?` at checks; no planning-error conversion is
/// required. For explicitly fallible planning use [`crate::TryStages`].
///
/// Pass the final result to `complete` (or `ResultExt::finish_phase`) to resolve
/// untouched stages. Use [`Stages::begin`] for direct function calls and hand
/// each result to [`crate::ResultExt::finish_phase`] before applying `?`.
/// Drop alone records abandonment. Recovery belongs within the operation that
/// owns a stage, using separate child owners for individual attempts.
#[must_use = "run stages and hand over the final result; Drop records abandonment"]
pub struct Stages<'a> {
    parent: &'a dyn Pulse,
    pending: VecDeque<(Child<'a>, Option<StageKey>)>,
    state: State,
}
enum State {
    Ready,
    Running,
    Stopped,
}

impl<'a> Stages<'a> {
    /// Declare a sequence. Rejected observations retain cancellation through
    /// fallback children and cannot fail the work.
    #[track_caller]
    pub fn new(parent: &'a dyn Pulse, parts: &[PhaseSpec<'_>]) -> Self {
        let pending = parent
            .plan(Execution::Sequence, parts)
            .into_iter()
            .map(|child| (child, None))
            .collect();
        Self {
            parent,
            pending,
            state: State::Ready,
        }
    }

    /// Declare a sequence and bind its reservations to variables.
    ///
    /// Destructure the returned array, then pass each key to [`Stages::begin`]
    /// in declaration order. Keys belong to this particular plan, even when
    /// another plan has identical specifications. Moving the plan is allowed.
    /// Dropping a key does not abandon its pending stage: the plan retains
    /// completion authority until the stage begins or the plan completes.
    ///
    /// Labels are for observers only; beginning a stage never compares strings.
    /// One shared identity allocation belongs to each nonempty definition.
    #[track_caller]
    pub fn define<const N: usize>(
        parent: &'a dyn Pulse,
        parts: [PhaseSpec<'_>; N],
    ) -> (Self, [StageKey; N]) {
        let mut stages = Self::new(parent, &parts);
        // Arc keeps identity stable across moves and prevents address reuse
        // while a stale key survives. No global counter or mutable state.
        let identity = (N != 0).then(|| Arc::new(()));
        let keys = core::array::from_fn(|index| {
            let key = StageKey {
                identity: Arc::clone(identity.as_ref().expect("nonempty definition")),
                index,
            };
            stages.pending[index].1 = Some(key.clone());
            key
        });
        (stages, keys)
    }

    /// Begin the next stage using its reservation from [`Stages::define`].
    ///
    /// Hand the operation's result to [`crate::ResultExt::finish_phase`]
    /// **before** applying `?`. Failed or cancelled handoffs immediately mark
    /// remaining stages [`Outcome::NotRun`]. Complete the plan with its final
    /// result to resolve untouched optional work.
    ///
    /// A key from another plan, out of order, or already used records
    /// [`PlanError::StageMismatch`] and returns a cancellation-only stage
    /// without consuming the next declaration. After failure or exhaustion,
    /// work also retains cancellation and the protocol issue is recorded.
    ///
    /// Only one active stage can borrow the plan. Join workers before handing
    /// over that stage's result. Dropping it unfinished records abandonment
    /// and prevents the remaining sequence from earning progress.
    #[track_caller]
    pub fn begin(&mut self, key: StageKey) -> Stage<'_, 'a> {
        self.begin_stage(Some(&key))
    }

    /// Begin the next stage in a runtime-sized sequence.
    ///
    /// Use with [`Stages::new`] when a loop repeats the same operation over a
    /// slice. The handle and result handoff are the same as [`Stages::begin`],
    /// but no key is checked. Exhaustion records [`PlanError::NoMoreStages`]
    /// and returns a cancellation-only stage.
    #[track_caller]
    pub fn begin_next(&mut self) -> Stage<'_, 'a> {
        self.begin_stage(None)
    }

    #[track_caller]
    fn begin_stage(&mut self, key: Option<&StageKey>) -> Stage<'_, 'a> {
        let child = self.begin_child(key);
        Stage {
            stages: Some(self),
            child,
        }
    }

    /// Run the next stage, preserving the original result and error identity.
    /// After a stage error, later declared stages become NotRun. Calling again
    /// is diagnosed and runs against a cancellation-only view of the parent.
    #[track_caller]
    pub fn run<T, E>(&mut self, work: impl FnOnce(&dyn Pulse) -> Result<T, E>) -> Result<T, E>
    where
        E: AsStopReason,
    {
        let child = self.begin_child(None);
        let result = work(&child);
        self.complete_result(child, result)
    }

    /// Run with an explicit classifier for foreign error wrappers or a local
    /// classification policy, returning the work's original Result unchanged.
    #[track_caller]
    pub fn run_classified<T, E>(
        &mut self,
        is_stop: impl FnOnce(&E) -> bool,
        work: impl FnOnce(&dyn Pulse) -> Result<T, E>,
    ) -> Result<T, E> {
        let child = self.begin_child(None);
        let result = work(&child);
        self.end(child, Outcome::from_result(&result, is_stop));
        result
    }

    // Keep bookkeeping out of the per-closure monomorphization.
    fn complete_result<T, E>(&mut self, child: Child<'_>, result: Result<T, E>) -> Result<T, E>
    where
        E: AsStopReason,
    {
        let outcome = Outcome::from_result(&result, |error| error.as_stop_reason().is_some());
        self.end(child, outcome);
        result
    }

    #[track_caller]
    fn begin_child(&mut self, key: Option<&StageKey>) -> Child<'a> {
        let child = self.next(key);
        if !matches!(self.state, State::Stopped) {
            self.state = State::Running;
        }
        if let Err(error) = child.start() {
            self.parent.record_issue(error);
        }
        child
    }

    fn end(&mut self, child: Child<'_>, outcome: Outcome) {
        child.complete_as(outcome);
        if matches!(
            outcome,
            Outcome::Failed | Outcome::Cancelled | Outcome::Abandoned | Outcome::NotRun
        ) {
            self.state = State::Stopped;
            self.resolve_remaining(Outcome::NotRun);
        } else if matches!(self.state, State::Running) {
            self.state = State::Ready;
        }
    }

    /// Explicitly mark the next phase unnecessary, making that decision visible now.
    #[track_caller]
    pub fn skip(&mut self) {
        self.next(None).complete_as(Outcome::Skipped);
    }

    #[track_caller]
    fn next(&mut self, key: Option<&StageKey>) -> Child<'a> {
        if matches!(self.state, State::Running) {
            // A prior closure panicked, and the caller caught it.
            self.state = State::Stopped;
            self.resolve_remaining(Outcome::NotRun);
        }
        if matches!(self.state, State::Stopped) {
            self.parent.record_issue(PlanError::Finished);
        } else if let Some((_, expected)) = self.pending.front() {
            if key.is_some_and(|key| {
                !expected
                    .as_ref()
                    .is_some_and(|expected| expected.matches(key))
            }) {
                self.parent.record_issue(PlanError::StageMismatch);
            } else {
                return self.pending.pop_front().expect("checked pending stage").0;
            }
        } else {
            self.parent.record_issue(PlanError::NoMoreStages);
        }
        Child::untracked(self.parent)
    }

    fn resolve_remaining(&mut self, outcome: Outcome) {
        while let Some((child, _)) = self.pending.pop_front() {
            child.complete_inferred(outcome);
        }
    }
}
impl Complete for Stages<'_> {
    fn complete_as(mut self, outcome: Outcome) {
        self.resolve_remaining(match outcome {
            Outcome::Succeeded | Outcome::Skipped => Outcome::Skipped,
            Outcome::Abandoned => Outcome::Abandoned,
            _ => Outcome::NotRun,
        });
    }
}

/// A reservation identifying one stage in one [`Stages::define`] plan.
///
/// Pass it to [`Stages::begin`]. It carries no progress or completion rights;
/// dropping it leaves the stage pending. A clone identifies the same stage,
/// not another execution: attempting to begin it twice is diagnosed.
#[derive(Clone, Debug)]
pub struct StageKey {
    identity: Arc<()>,
    index: usize,
}
impl StageKey {
    fn matches(&self, other: &Self) -> bool {
        self.index == other.index && Arc::ptr_eq(&self.identity, &other.identity)
    }
}

/// One active sequential stage, borrowing its plan until result handoff.
///
/// Pass `&stage` as the operation's [`Pulse`], then use
/// [`crate::ResultExt::finish_phase`] or [`Complete::complete`] before `?`.
/// Completion consumes the handle and updates its plan. Dropping it without
/// completion records [`Outcome::Abandoned`] and resolves pending work as
/// [`Outcome::NotRun`]. Shared views have no completion rights.
///
/// A second stage cannot begin while the first is live:
///
/// ```compile_fail,E0499
/// use how_far::{prelude::*, NoPulse, PhaseSpec, Stages, Total};
/// let (mut stages, [a, b]) = Stages::define(&NoPulse, [
///     PhaseSpec::new("a", 1, Total::Unknown),
///     PhaseSpec::new("b", 1, Total::Unknown),
/// ]);
/// let first = stages.begin(a);
/// let second = stages.begin(b);
/// first.complete(Ok::<_, how_far::StopReason>(())).unwrap();
/// ```
#[must_use = "hand over the stage result before ?; dropping it records Abandoned"]
pub struct Stage<'s, 'p> {
    stages: Option<&'s mut Stages<'p>>,
    child: Child<'p>,
}
impl Stage<'_, '_> {
    /// Start independently owned branches within this stage.
    ///
    /// This declares [`Execution::ForkJoin`] and starts each branch; it does
    /// not schedule work. Move the handles into scoped threads, Rayon work,
    /// or borrowed async futures. Each branch can use different data and
    /// return a different type, or declare a nested [`Stages`] sequence.
    ///
    /// Hand each result to its branch, join **all** workers, then hand the
    /// combined result to this stage. A failed branch does not automatically
    /// cancel its siblings. Every returned branch is active: dropping one
    /// unfinished records [`Outcome::Abandoned`], not optional skipped work.
    /// Rejected plans retain cancellation in every branch and record an issue.
    ///
    /// The parent cannot be completed while a branch still borrows it:
    ///
    /// ```compile_fail,E0505
    /// use how_far::{prelude::*, NoPulse, PhaseSpec, Stages, Total, Outcome};
    /// let (mut stages, [work]) = Stages::define(&NoPulse, [
    ///     PhaseSpec::new("work", 1, Total::Unknown),
    /// ]);
    /// let work = stages.begin(work);
    /// let [branch] = work.fork([PhaseSpec::new("branch", 1, Total::Unknown)]);
    /// work.complete_as(Outcome::Succeeded);
    /// branch.complete_as(Outcome::Succeeded);
    /// ```
    #[track_caller]
    pub fn fork<const N: usize>(&self, parts: [PhaseSpec<'_>; N]) -> [Child<'_>; N] {
        let mut children = self.fork_children(&parts).into_iter();
        // Best-effort planning guarantees exactly one handle per declaration,
        // including when a sink violates its split contract.
        core::array::from_fn(|_| children.next().expect("one child per declaration"))
    }

    #[track_caller]
    fn fork_children(&self, parts: &[PhaseSpec<'_>]) -> alloc::vec::Vec<Child<'_>> {
        let children = self.child.plan(Execution::ForkJoin, parts);
        for child in &children {
            if let Err(error) = child.start() {
                self.child.record_issue(error);
            }
        }
        children
    }

    fn close(&mut self, outcome: Outcome) {
        if let Some(stages) = self.stages.take() {
            let child = core::mem::replace(&mut self.child, Child::inert());
            stages.end(child, outcome);
        }
    }
}
impl core::fmt::Debug for Stage<'_, '_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Stage")
            .field("child", &self.child)
            .finish_non_exhaustive()
    }
}
impl Complete for Stage<'_, '_> {
    fn complete_as(mut self, outcome: Outcome) {
        self.close(outcome);
    }
}
impl Drop for Stage<'_, '_> {
    fn drop(&mut self) {
        self.close(Outcome::Abandoned);
    }
}
impl crate::Stop for Stage<'_, '_> {
    #[inline]
    #[track_caller]
    fn check(&self) -> Result<(), crate::StopReason> {
        self.child.check()
    }
    #[inline]
    fn may_stop(&self) -> bool {
        self.child.may_stop()
    }
}
impl crate::Report for Stage<'_, '_> {
    #[inline]
    #[track_caller]
    fn advance(&self, completed: u64) {
        self.child.advance(completed);
    }
    #[inline]
    fn may_report(&self) -> bool {
        self.child.may_report()
    }
}
impl Pulse for Stage<'_, '_> {
    #[track_caller]
    fn record_issue(&self, error: PlanError) {
        self.child.record_issue(error);
    }
    fn split(
        &self,
        execution: Execution,
        parts: &[PhaseSpec<'_>],
    ) -> Result<alloc::vec::Vec<Child<'_>>, PlanError> {
        self.child.split(execution, parts)
    }
    fn start(&self) -> Result<(), PlanError> {
        self.child.start()
    }
    fn set_total(&self, total: crate::Total) -> Result<(), PlanError> {
        self.child.set_total(total)
    }
    fn share(&self) -> Result<crate::SharedPulse, PlanError> {
        self.child.share()
    }
}
