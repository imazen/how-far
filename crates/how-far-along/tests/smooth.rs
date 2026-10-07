//! Display-side smoothing between reports (the `smooth` feature).
#![cfg(feature = "smooth")]

use how_far_along::{
    Complete, Execution, Observer, Outcome, Phase, PhaseSpec, ProgressSmoother, Report, Total,
};
use std::time::Duration;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn at(smooth: &mut ProgressSmoother, observer: &Observer, now: u64) -> f64 {
    smooth
        .display_fraction(&observer.snapshot(), ms(now))
        .unwrap()
}

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

#[test]
fn evenly_spaced_reports_are_extrapolated_up_to_the_next_one() {
    let mut job = Phase::new("rows", Total::Exact(100));
    job.start().unwrap();
    let (reporter, observer) = (job.reporter(), job.observer());
    let mut smooth = ProgressSmoother::new();
    near(at(&mut smooth, &observer, 0), 0.0);
    // Ten rows every 50 ms.
    reporter.advance(10);
    near(at(&mut smooth, &observer, 50), 0.10);
    near(at(&mut smooth, &observer, 60), 0.12);
    near(at(&mut smooth, &observer, 75), 0.15);
    reporter.advance(10);
    near(at(&mut smooth, &observer, 100), 0.20);
    near(at(&mut smooth, &observer, 140), 0.28);
    // A late report holds the display at the next expected count.
    near(at(&mut smooth, &observer, 400), 0.30);
    reporter.advance(10);
    near(at(&mut smooth, &observer, 410), 0.30);
}

#[test]
fn a_smaller_report_than_expected_never_moves_the_display_back() {
    let mut job = Phase::new("tiles", Total::Exact(100));
    job.start().unwrap();
    let (reporter, observer) = (job.reporter(), job.observer());
    let mut smooth = ProgressSmoother::new();
    at(&mut smooth, &observer, 0);
    reporter.advance(10);
    at(&mut smooth, &observer, 100);
    near(at(&mut smooth, &observer, 190), 0.19);
    // Only 5 more units arrive; the display holds at 0.19 until work passes it.
    reporter.advance(5);
    near(at(&mut smooth, &observer, 200), 0.19);
    reporter.advance(10);
    assert!(at(&mut smooth, &observer, 210) >= 0.25);
}

#[test]
fn the_estimate_never_passes_the_total_or_runs_before_a_pace_is_known() {
    let mut job = Phase::new("short", Total::Exact(3));
    job.start().unwrap();
    let (reporter, observer) = (job.reporter(), job.observer());
    let mut smooth = ProgressSmoother::new();
    // Attached after work began: no change seen yet, so no pace.
    reporter.advance(2);
    near(at(&mut smooth, &observer, 0), 2.0 / 3.0);
    near(at(&mut smooth, &observer, 5_000), 2.0 / 3.0);
    reporter.advance(1);
    near(at(&mut smooth, &observer, 5_010), 1.0);
    near(at(&mut smooth, &observer, 9_000), 1.0);
}

#[test]
fn each_stage_keeps_its_own_pace_within_the_weighted_tree() {
    let mut job = Phase::new("convert", Total::Unknown);
    let mut stages = job
        .split_vec(
            Execution::Sequence,
            &[
                PhaseSpec::new("decode", 1, Total::Exact(10)),
                PhaseSpec::new("encode", 3, Total::Exact(10)),
            ],
        )
        .unwrap();
    let observer = job.observer();
    let mut smooth = ProgressSmoother::new();
    let (decode, encode) = (stages[0].reporter(), stages[1].reporter());
    stages[0].start().unwrap();
    at(&mut smooth, &observer, 0);
    // Decode: one unit every 10 ms.
    for i in 1..=10 {
        decode.advance(1);
        at(&mut smooth, &observer, 10 * i);
    }
    stages[0].finish().unwrap();
    near(at(&mut smooth, &observer, 100), 0.25);
    // Encode starts slower: one unit every 100 ms. The fast decode pace is
    // not carried over: until encode has changed once, it shows its count.
    stages[1].start().unwrap();
    near(at(&mut smooth, &observer, 110), 0.25);
    encode.advance(1);
    near(at(&mut smooth, &observer, 210), 0.25 + 0.75 * 0.1);
    near(at(&mut smooth, &observer, 260), 0.25 + 0.75 * 0.15);
    encode.advance(9);
    stages[1].finish().unwrap();
    job.finish().unwrap();
    near(at(&mut smooth, &observer, 300), 1.0);
}

#[test]
fn unknown_totals_are_none_and_finished_leaves_are_forgotten() {
    let job = Phase::new("unknown", Total::Unknown);
    let mut smooth = ProgressSmoother::new();
    job.reporter().advance(5);
    assert_eq!(
        smooth.display_fraction(&job.observer().snapshot(), ms(0)),
        None
    );

    let mut smooth = ProgressSmoother::new();
    let mut job = Phase::new("known", Total::Exact(4));
    job.start().unwrap();
    let observer = job.observer();
    job.reporter().advance(1);
    at(&mut smooth, &observer, 0);
    job.reporter().advance(1);
    at(&mut smooth, &observer, 10);
    job.complete_as(Outcome::Succeeded);
    near(at(&mut smooth, &observer, 20), 1.0);
    // The finished leaf's pace was dropped; the display stays at 1.
    assert!(format!("{smooth:?}").contains("tracked_paces: 0"));
}

#[test]
fn a_count_past_an_estimated_total_shows_no_more_than_done() {
    let mut job = Phase::new("rows", Total::Estimated(10));
    job.start().unwrap();
    let (reporter, observer) = (job.reporter(), job.observer());
    let mut smooth = ProgressSmoother::new();
    at(&mut smooth, &observer, 0);
    reporter.advance(15);
    near(at(&mut smooth, &observer, 10), 1.0);
    near(at(&mut smooth, &observer, 20), 1.0);
}

#[test]
fn an_overrun_stage_counts_as_done_not_more() {
    let mut job = Phase::new("convert", Total::Unknown);
    let mut stages = job
        .split_vec(
            Execution::Sequence,
            &[
                PhaseSpec::new("decode", 1, Total::Estimated(10)),
                PhaseSpec::new("encode", 1, Total::Exact(10)),
            ],
        )
        .unwrap();
    let observer = job.observer();
    let mut smooth = ProgressSmoother::new();
    let decode = stages[0].reporter();
    stages[0].start().unwrap();
    at(&mut smooth, &observer, 0);
    decode.advance(20);
    let snapshot = observer.snapshot().fraction().unwrap();
    near(snapshot, 0.5);
    near(at(&mut smooth, &observer, 10), snapshot);
    near(at(&mut smooth, &observer, 20), snapshot);
}

#[test]
fn a_revised_total_starts_the_stage_over_from_its_count() {
    let mut job = Phase::new("tiles", Total::Exact(100));
    job.start().unwrap();
    let (reporter, observer) = (job.reporter(), job.observer());
    let mut smooth = ProgressSmoother::new();
    at(&mut smooth, &observer, 0);
    reporter.advance(50);
    near(at(&mut smooth, &observer, 100), 0.5);
    // Extrapolated half the work left past the 50 units done.
    near(at(&mut smooth, &observer, 200), 0.75);
    // 50 of 60: the snapshot's own fraction.
    job.set_total(Total::Exact(60)).unwrap();
    near(at(&mut smooth, &observer, 200), 50.0 / 60.0);
    // A raised total steps the display back to the work done, with no pace
    // until the stage is seen to change again.
    job.set_total(Total::Exact(1000)).unwrap();
    near(at(&mut smooth, &observer, 200), 0.05);
    near(at(&mut smooth, &observer, 5_000), 0.05);
    // Unknown, then known again: none, then the count.
    job.set_total(Total::Unknown).unwrap();
    assert_eq!(
        smooth.display_fraction(&observer.snapshot(), ms(5_100)),
        None
    );
    job.set_total(Total::Exact(1000)).unwrap();
    near(at(&mut smooth, &observer, 5_200), 0.05);
}

#[test]
fn a_stage_that_fails_or_is_cancelled_shows_its_recorded_work() {
    for outcome in [Outcome::Failed, Outcome::Cancelled, Outcome::Abandoned] {
        let mut job = Phase::new("tiles", Total::Exact(100));
        job.start().unwrap();
        let (reporter, observer) = (job.reporter(), job.observer());
        let mut smooth = ProgressSmoother::new();
        at(&mut smooth, &observer, 0);
        reporter.advance(50);
        at(&mut smooth, &observer, 100);
        near(at(&mut smooth, &observer, 200), 0.75);
        job.finish_with(outcome).unwrap();
        near(at(&mut smooth, &observer, 201), 0.5);
    }
}

#[test]
fn a_failed_stage_in_a_running_tree_shows_the_tree_s_record() {
    let mut job = Phase::new("convert", Total::Unknown);
    let mut stages = job
        .split_vec(
            Execution::Sequence,
            &[
                PhaseSpec::new("decode", 1, Total::Exact(10)),
                PhaseSpec::new("encode", 3, Total::Exact(10)),
            ],
        )
        .unwrap();
    let observer = job.observer();
    let mut smooth = ProgressSmoother::new();
    stages[0].start().unwrap();
    at(&mut smooth, &observer, 0);
    stages[0].reporter().advance(4);
    at(&mut smooth, &observer, 100);
    // Decode extrapolated by half the 6 units left, to 7 of 10: a quarter of 0.7.
    near(at(&mut smooth, &observer, 200), 0.175);
    stages[0].finish_with(Outcome::Failed).unwrap();
    let recorded = observer.snapshot().fraction().unwrap();
    near(recorded, 0.1);
    near(at(&mut smooth, &observer, 201), recorded);
}

#[test]
fn extrapolation_moves_at_most_the_latest_change_and_never_completes_a_stage() {
    // A batch of 100, then one of 1: the display runs one unit ahead, not
    // the average batch.
    let mut job = Phase::new("rows", Total::Exact(1000));
    job.start().unwrap();
    let (reporter, observer) = (job.reporter(), job.observer());
    let mut smooth = ProgressSmoother::new();
    at(&mut smooth, &observer, 0);
    reporter.advance(100);
    at(&mut smooth, &observer, 100);
    reporter.advance(1);
    at(&mut smooth, &observer, 200);
    near(at(&mut smooth, &observer, 300), 0.102);

    // Half the work in one change, then nothing: the display stops half-way
    // through the half left, and only reported work completes the stage.
    let mut job = Phase::new("tiles", Total::Exact(100));
    job.start().unwrap();
    let (reporter, observer) = (job.reporter(), job.observer());
    let mut smooth = ProgressSmoother::new();
    at(&mut smooth, &observer, 0);
    reporter.advance(50);
    at(&mut smooth, &observer, 100);
    near(at(&mut smooth, &observer, 10_000), 0.75);
    reporter.advance(50);
    near(at(&mut smooth, &observer, 10_001), 1.0);
}

#[test]
fn changes_seen_at_one_time_are_one_change() {
    let mut job = Phase::new("rows", Total::Exact(100));
    job.start().unwrap();
    let (reporter, observer) = (job.reporter(), job.observer());
    let (mut split, mut whole) = (ProgressSmoother::new(), ProgressSmoother::new());
    at(&mut split, &observer, 0);
    at(&mut whole, &observer, 0);
    reporter.advance(1);
    at(&mut split, &observer, 100);
    reporter.advance(1);
    at(&mut split, &observer, 100);
    // An earlier time counts as the same time.
    reporter.advance(1);
    at(&mut split, &observer, 50);
    at(&mut whole, &observer, 100);
    // Three units in 100 ms: half-way to the next change, 1.5 units ahead.
    near(at(&mut split, &observer, 150), 0.045);
    for now in [150, 175, 199, 400] {
        near(
            at(&mut split, &observer, now),
            at(&mut whole, &observer, now),
        );
    }
}

#[test]
fn a_stall_does_not_slow_the_pace_after_it() {
    let mut job = Phase::new("rows", Total::Exact(100));
    job.start().unwrap();
    let (reporter, observer) = (job.reporter(), job.observer());
    let mut smooth = ProgressSmoother::new();
    at(&mut smooth, &observer, 0);
    // Five seconds of setup, then a row every 100 ms.
    reporter.advance(1);
    at(&mut smooth, &observer, 5_000);
    for row in 1..=10 {
        reporter.advance(1);
        at(&mut smooth, &observer, 5_000 + 100 * row);
    }
    // Half-way to row 12, the display is half a row ahead of row 11.
    near(at(&mut smooth, &observer, 6_050), 0.115);
}

#[test]
fn the_same_or_an_earlier_time_holds_the_display() {
    let mut job = Phase::new("rows", Total::Exact(10));
    job.start().unwrap();
    let (reporter, observer) = (job.reporter(), job.observer());
    let mut smooth = ProgressSmoother::new();
    at(&mut smooth, &observer, 0);
    reporter.advance(1);
    at(&mut smooth, &observer, 100);
    let shown = at(&mut smooth, &observer, 150);
    near(at(&mut smooth, &observer, 150), shown);
    near(at(&mut smooth, &observer, 120), shown);
}
