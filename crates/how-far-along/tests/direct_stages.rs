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
    let mut stages = Stages::new(p, &parts());
    let decode_phase = stages.begin("decode");
    let pixels = decode(input, &decode_phase).finish_phase(decode_phase)?;
    let resize_phase = stages.begin("resize");
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
    let mut stages = Stages::new(&root, &parts());
    let stage = stages.begin("decode");
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
        let mut stages = Stages::new(&root, &parts());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let stage = stages.begin("decode");
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
fn names_are_owned_and_legacy_helpers_can_mix_with_named_handles() {
    let root = tree();
    let mut stages = {
        let name = String::from("第一阶段");
        Stages::new(
            &root,
            &[
                PhaseSpec::new(&name, 1, Total::Exact(1)),
                PhaseSpec::new("same", 1, Total::Unknown),
                PhaseSpec::new("same", 1, Total::Unknown),
                PhaseSpec::new("last", 1, Total::Unknown),
            ],
        )
    };
    let stage = stages.begin("第一阶段");
    stage.step(1).finish_phase(stage).unwrap();
    stages.skip();
    let stage = stages.begin("same");
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
fn rejected_plans_and_wrong_names_keep_cancellation_without_consuming_the_next_stage() {
    let pulse = Issues {
        errors: Mutex::new(Vec::new()),
    };
    let mut stages = Stages::new(&pulse, &parts());
    let wrong = stages.begin("resize");
    assert_eq!(wrong.check(), Err(StopReason::Cancelled));
    wrong.complete_as(Outcome::Succeeded);
    let right = stages.begin("decode");
    assert_eq!(right.check(), Err(StopReason::Cancelled));
    right.complete_as(Outcome::Succeeded);
    let repeated = stages.begin("decode");
    repeated.complete_as(Outcome::Succeeded);
    let right = stages.begin("resize");
    right.complete_as(Outcome::Failed);
    let after_error = stages.begin("optional");
    assert_eq!(after_error.check(), Err(StopReason::Cancelled));
    after_error.complete_as(Outcome::Succeeded);
    assert_eq!(
        *pulse.errors.lock().unwrap(),
        [
            PlanError::Unsupported,
            PlanError::StageNameMismatch,
            PlanError::StageNameMismatch,
            PlanError::Finished,
        ]
    );
}
#[test]
fn a_named_stage_forwards_nesting_owned_workers_and_total_revision() {
    let root = tree();
    let mut stages = Stages::new(&root, &parts());
    let stage = stages.begin("decode");
    stage.set_total(Total::Exact(3)).unwrap();
    let shared = stage.share().unwrap();
    let child_id = root.observer().snapshot().children[0].id;
    std::thread::spawn(move || {
        let mut nested = Stages::new(&shared, &[PhaseSpec::new("nested", 1, Total::Exact(3))]);
        let stage = nested.begin("nested");
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
    let mut stages = Stages::new(&pulse, &[PhaseSpec::new("one", 1, Total::Unknown)]);
    stages.begin("one").complete_as(Outcome::Succeeded);
    let extra = stages.begin("one");
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
        let mut stages = Stages::new(pulse, &parts());
        let stage = stages.begin("decode");
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
        let mut stages = Stages::new(pulse, &parts());
        let stage = stages.begin("decode");
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
