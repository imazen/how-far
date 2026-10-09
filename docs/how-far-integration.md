# Integrating a library and its caller

A library takes `&dyn Pulse` and returns its own result. The caller chooses
cancellation, observation, and diagnostics. The Rust snippets below are compiled
and run as doctests in `how-far-example-app`; `just check-integration` also runs
the cross-crate scenarios and the runnable `integration` example.

## Dependencies before publication

The how-far crates are not published yet. Pin them to the same revision:

```toml
[dependencies]
how-far = { git = "https://github.com/imazen/how-far", rev = "b90298891ed81ce0cafc70ae425cffb7b0670b82" }

[dev-dependencies]
how-far-along = { git = "https://github.com/imazen/how-far", rev = "b90298891ed81ce0cafc70ae425cffb7b0670b82", features = ["callback", "smooth"] }
how-far-really = { git = "https://github.com/imazen/how-far", rev = "b90298891ed81ce0cafc70ae425cffb7b0670b82" }
```

For a library, the tracker and diagnostic tools normally belong in
`dev-dependencies`. An application displaying progress needs `how-far-along`
in its normal dependencies. Put `how-far-really` in normal dependencies only
when the application intentionally collects diagnostics at runtime. Stage
grouping stays in that crate behind `stage-suggestions`; it is tuning advice
from observed runs, not a library dependency or an automatically applied plan.
If the library also uses `enough`, use the same Cargo source as how-far
(currently crates.io, version `0.4.5`). Two copies from different sources have
different `Stop` and `StopReason` identities.

## Library code: close what you own

Use `Stages::complete_with` around a sequence containing `?`. Each `run` owns
and completes its child; the enclosing helper completes unused children from
the body's result. It leaves the borrowed input pulse for its caller to finish.

```rust
use how_far::{prelude::*, PhaseSpec, Stages, StopReason, Total};

fn copy(input: &[u8], pulse: &dyn Pulse) -> Result<Vec<u8>, StopReason> {
    Stages::new(pulse, &[
        PhaseSpec::new("copy", 4, Total::Exact(input.len() as u64)).units("bytes"),
        PhaseSpec::new("finalize", 1, Total::Exact(1)),
    ]).complete_with(|stages| {
        let output = stages.run(|stage| {
            stage.check()?;
            let mut output = Vec::with_capacity(input.len());
            let mut paced = stage.paced(64);
            for &byte in input {
                output.push(byte);
                paced.step(1)?;
            }
            paced.finish()?;
            Ok::<_, StopReason>(output)
        })?;
        stages.run(|stage| stage.step(1))?;
        Ok(output)
    })
}
assert_eq!(copy(&[1, 2], &how_far::NoPulse), Ok(vec![1, 2]));
```

Keep the library's error type. Implement `From<StopReason>` to use `?`, and
`AsStopReason` to classify cancellation, delegating through nested errors.
For an error type you cannot implement that trait for, use the classified
helpers. `Stages` keeps rejected observations out of the work's result and
retains cancellation through untracked children. Use `TryStages` (feature
`checked`) only when planning errors are part of the intended public contract.

Pacing trades checkpoint frequency for overhead. `Paced::finish` checks the
last partial batch; Drop only reports it. Report work after doing it. Use
`Phases` for independently selected attempts, such as a fallback after a
failed decoder, rather than treating the failed attempt as a failed sequence.

## Caller code: preserve the result and finish the root

These examples call the repository's separate pipeline and codec crates.
Tracking must preserve the output and the error, including errors nested in
another library. Handoff the result before applying `?` or unwrapping it:

```rust
use how_far::{prelude::*, NoPulse, Outcome, Total, Unstoppable};
use how_far_along::{Phase, PulseTree, Status};
use how_far_example_codec::Mode;
use how_far_example_pipeline::convert;

let expected = convert(&[1, 2], &NoPulse, Mode::Fast, false);
let tree = PulseTree::new(Phase::new("convert", Total::Unknown), Unstoppable);
let observer = tree.observer();
let result = convert(&[1, 2], &tree, Mode::Fast, false);
assert_eq!(observer.snapshot().status, Status::Running);
let result = result.finish_phase(tree);
assert_eq!(result, expected);
assert_eq!(observer.snapshot().status, Status::Finished(Outcome::Succeeded));
```

With only a token, use `StopOnly::borrowed(&token)` for scoped work or
`StopOnly::new(token)` when workers need owned shared views. With no tracking
or cancellation, pass `&NoPulse`. Neither needs a tracker dependency.

A checkpoint callback is useful for precise integration tests without a custom
stop-policy type. Here cancellation reaches the nested codec unchanged:

```rust
use how_far::{prelude::*, Outcome, StopReason};
use how_far_along::{FnPulse, Status};
use how_far_example_codec::{Error as CodecError, Mode};
use how_far_example_pipeline::{convert, Error};

let pulse = FnPulse::new("convert", |_| Err(StopReason::Cancelled));
let observer = pulse.observer();
let result = convert(&[1, 2], &pulse, Mode::Fast, false).finish_phase(pulse);
assert_eq!(result, Err(Error::Codec {
    image: 0, error: CodecError::Stopped(StopReason::Cancelled),
}));
assert_eq!(observer.snapshot().status, Status::Finished(Outcome::Cancelled));
```

`FnPulse` calls back on checks, not on reports, completion, or Drop. Read the
observer once after the final handoff. Callbacks can overlap across workers;
use `LocalPoller` for thread-affine UI callbacks. Join owned workers before
finishing their owner; `share()` grants observation/reporting/checking rights,
not completion rights. A borrowed stop cannot become an owned shared view.

## Diagnostics and display are caller choices

Wrap the root with `DiagnosticPulse`, pass that wrapper to the same library,
and hand the result to the wrapper. Then record operation return and collect
the trace. This also preserves ordinary failures:

```rust
use how_far::{prelude::*, Outcome, Total, Unstoppable};
use how_far_along::{Phase, PulseTree, Status};
use how_far_really::{diagnostics::DiagnosticPulse, profile::{Profiler, StdClock}};
use how_far_example_codec::Mode;
use how_far_example_pipeline::convert;

let profiler = Profiler::new(StdClock::new(), 128);
let pulse = DiagnosticPulse::new(
    PulseTree::new(Phase::new("convert", Total::Unknown), Unstoppable), &profiler,
);
let observer = pulse.observer();
let result = convert(&[1, 2], &pulse, Mode::Fatal, false).finish_phase(pulse);
profiler.operation_returned();
assert!(result.is_err());
let trace = profiler.snapshot().with_progress(observer.snapshot());
assert_eq!(trace.progress.as_ref().unwrap().status, Status::Finished(Outcome::Failed));
```

For a moving display, keep one `ProgressSmoother` per operation's observed
tree and feed it snapshots plus elapsed time from the host clock. It changes
only the displayed fraction. Cancellation decisions, final outcomes and
completed counts come from the snapshot. Its estimates can step back after
failure or a total revision. See the runnable example for host polling around
a worker and the [testing guide](how-far-testing-and-tuning.md) for measuring
checkpoint cadence, uneven pace and candidate stage weights.

## Adoption review

Check output parity against the existing operation, cancellation within work
and at stage boundaries, ordinary failures, nesting inside another caller's
plan, strided input where supported, and all optional passes. The cross-crate
examples cover those ownership and error contracts with a synthetic codec;
they do not replace pixel/output parity tests in a real library.

The external [zenresize pilot](https://github.com/imazen/zenresize/pull/16)
exercises real u8 resizing and optional post-processing. Its original draft
used `Steps`, exposed planning failures, and completed the borrowed caller
phase. Those are migration points to review against the contract above;
other pixel formats and streaming APIs need their own adoption work.
