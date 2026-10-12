//! Application-planned trees: owned `Phase` handles and `Reporter`s.

use how_far_along::ext::ReportExt;
use how_far_along::{
    Execution, Outcome, Phase, PhaseSpec, PlanError, Report, Snapshot, Status, Total,
};
use rayon::prelude::*;
use std::{
    num::{NonZeroU64, NonZeroUsize},
    sync::{Arc, Barrier, mpsc},
};

fn near(a: Option<f64>, b: f64) {
    assert!((a.unwrap() - b).abs() < 1e-12, "{a:?} != {b}");
}

#[test]
fn serial_parallel_join_serial_keeps_middle_budget_at_thirty_percent() {
    let mut job = Phase::new("job", Total::Unknown);
    let observer = job.observer();
    let [mut before, mut middle, mut after] = job
        .split(
            Execution::Sequence,
            [
                PhaseSpec::new("before", 35, Total::Exact(1)),
                PhaseSpec::new("middle", 30, Total::Unknown),
                PhaseSpec::new("after", 35, Total::Exact(1)),
            ],
        )
        .unwrap();
    let [mut small, mut slow] = middle
        .split(
            Execution::ForkJoin,
            [
                PhaseSpec::new("small", 1, Total::Exact(1)),
                PhaseSpec::new("slow", 1, Total::Exact(100)),
            ],
        )
        .unwrap();
    before.finish().unwrap();
    near(observer.snapshot().fraction(), 0.35);
    assert!(!middle.reporter().may_report());
    middle.reporter().advance(999); // Branch work never double counts.
    let (done_tx, done_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            small.reporter().advance(1);
            small.finish().unwrap();
            done_tx.send(()).unwrap();
        });
        scope.spawn(move || {
            release_rx.recv().unwrap();
            slow.reporter().advance(100);
            slow.finish().unwrap();
        });
        done_rx.recv().unwrap();
        near(observer.snapshot().fraction(), 0.50);
        assert_eq!(middle.finish(), Err(PlanError::UnfinishedChildren));
        assert_eq!(job.finish(), Err(PlanError::UnfinishedChildren));
        release_tx.send(()).unwrap();
    });
    middle.finish().unwrap();
    near(observer.snapshot().fraction(), 0.65);
    after.finish().unwrap();
    assert_eq!(observer.snapshot().status, Status::Running); // Counted 100% is not an outcome.
    job.finish().unwrap();
    assert_eq!(
        observer.snapshot().status,
        Status::Finished(Outcome::Succeeded)
    );
}

#[test]
fn repeated_nested_joins_and_skipped_reduction() {
    let mut job = Phase::new("job", Total::Unknown);
    let mut groups = job
        .split(
            Execution::Sequence,
            [
                PhaseSpec::new("first", 1, Total::Unknown),
                PhaseSpec::new("second", 1, Total::Unknown),
            ],
        )
        .unwrap();
    for group in &mut groups {
        let [mut fork, mut reduce] = group
            .split(
                Execution::Sequence,
                [
                    PhaseSpec::new("fork", 9, Total::Unknown),
                    PhaseSpec::new("reduce", 1, Total::Unknown),
                ],
            )
            .unwrap();
        let names: Vec<String> = (0..4).map(|i| format!("part{i}")).collect();
        let parts: Vec<_> = (0..4)
            .map(|i| PhaseSpec::new(&names[i], i as u64 + 1, Total::Exact(i as u64)))
            .collect();
        let children = fork.split_vec(Execution::ForkJoin, &parts).unwrap();
        std::thread::scope(|scope| {
            for mut child in children {
                scope.spawn(move || child.finish().unwrap());
            }
        });
        fork.finish().unwrap();
        reduce.finish_with(Outcome::Skipped).unwrap();
        group.finish().unwrap();
    }
    job.finish().unwrap();
    near(job.observer().snapshot().fraction(), 1.0);
}

#[test]
fn rayon_dynamic_asymmetric_tasks_share_one_count_and_flush_batches() {
    let mut job = Phase::new("pool", Total::Exact(1001));
    job.set_execution(Execution::work_pool(NonZeroUsize::new(4).unwrap()))
        .unwrap();
    let reporter = job.reporter();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    pool.install(|| {
        (0..1001).into_par_iter().for_each_init(
            || reporter.clone().batched(NonZeroU64::new(16).unwrap()),
            |batch, item| {
                for i in 0..item % 19 {
                    std::hint::black_box(i * item);
                }
                batch.advance(1);
            },
        );
    });
    assert_eq!(job.observer().snapshot().completed, 1001);
    job.finish().unwrap();
}

#[test]
fn manual_workers_sum_exactly_and_terminal_snapshot_ignores_stale_reporters() {
    let mut job = Phase::new("manual", Total::Exact(8000));
    let reporter = Arc::new(job.reporter());
    let barrier = Arc::new(Barrier::new(8));
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let reporter = &reporter;
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                for _ in 0..1000 {
                    reporter.advance(1);
                }
            });
        }
    });
    job.finish().unwrap();
    let final_snapshot = job.observer().snapshot();
    assert_eq!(final_snapshot.completed, 8000);
    reporter.advance(u64::MAX);
    assert_eq!(job.observer().snapshot(), final_snapshot);
    assert_eq!(job.set_total(Total::Exact(10)), Err(PlanError::Finished));
    let fresh_attempt = Phase::new("manual retry", Total::Exact(8000));
    assert_eq!(fresh_attempt.observer().snapshot().completed, 0);
}

#[test]
fn exact_estimated_unknown_zero_overrun_and_overflow_are_distinct() {
    let mut job = Phase::new("unknown", Total::Unknown);
    let reporter = job.reporter();
    reporter.advance(4);
    assert_eq!(job.observer().snapshot().fraction(), None);
    job.set_total(Total::Estimated(8)).unwrap();
    near(job.observer().snapshot().fraction(), 0.5);
    job.set_total(Total::Estimated(16)).unwrap();
    near(job.observer().snapshot().fraction(), 0.25);
    job.set_total(Total::Exact(2)).unwrap();
    let snapshot = job.observer().snapshot();
    assert_eq!(snapshot.initial_total, Total::Unknown);
    assert_eq!(snapshot.total_revisions.len(), 3);
    assert!(snapshot.overrun());
    assert_eq!(snapshot.status, Status::Running);
    let mut zero = Phase::new("empty", Total::Exact(0));
    near(zero.observer().snapshot().fraction(), 0.0);
    zero.finish().unwrap();
    near(zero.observer().snapshot().fraction(), 1.0);
    let huge = Phase::new("overflow", Total::Unknown);
    huge.reporter().advance(Snapshot::counter_max());
    assert!(!huge.observer().snapshot().overflowed);
    huge.reporter().advance(1);
    assert!(huge.observer().snapshot().overflowed);
    assert_eq!(
        huge.observer().snapshot().completed,
        Snapshot::counter_max()
    );
    assert_eq!(huge.observer().snapshot().fraction(), None);
}

#[test]
fn unknown_subtree_keeps_reserved_budget_and_failures_do_not_discharge_it() {
    let mut job = Phase::new("job", Total::Unknown);
    let [mut known, mut unknown] = job
        .split(
            Execution::Sequence,
            [
                PhaseSpec::new("known", 7, Total::Exact(10)),
                PhaseSpec::new("discovery", 3, Total::Unknown),
            ],
        )
        .unwrap();
    known.finish().unwrap();
    assert_eq!(job.observer().snapshot().fraction(), Some(0.7));
    assert!((job.observer().snapshot().unresolved_fraction() - 0.3).abs() < 1e-12);
    unknown.reporter().advance(2);
    unknown.finish_with(Outcome::Cancelled).unwrap();
    job.finish_with(Outcome::Cancelled).unwrap();
    assert_eq!(job.observer().snapshot().fraction(), Some(0.7));
}

#[test]
fn an_invalid_plan_fails_as_no_pulse_does_and_changes_nothing() {
    use how_far_along::{NoPulse, Pulse};
    let plans: [&[PhaseSpec<'_>]; 4] = [
        &[],
        &[PhaseSpec::new("zero", 0, Total::Unknown)],
        &[
            PhaseSpec::new("a", u64::MAX, Total::Unknown),
            PhaseSpec::new("b", 1, Total::Unknown),
        ],
        // Invalid twice over: both report the problem found first.
        &[
            PhaseSpec::new("a", u64::MAX, Total::Unknown),
            PhaseSpec::new("b", 1, Total::Unknown),
            PhaseSpec::new("zero", 0, Total::Unknown),
        ],
    ];
    let mut job = Phase::new("plan", Total::Unknown);
    for parts in plans {
        let no_pulse = NoPulse.split(Execution::Sequence, parts);
        let tree = job.split_vec(Execution::Sequence, parts);
        assert_eq!(tree.err(), no_pulse.err(), "{parts:?}");
    }
    job.set_units("bytes").unwrap();
    let _reporter = job.reporter();
    assert_eq!(job.set_units("rows"), Err(PlanError::AlreadyInUse));
    assert!(matches!(
        job.split(
            Execution::Sequence,
            [PhaseSpec::new("a", 1, Total::Unknown)]
        ),
        Err(PlanError::AlreadyInUse)
    ));
}

#[test]
fn every_outcome_is_recorded_and_stays_put() {
    let mut job = Phase::new("job", Total::Unknown);
    let observer = job.observer();
    let parts = [
        PhaseSpec::new("succeeded", 1, Total::Exact(2)),
        PhaseSpec::new("skipped", 1, Total::Exact(2)),
        PhaseSpec::new("cancelled", 1, Total::Exact(2)),
        PhaseSpec::new("failed", 1, Total::Exact(2)),
        PhaseSpec::new("abandoned", 1, Total::Exact(2)),
    ];
    let mut children = job.split_vec(Execution::Sequence, &parts).unwrap();
    let abandoned = children.pop().unwrap();
    abandoned.reporter().advance(1);
    drop(abandoned);
    let outcomes = [
        Outcome::Succeeded,
        Outcome::Skipped,
        Outcome::Cancelled,
        Outcome::Failed,
    ];
    for (child, outcome) in children.iter_mut().zip(outcomes) {
        child.reporter().advance(1);
        child.finish_with(outcome).unwrap();
    }
    job.finish_with(Outcome::Failed).unwrap();
    let snapshot = observer.snapshot();
    assert_eq!(snapshot.status, Status::Finished(Outcome::Failed));
    let statuses: Vec<_> = snapshot.children.iter().map(|c| c.status).collect();
    assert_eq!(
        statuses,
        [
            Status::Finished(Outcome::Succeeded),
            Status::Finished(Outcome::Skipped),
            Status::Finished(Outcome::Cancelled),
            Status::Finished(Outcome::Failed),
            Status::Finished(Outcome::Abandoned),
        ]
    );
    assert!(snapshot.children.iter().all(|c| c.completed == 1));
    for child in &children {
        child.reporter().advance(1);
    }
    assert_eq!(observer.snapshot(), snapshot);
}

#[test]
fn abandoning_owner_freezes_even_if_child_owner_lives_on() {
    let mut job = Phase::new("parent", Total::Unknown);
    let observer = job.observer();
    let [mut child] = job
        .split(
            Execution::Sequence,
            [PhaseSpec::new("child", 1, Total::Exact(3))],
        )
        .unwrap();
    let reporter = child.reporter();
    reporter.advance(1);
    drop(job);
    let frozen = observer.snapshot();
    assert_eq!(frozen.status, Status::Finished(Outcome::Abandoned));
    reporter.advance(2);
    child.finish().unwrap();
    assert_eq!(observer.snapshot(), frozen);
}

#[test]
fn snapshots_during_metadata_publication_keep_each_revision_coherent() {
    let mut job = Phase::new("updates", Total::Estimated(1));
    let observer = job.observer();
    let reporter = job.reporter();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            for _ in 0..500 {
                let snapshot = observer.snapshot();
                assert_eq!(
                    snapshot.total,
                    snapshot
                        .total_revisions
                        .last()
                        .copied()
                        .unwrap_or(snapshot.initial_total)
                );
            }
        });
        scope.spawn(move || {
            for _ in 0..500 {
                reporter.advance(1);
            }
        });
        for n in 2..100 {
            job.set_total(Total::Estimated(n)).unwrap();
        }
    });
    job.finish().unwrap();
    assert_eq!(observer.snapshot().completed, 500);
}

#[test]
fn sharing_contract_and_owned_trait_objects() {
    fn shared<T: Send + Sync>() {}
    shared::<Phase>();
    shared::<how_far_along::Reporter>();
    shared::<how_far_along::Observer>();
    let phase = Phase::new("erased", Total::Exact(5));
    let boxed: Box<dyn Report> = Box::new(phase.reporter());
    let arc: Arc<dyn Report> = Arc::new(phase.reporter());
    boxed.advance(2);
    arc.advance(3);
    assert_eq!(phase.observer().snapshot().completed, 5);
}

#[test]
fn only_a_leaf_has_its_own_total_and_a_failed_finish_can_be_retried() {
    let mut job = Phase::new("job", Total::Unknown);
    let [mut child] = job
        .split(
            Execution::Sequence,
            [PhaseSpec::new("child", 1, Total::Exact(1))],
        )
        .unwrap();
    assert_eq!(job.set_total(Total::Exact(5)), Err(PlanError::NotALeaf));
    assert_eq!(job.finish(), Err(PlanError::UnfinishedChildren));
    child.reporter().advance(1);
    child.finish().unwrap();
    job.finish().unwrap();
    assert_eq!(job.observer().snapshot().fraction(), Some(1.0));
}

#[test]
fn weighted_progress_advances_before_an_unknown_optional_stage_is_resolved() {
    let mut job = Phase::new("export", Total::Unknown);
    let [mut resize, mut optional] = job
        .split(
            Execution::Sequence,
            [
                PhaseSpec::new("resize", 1, Total::Exact(4)),
                PhaseSpec::new("optional", 1, Total::Unknown),
            ],
        )
        .unwrap();
    let observer = job.observer();
    resize.reporter().advance(1);
    assert_eq!(observer.snapshot().fraction(), Some(0.125));
    assert_eq!(observer.summary().fraction, Some(0.125));
    assert_eq!(observer.snapshot().unresolved_fraction(), 0.5);
    assert_eq!(observer.summary().unresolved_fraction, 0.5);
    assert_eq!(optional.observer().snapshot().fraction(), None);
    resize.reporter().advance(1);
    assert_eq!(observer.snapshot().fraction(), Some(0.25));
    optional.finish_with(Outcome::Skipped).unwrap();
    assert_eq!(observer.snapshot().fraction(), Some(0.75));
    assert_eq!(observer.summary().fraction, Some(0.75));
    assert_eq!(observer.snapshot().unresolved_fraction(), 0.0);
    resize.finish().unwrap();
    job.finish().unwrap();
    assert_eq!(observer.snapshot().fraction(), Some(1.0));
    assert_eq!(observer.summary().fraction, Some(1.0));
}

#[test]
fn nested_unknown_budgets_keep_known_progress_in_live_and_frozen_observations() {
    let mut job = Phase::new("job", Total::Unknown);
    let [mut branch, mut unknown_outer] = job
        .split(
            Execution::ForkJoin,
            [
                PhaseSpec::new("branch", 1, Total::Unknown),
                PhaseSpec::new("unknown outer", 1, Total::Unknown),
            ],
        )
        .unwrap();
    let [mut known, mut unknown_inner] = branch
        .split(
            Execution::ForkJoin,
            [
                PhaseSpec::new("known", 1, Total::Exact(4)),
                PhaseSpec::new("unknown inner", 1, Total::Unknown),
            ],
        )
        .unwrap();
    known.reporter().advance(2);
    let observer = job.observer();
    let check = || {
        let snapshot = observer.snapshot();
        assert_eq!(snapshot.fraction(), Some(0.125));
        assert_eq!(snapshot.unresolved_fraction(), 0.75);
        assert_eq!(observer.summary().fraction, snapshot.fraction());
        assert_eq!(
            observer.summary().unresolved_fraction,
            snapshot.unresolved_fraction()
        );
        assert_eq!(observer.try_summary(), Some(observer.summary()));
        let mut json = String::new();
        snapshot.write_json(&mut json).unwrap();
        let root = json.split("\"children\":").next().unwrap();
        assert!(root.contains("\"fraction\":0.125,"));
        assert!(root.contains("\"unresolved_fraction\":0.75,"));
    };
    check();
    known.finish_with(Outcome::Cancelled).unwrap();
    unknown_inner.finish_with(Outcome::Cancelled).unwrap();
    branch.finish_with(Outcome::Cancelled).unwrap();
    unknown_outer.finish_with(Outcome::Cancelled).unwrap();
    job.finish_with(Outcome::Cancelled).unwrap();
    check();
    assert_eq!(
        observer.snapshot().status,
        Status::Finished(Outcome::Cancelled)
    );
}

#[test]
fn all_unknown_and_overflowed_children_reserve_their_weight_without_hiding_siblings() {
    let mut job = Phase::new("job", Total::Unknown);
    let [mut left, mut right] = job
        .split(
            Execution::ForkJoin,
            [
                PhaseSpec::new("left", 1, Total::Unknown),
                PhaseSpec::new("right", 1, Total::Unknown),
            ],
        )
        .unwrap();
    let observer = job.observer();
    assert_eq!(observer.snapshot().fraction(), Some(0.0));
    assert_eq!(observer.snapshot().unresolved_fraction(), 1.0);
    assert_eq!(observer.summary().fraction, Some(0.0));
    assert_eq!(observer.summary().unresolved_fraction, 1.0);
    left.set_total(Total::Exact(4)).unwrap();
    left.reporter().advance(2);
    right.set_total(Total::Exact(8)).unwrap();
    right.reporter().advance(Snapshot::counter_max());
    right.reporter().advance(1);
    assert!(right.observer().snapshot().overflowed);
    assert_eq!(right.observer().snapshot().fraction(), None);
    assert_eq!(observer.snapshot().fraction(), Some(0.25));
    assert_eq!(observer.snapshot().unresolved_fraction(), 0.5);
    assert_eq!(observer.summary().fraction, Some(0.25));
    assert_eq!(observer.summary().unresolved_fraction, 0.5);
}
