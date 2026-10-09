//! Run with `cargo run -p how-far-example-app --example integration`.
use how_far::{NoPulse, Outcome, StopOnly, StopReason, Total, Unstoppable, prelude::*};
use how_far_along::{FnPulse, Phase, ProgressSmoother, PulseTree, Status};
use how_far_example_codec::Mode;
use how_far_example_pipeline::convert;
use std::time::{Duration, Instant};

fn main() {
    let input = vec![42; 256 * 1024];
    let expected = convert(&input, &NoPulse, Mode::Fast, false).unwrap();

    let token = almost_enough::Stopper::new();
    token.cancel();
    let stopped = convert(&input, &StopOnly::borrowed(&token), Mode::Fast, false);
    assert_eq!(
        stopped.unwrap_err().as_stop_reason(),
        Some(StopReason::Cancelled)
    );

    let tree = PulseTree::new(Phase::new("convert", Total::Unknown), Unstoppable);
    let observer = tree.observer();
    let worker =
        std::thread::spawn(move || convert(&input, &tree, Mode::Fast, false).finish_phase(tree));
    let clock = Instant::now();
    let mut smooth = ProgressSmoother::new();
    while !worker.is_finished() {
        let snapshot = observer.snapshot();
        if let Some(fraction) = smooth.display_fraction(&snapshot, clock.elapsed()) {
            eprintln!("display: {:.1}%", fraction * 100.0);
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    assert_eq!(worker.join().unwrap().unwrap(), expected);
    // Completion does not invoke a callback; always read the final observation.
    let snapshot = observer.snapshot();
    assert_eq!(snapshot.status, Status::Finished(Outcome::Succeeded));
    println!("tracked: {:?}", snapshot.status);

    let callback = FnPulse::new("cancel at a checkpoint", |_| Err(StopReason::Cancelled));
    let observer = callback.observer();
    let result = convert(&[1, 2], &callback, Mode::Fast, false).finish_phase(callback);
    assert_eq!(
        result.unwrap_err().as_stop_reason(),
        Some(StopReason::Cancelled)
    );
    assert_eq!(
        observer.snapshot().status,
        Status::Finished(Outcome::Cancelled)
    );

    let (result, trace) = how_far_example_app::observe(Unstoppable, |pulse| {
        convert(&[1, 2], pulse, Mode::Fallback, false)
    });
    assert_eq!(result.unwrap(), vec![254, 253]);
    for finding in trace.diagnose(&Default::default()) {
        println!("{finding}");
    }
}
