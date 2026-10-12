use how_far_along::{prelude::*, *};
use std::sync::Mutex;

fn tree() -> PulseTree {
    PulseTree::new(Phase::new("job", Total::Unknown), Unstoppable)
}
fn parts() -> [PhaseSpec<'static>; 3] {
    [
        PhaseSpec::new("decode", 2, Total::Exact(2)),
        PhaseSpec::new("resize", 3, Total::Exact(1)),
        PhaseSpec::new("optional", 1, Total::Unknown),
    ]
}
fn decode(input: &[u8], p: &dyn Pulse) -> Result<Vec<u8>, StopReason> {
    p.check()?;
    let output = input.to_vec();
    p.step(input.len() as u64)?;
    Ok(output)
}
fn resize(input: &[u8], p: &dyn Pulse) -> Result<usize, StopReason> {
    p.step(1)?;
    Ok(input.len())
}
fn pipeline(input: &[u8], p: &dyn Pulse) -> Result<usize, StopReason> {
    let (mut stages, [decode_key, resize_key, _optional_key]) = Stages::define(p, parts());
    let decode_phase = stages.begin(decode_key);
    let pixels = decode(input, &decode_phase).finish_phase(decode_phase)?;
    let resize_phase = stages.begin(resize_key);
    let count = resize(&pixels, &resize_phase).finish_phase(resize_phase)?;
    stages.complete(Ok(count))
}
#[test]
fn different_operations_use_ordinary_calls_and_leave_the_borrowed_root_open() {
    let root = tree();
    assert_eq!(pipeline(&[7, 9], &root), Ok(2));
    let snap = root.observer().snapshot();
    assert_eq!(snap.status, Status::Running);
    assert_eq!(snap.children[0].completed, 2);
    assert_eq!(snap.children[1].completed, 1);
    assert_eq!(snap.children[2].status, Status::Finished(Outcome::Skipped));
    assert!(snap.children[2].completion_inferred);
    // Weighted fractions may round just below one; lifecycle states are exact.
    assert!((snap.fraction().unwrap() - 1.0).abs() <= f64::EPSILON);
    root.complete(Ok::<_, StopReason>(())).unwrap();
}
#[test]
fn cancellation_before_question_mark_resolves_the_tail_without_a_closure() {
    let root = FnPulse::new("job", |cx| {
        assert_eq!(cx.phase_name(), "decode");
        Err(StopReason::TimedOut)
    });
    assert_eq!(pipeline(&[7, 9], &root), Err(StopReason::TimedOut));
    let snap = root.observer().snapshot();
    assert_eq!(
        snap.children[0].status,
        Status::Finished(Outcome::Cancelled)
    );
    for child in &snap.children[1..] {
        assert_eq!(child.status, Status::Finished(Outcome::NotRun));
        assert!(child.completion_inferred);
    }
    assert_eq!(snap.fraction(), Some(0.0));
}
#[test]
fn foreign_failure_keeps_its_identity_and_stops_the_sequence_immediately() {
    struct ForeignError(u32);
    let root = tree();
    let (mut stages, [decode_key, _resize_key, _optional_key]) = Stages::define(&root, parts());
    let stage = stages.begin(decode_key);
    let error = Box::new(ForeignError(17));
    let identity = &*error as *const _;
    let result = stage.complete_classified(Err::<(), _>(error), |_| false);
    let error = result.err().unwrap();
    assert_eq!(&*error as *const _, identity);
    assert_eq!(error.0, 17);
    // Inspect before dropping or completing the plan: the stage handoff did it.
    let snap = root.observer().snapshot();
    assert_eq!(snap.children[0].status, Status::Finished(Outcome::Failed));
    assert_eq!(snap.children[1].status, Status::Finished(Outcome::NotRun));
    assert_eq!(snap.children[2].status, Status::Finished(Outcome::NotRun));
}
#[test]
fn dropped_or_unwound_stage_is_abandoned_and_never_earns_success() {
    for panic in [false, true] {
        let root = tree();
        let (mut stages, [decode_key, _resize_key, _optional_key]) = Stages::define(&root, parts());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let stage = stages.begin(decode_key);
            stage.advance(1);
            if panic {
                panic!("operation panic");
            }
            drop(stage);
        }));
        assert_eq!(result.is_err(), panic);
        let snap = root.observer().snapshot();
        assert_eq!(
            snap.children[0].status,
            Status::Finished(Outcome::Abandoned)
        );
        assert!(!snap.children[0].completion_inferred);
        assert_eq!(snap.children[1].status, Status::Finished(Outcome::NotRun));
        // Even an incorrect later success handoff cannot rewrite terminal evidence.
        stages.complete(Ok::<_, StopReason>(())).unwrap();
        assert_eq!(root.observer().snapshot(), snap);
    }
}
#[test]
fn temporary_labels_and_repeated_names_work_with_reservations_and_legacy_helpers() {
    let root = tree();
    let (mut stages, [first, _skip, third, _last]) = {
        let name = String::from("第一阶段");
        Stages::define(
            &root,
            [
                PhaseSpec::new(&name, 1, Total::Exact(1)),
                PhaseSpec::new("same", 1, Total::Unknown),
                PhaseSpec::new("same", 1, Total::Unknown),
                PhaseSpec::new("last", 1, Total::Unknown),
            ],
        )
    };
    let stage = stages.begin(first);
    stage.step(1).finish_phase(stage).unwrap();
    stages.skip();
    let stage = stages.begin(third);
    stage.complete(Ok::<_, StopReason>(())).unwrap();
    stages.run(|p| p.check()).unwrap();
    stages.complete(Ok::<_, StopReason>(())).unwrap();
    let snap = root.observer().snapshot();
    assert_eq!(snap.children[1].status, Status::Finished(Outcome::Skipped));
    for index in [0, 2, 3] {
        assert_eq!(
            snap.children[index].status,
            Status::Finished(Outcome::Succeeded)
        );
    }
}

struct Issues {
    errors: Mutex<Vec<PlanError>>,
}
impl Stop for Issues {
    fn check(&self) -> Result<(), StopReason> {
        Err(StopReason::Cancelled)
    }
}
impl Report for Issues {
    fn advance(&self, _: u64) {}
}
impl Pulse for Issues {
    fn record_issue(&self, error: PlanError) {
        self.errors.lock().unwrap().push(error);
    }
    fn split(
        &self,
        _: Execution,
        _: &[PhaseSpec<'_>],
    ) -> Result<Vec<how_far::Child<'_>>, PlanError> {
        Err(PlanError::Unsupported)
    }
}
#[test]
fn rejected_plans_and_wrong_keys_keep_cancellation_without_consuming_the_next_stage() {
    let pulse = Issues {
        errors: Mutex::new(Vec::new()),
    };
    let (mut stages, [decode_key, resize_key, optional_key]) = Stages::define(&pulse, parts());
    let wrong = stages.begin(resize_key.clone());
    assert_eq!(wrong.check(), Err(StopReason::Cancelled));
    wrong.complete_as(Outcome::Succeeded);
    let right = stages.begin(decode_key.clone());
    assert_eq!(right.check(), Err(StopReason::Cancelled));
    right.complete_as(Outcome::Succeeded);
    let repeated = stages.begin(decode_key);
    repeated.complete_as(Outcome::Succeeded);
    let right = stages.begin(resize_key);
    right.complete_as(Outcome::Failed);
    let after_error = stages.begin(optional_key);
    assert_eq!(after_error.check(), Err(StopReason::Cancelled));
    after_error.complete_as(Outcome::Succeeded);
    assert_eq!(
        *pulse.errors.lock().unwrap(),
        [
            PlanError::Unsupported,
            PlanError::StageMismatch,
            PlanError::StageMismatch,
            PlanError::Finished,
        ]
    );
}
#[test]
fn a_named_stage_forwards_nesting_owned_workers_and_total_revision() {
    let root = tree();
    let (mut stages, [decode_key, _resize_key, _optional_key]) = Stages::define(&root, parts());
    let stage = stages.begin(decode_key);
    stage.set_total(Total::Exact(3)).unwrap();
    let shared = stage.share().unwrap();
    let child_id = root.observer().snapshot().children[0].id;
    std::thread::spawn(move || {
        let (mut nested, [nested_key]) =
            Stages::define(&shared, [PhaseSpec::new("nested", 1, Total::Exact(3))]);
        let stage = nested.begin(nested_key);
        stage.step(3).finish_phase(stage).unwrap();
        nested.complete(Ok::<_, StopReason>(())).unwrap();
    })
    .join()
    .unwrap();
    stage.complete(Ok::<_, StopReason>(())).unwrap();
    stages.complete(Ok::<_, StopReason>(())).unwrap();
    let snap = root.observer().snapshot();
    assert_eq!(snap.children[0].id, child_id);
    assert_eq!(snap.children[0].children[0].completed, 3);
    assert_eq!(
        snap.children[0].status,
        Status::Finished(Outcome::Succeeded)
    );
}
#[test]
fn exhausted_sequence_keeps_the_stop_policy_and_reports_the_issue() {
    let pulse = Issues {
        errors: Mutex::new(Vec::new()),
    };
    let (mut stages, [one]) = Stages::define(&pulse, [PhaseSpec::new("one", 1, Total::Unknown)]);
    stages.begin(one.clone()).complete_as(Outcome::Succeeded);
    let extra = stages.begin(one);
    assert_eq!(extra.check(), Err(StopReason::Cancelled));
    extra.complete_as(Outcome::Succeeded);
    assert_eq!(
        *pulse.errors.lock().unwrap(),
        [PlanError::Unsupported, PlanError::NoMoreStages]
    );
}

#[test]
fn a_bare_question_mark_cannot_invent_a_result_handoff() {
    fn forget_handoff(pulse: &dyn Pulse) -> Result<(), StopReason> {
        let (mut stages, [decode_key, _resize_key, _optional_key]) = Stages::define(pulse, parts());
        let stage = stages.begin(decode_key);
        stage.check()?; // Deliberate misuse: no result was handed to the owner.
        stage.complete(Ok::<_, StopReason>(()))?;
        stages.complete(Ok(()))
    }
    let root = FnPulse::new("job", |_| Err(StopReason::Cancelled));
    assert_eq!(forget_handoff(&root), Err(StopReason::Cancelled));
    let snap = root.observer().snapshot();
    assert_eq!(
        snap.children[0].status,
        Status::Finished(Outcome::Abandoned)
    );
    assert_eq!(snap.children[1].status, Status::Finished(Outcome::NotRun));
    assert_eq!(snap.children[2].status, Status::Finished(Outcome::NotRun));
}

#[test]
fn cancelling_a_suspended_async_operation_abandons_its_active_stage() {
    use core::future::Future;
    use core::task::{Context, Poll, Waker};
    async fn operation(pulse: &dyn Pulse) -> Result<(), StopReason> {
        let (mut stages, [decode_key, _resize_key, _optional_key]) = Stages::define(pulse, parts());
        let stage = stages.begin(decode_key);
        core::future::pending::<()>().await;
        stage.complete(Ok::<_, StopReason>(()))?;
        stages.complete(Ok(()))
    }
    fn is_send<T: Send>(_: &T) {}
    let root = tree();
    let mut task = Box::pin(operation(&root));
    is_send(&task);
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(task.as_mut().poll(&mut context), Poll::Pending);
    assert_eq!(
        root.observer().snapshot().children[0].status,
        Status::Running
    );
    drop(task);
    let snap = root.observer().snapshot();
    assert_eq!(
        snap.children[0].status,
        Status::Finished(Outcome::Abandoned)
    );
    assert_eq!(snap.children[1].status, Status::Finished(Outcome::NotRun));
    assert_eq!(snap.children[2].status, Status::Finished(Outcome::NotRun));
}

#[test]
fn reservations_survive_moves_and_cannot_alias_another_identical_plan() {
    let pulse = Issues {
        errors: Mutex::new(Vec::new()),
    };
    let (first, [first_key]) = Stages::define(&pulse, [parts()[0]]);
    // Drop the original plan while retaining its key. Allocator address reuse
    // must not make this stale key valid for a later plan.
    drop(first);
    let (mut second, [second_key]) = Stages::define(&pulse, [parts()[0]]);
    second.begin(first_key).complete_as(Outcome::Succeeded);
    second.begin(second_key).complete_as(Outcome::Succeeded);
    second.complete_as(Outcome::Succeeded);
    assert_eq!(
        *pulse.errors.lock().unwrap(),
        [
            PlanError::Unsupported,
            PlanError::Unsupported,
            PlanError::StageMismatch,
        ]
    );

    let root = tree();
    let (stages, [decode, _resize, optional]) = Stages::define(&root, parts());
    drop(optional); // A discarded reservation does not own completion.
    let mut moved = Box::new(stages);
    let stage = moved.begin(decode);
    stage.complete_as(Outcome::Succeeded);
    (*moved).complete_as(Outcome::Succeeded);
    let snap = root.observer().snapshot();
    assert_eq!(
        snap.children[0].status,
        Status::Finished(Outcome::Succeeded)
    );
    assert_eq!(snap.children[1].status, Status::Finished(Outcome::Skipped));
    assert_eq!(snap.children[2].status, Status::Finished(Outcome::Skipped));
    assert!(snap.children[2].completion_inferred);
}

#[test]
fn fork_joins_different_data_and_results_with_a_nested_sequence() {
    let root = tree();
    let (mut stages, [analysis, after]) = Stages::define(
        &root,
        [
            PhaseSpec::new("analysis", 4, Total::Unknown),
            PhaseSpec::new("after", 1, Total::Unknown),
        ],
    );
    let analysis = stages.begin(analysis);
    let [pixels_stage, metadata_stage] = analysis.fork([
        PhaseSpec::new("pixels", 3, Total::Unknown),
        PhaseSpec::new("metadata", 1, Total::Exact(1)),
    ]);
    let snap = root.observer().snapshot();
    assert_eq!(snap.children[0].execution, Execution::ForkJoin);
    for child in &snap.children[0].children {
        assert_eq!(child.status, Status::Running);
    }
    let pixels = vec![7, 9];
    let metadata = String::from("profile");
    let barrier = std::sync::Barrier::new(2);
    let (pixels_result, metadata_result) = std::thread::scope(|scope| {
        let barrier = &barrier;
        let pixels_task = scope.spawn(move || {
            barrier.wait();
            // Each branch can plan its own sequential operations.
            pipeline(&pixels, &pixels_stage).finish_phase(pixels_stage)
        });
        let metadata_task = scope.spawn(move || {
            barrier.wait();
            let result = metadata_stage.step(1).map(|()| metadata);
            result.finish_phase(metadata_stage)
        });
        (pixels_task.join().unwrap(), metadata_task.join().unwrap())
    });
    let result = match (pixels_result, metadata_result) {
        (Ok(count), Ok(metadata)) => Ok((count, metadata)),
        (Err(error), _) | (_, Err(error)) => Err(error),
    };
    assert_eq!(
        result.finish_phase(analysis),
        Ok((2, String::from("profile")))
    );
    stages.begin(after).complete_as(Outcome::Succeeded);
    stages.complete_as(Outcome::Succeeded);
    let snap = root.observer().snapshot();
    assert_eq!(snap.status, Status::Running);
    assert_eq!(
        snap.children[0].status,
        Status::Finished(Outcome::Succeeded)
    );
    assert_eq!(
        snap.children[1].status,
        Status::Finished(Outcome::Succeeded)
    );
    let branches = &snap.children[0].children;
    assert_eq!(branches[0].children[0].completed, 2);
    assert_eq!(branches[0].children[1].completed, 1);
    assert_eq!(
        branches[0].children[2].status,
        Status::Finished(Outcome::Skipped)
    );
    assert_eq!(branches[1].completed, 1);
}

#[test]
fn branch_failure_does_not_finish_its_sibling_and_parent_result_resolves_the_tail() {
    for outcome in [Outcome::Failed, Outcome::Cancelled] {
        let root = tree();
        let (mut stages, [work, _later]) = Stages::define(&root, [parts()[0], parts()[1]]);
        let work = stages.begin(work);
        let [left, right] = work.fork([parts()[0], parts()[1]]);
        left.complete_as(outcome);
        let snap = root.observer().snapshot();
        assert_eq!(snap.children[0].status, Status::Running);
        assert_eq!(
            snap.children[0].children[0].status,
            Status::Finished(outcome)
        );
        assert_eq!(snap.children[0].children[1].status, Status::Running);
        assert_eq!(snap.children[1].status, Status::NotStarted);
        // The sibling can still do useful work after the first branch fails.
        right.step(1).finish_phase(right).unwrap();
        work.complete_as(outcome);
        let snap = root.observer().snapshot();
        assert_eq!(snap.children[0].status, Status::Finished(outcome));
        assert_eq!(
            snap.children[0].children[1].status,
            Status::Finished(Outcome::Succeeded)
        );
        assert_eq!(snap.children[1].status, Status::Finished(Outcome::NotRun));
        assert!(snap.children[1].completion_inferred);
    }
}

#[test]
fn fork_drop_and_unwind_preserve_abandonment() {
    for panic in [false, true] {
        let root = tree();
        let (mut stages, [work, _later]) = Stages::define(&root, [parts()[0], parts()[1]]);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let work = stages.begin(work);
            let [left, right] = work.fork([parts()[0], parts()[1]]);
            right.complete_as(Outcome::Succeeded);
            if panic {
                panic!("branch panicked");
            }
            drop(left);
            drop(work);
        }));
        assert_eq!(result.is_err(), panic);
        let snap = root.observer().snapshot();
        assert_eq!(
            snap.children[0].status,
            Status::Finished(Outcome::Abandoned)
        );
        assert_eq!(
            snap.children[0].children[0].status,
            Status::Finished(Outcome::Abandoned)
        );
        assert_eq!(
            snap.children[0].children[1].status,
            Status::Finished(Outcome::Succeeded)
        );
        assert_eq!(snap.children[1].status, Status::Finished(Outcome::NotRun));
    }
}

#[test]
fn rejected_forks_preserve_cancellation_in_every_branch() {
    let pulse = Issues {
        errors: Mutex::new(Vec::new()),
    };
    // A child of a rejected sequence is untracked but can still plan valid
    // children. Use a zero weight to reject the nested fork too.
    let (mut stages, [work]) = Stages::define(&pulse, [parts()[0]]);
    let work = stages.begin(work);
    let [left, right] = work.fork([PhaseSpec::new("invalid", 0, Total::Unknown), parts()[1]]);
    assert_eq!(left.check(), Err(StopReason::Cancelled));
    assert_eq!(right.check(), Err(StopReason::Cancelled));
    left.complete_as(Outcome::Cancelled);
    right.complete_as(Outcome::Cancelled);
    work.complete_as(Outcome::Cancelled);
    stages.complete_as(Outcome::Cancelled);
    assert_eq!(
        *pulse.errors.lock().unwrap(),
        [PlanError::Unsupported, PlanError::EmptyOrZeroWeight]
    );
}

#[test]
fn runtime_sized_sequences_use_the_same_handle_and_handoff() {
    let root = tree();
    let specs: Vec<_> = parts().into_iter().collect();
    let mut stages = Stages::new(&root, &specs);
    for _ in &specs {
        let stage = stages.begin_next();
        stage.check().finish_phase(stage).unwrap();
    }
    stages.complete_as(Outcome::Succeeded);
    for child in root.observer().snapshot().children {
        assert_eq!(child.status, Status::Finished(Outcome::Succeeded));
    }
}
