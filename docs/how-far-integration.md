# Integrating a library and its caller

A library takes `&dyn Pulse` and returns its own result. The caller chooses
cancellation, observation, and diagnostics. The Rust snippets below are compiled
and run as doctests in `how-far-example-app`; `just check-integration` also runs
the cross-crate scenarios and the runnable `integration` example.

## Dependencies before publication

The how-far crates are not published yet. Pin them to the same revision:

```toml
[dependencies]
how-far = { git = "https://github.com/imazen/how-far", rev = "699c683baad933de6f7b66d6780fba10c40caa79" }

[dev-dependencies]
how-far-along = { git = "https://github.com/imazen/how-far", rev = "699c683baad933de6f7b66d6780fba10c40caa79", features = ["smooth"] }
how-far-really = { git = "https://github.com/imazen/how-far", rev = "699c683baad933de6f7b66d6780fba10c40caa79" }
```

For a library, the tracker and diagnostic tools normally belong in
`dev-dependencies`. An application displaying progress needs `how-far-along`
in its normal dependencies. Put `how-far-really` in normal dependencies only
when the application intentionally collects diagnostics at runtime. Stage
grouping stays in that crate behind `experimental-stage-suggestions`; it is tuning advice
from observed runs, not a library dependency or an automatically applied plan.
If the library also uses `enough`, use the same Cargo source as how-far
(currently crates.io, version `0.4.5`). Two copies from different sources have
different `Stop` and `StopReason` identities.

## Library code: close what you own

Bind stage reservations to variables and call ordinary functions with their handles. Hand each
result to `.finish_phase(stage)` before `?`: a failure immediately marks the
remaining sequence `NotRun`. On success, `stages.complete(Ok(value))` resolves
unused optional stages. The borrowed input pulse remains its caller's to finish.

```rust
use how_far::{prelude::*, PhaseSpec, Stages, StopReason, Total};

fn copy(input: &[u8], pulse: &dyn Pulse) -> Result<Vec<u8>, StopReason> {
    let (mut stages, [copy_key, finalize_key]) = Stages::define(pulse, [
        PhaseSpec::new("copy", 4, Total::Exact(input.len() as u64)).units("bytes"),
        PhaseSpec::new("finalize", 1, Total::Exact(1)),
    ]);
    let copy_phase = stages.begin(copy_key);
    let output = copy_bytes(input, &copy_phase).finish_phase(copy_phase)?;
    let finalize_phase = stages.begin(finalize_key);
    finalize_phase.step(1).finish_phase(finalize_phase)?;
    stages.complete(Ok(output))
}

fn copy_bytes(input: &[u8], pulse: &dyn Pulse) -> Result<Vec<u8>, StopReason> {
    pulse.check()?;
    let mut output = Vec::with_capacity(input.len());
    let mut paced = pulse.paced(64);
    for &byte in input {
        output.push(byte);
        paced.step(1)?;
    }
    paced.finish()?;
    Ok(output)
}
assert_eq!(copy(&[1, 2], &how_far::NoPulse), Ok(vec![1, 2]));
```

Different stages may borrow different inputs and return different types. Write
those calls in order; iterate only when applying an operation to repeated data.
`Stages::define` returns one reservation per definition. `begin(key)` checks
that it belongs to this plan and is next in declaration order. It does not
search labels or schedule work. Use `skip()` to discharge an optional stage
before beginning a later one. A wrong-plan, reused, or out-of-order key records
`StageMismatch`, preserves cancellation, and leaves the next declaration pending.
Repeated labels are fine; keys distinguish their positions. Dropping a key
leaves its stage pending for the plan to resolve.

For a runtime-sized list of repeated work, construct `Stages::new(pulse, &specs)`
and call `begin_next()` inside the loop. The operation and result handoff are
identical; the loop supplies the ordering instead of named variables.

The same handle can span an async call: use
`operation(&stage).await.finish_phase(stage)?`. Dropping a suspended operation
abandons its active stage; Drop cannot infer a cancellation result.

An active `Stage` borrows the plan until completion. Its `share()` supports
owned workers, but they must join before the handoff. A dropped active stage
records `Abandoned` and leaves the remaining sequence `NotRun`. Neither Drop
nor a bare `?` can classify an error that never reaches a result handoff.
For substantial inline fallible work, use a normal helper function so its
result can be handed over. `run` and `complete_with` remain optional closure
helpers, including when a closure is more convenient for a short local body.

Keep the library's error type. Implement `From<StopReason>` to use `?`, and
`AsStopReason` to classify cancellation, delegating through nested errors.
For an error type you cannot implement that trait for, use the classified
helpers. `Stages` keeps rejected observations out of the work's result and
retains cancellation through untracked children. Use `TryStages` only when planning errors are part of the intended public contract.

Pacing trades checkpoint frequency for overhead. `Paced::finish` checks the
last partial batch; Drop only reports it. Report work after doing it. Use
`Phases` for independently selected attempts, such as a fallback after a
failed decoder, rather than treating the failed attempt as a failed sequence.

## Fork–join with different operations

An active stage can fork into branches with different inputs, operations and
result types. `fork` starts the branch handles but schedules nothing. The
closures below belong to scoped threading; the operation calls and result
handoffs are the same as sequential code.

```rust
use how_far::{prelude::*, PhaseSpec, Stages, StopReason, Total};
use how_far_along::{Phase, PulseTree, Status, Unstoppable};

fn analyze_pixels(pixels: &[u8], pulse: &dyn Pulse) -> Result<u64, StopReason> {
    pulse.check()?;
    let sum = pixels.iter().map(|&p| u64::from(p)).sum();
    pulse.step(pixels.len() as u64)?;
    Ok(sum)
}

fn read_metadata(header: &str, pulse: &dyn Pulse) -> Result<String, StopReason> {
    pulse.check()?;
    let metadata = header.trim().to_owned();
    pulse.step(1)?;
    Ok(metadata)
}

fn analyze(pixels: &[u8], header: &str, pulse: &dyn Pulse)
    -> Result<(u64, String), StopReason>
{
    let (mut stages, [analysis]) = Stages::define(pulse, [
        PhaseSpec::new("analysis", 1, Total::Unknown),
    ]);
    let analysis = stages.begin(analysis);
    let [pixels_stage, metadata_stage] = analysis.fork([
        PhaseSpec::new("pixels", 3, Total::Exact(pixels.len() as u64)),
        PhaseSpec::new("metadata", 1, Total::Exact(1)),
    ]);
    let (pixels_result, metadata_result) = std::thread::scope(|scope| {
        let pixels_task = scope.spawn(move ||
            analyze_pixels(pixels, &pixels_stage).finish_phase(pixels_stage));
        let metadata_task = scope.spawn(move ||
            read_metadata(header, &metadata_stage).finish_phase(metadata_stage));
        (pixels_task.join().unwrap(), metadata_task.join().unwrap())
    });
    // Both branches have joined and received their results before any `?`.
    let result = match (pixels_result, metadata_result) {
        (Ok(sum), Ok(metadata)) => Ok((sum, metadata)),
        (Err(error), _) | (_, Err(error)) => Err(error),
    };
    let output = result.finish_phase(analysis)?;
    stages.complete(Ok(output))
}

let tree = PulseTree::new(Phase::new("job", Total::Unknown), Unstoppable);
let observer = tree.observer();
let result = analyze(&[2, 3], " profile ", &tree).finish_phase(tree);
assert_eq!(result, Ok((5, String::from("profile"))));
assert_eq!(observer.snapshot().status, Status::Finished(how_far::Outcome::Succeeded));
```

Every forked handle is active. Dropping one without handing over its result
records `Abandoned`; an unused sequential reservation instead stays pending.
A failed branch does not automatically cancel siblings. Join every branch,
combine their results using the application's error policy, then finish the
enclosing stage. Its failure resolves later sequential stages as `NotRun`.
For different error types, map each into the library's error type before the
combined handoff.

Branches can declare their own `Stages::define` sequences using `&branch` as
their pulse. Rayon and borrowed async futures use the same handles; an async
join must await all branches before the enclosing handoff. Dropping a
short-circuiting join abandons any unfinished branch. Detached tasks instead
need `share()` and explicit joining before their original owners complete.
The array helper covers fixed branch sets; runtime-sized batches use the
lower-level `Pulse::split` or best-effort `(&dyn Pulse)::plan` interface.

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
exercises real u8 resizing and optional post-processing. The updated draft uses `Stages`, returns ordinary `StopReason`, and leaves the
borrowed phase to its caller. Its tests cover exact cancellation, rejected
observations and patterned padded input through all four post-pass combinations.
Other pixel formats and streaming APIs need their own adoption work.

The encoder adoption is a [zenpng fixture](../dev/adopt-zenpng.py), not an open
encoder PR. It materializes pinned zenpng commit
`27393eef995aea329cfbc1df5cac5e3f7346b4cb` and adds optional progress at existing
compression phase boundaries. From the how-far root, use a new destination:

```sh
just prepare-encoder ../zen/zenpng "$HOME/tmp/howfar-encoder"
just check-encoder "$HOME/tmp/howfar-encoder"
```

The generator reads the source checkout's lockfile; the probe resolves its own
lockfile in the destination. Preserve those files when recording measurements.
The [probe](../dev/encoder-probe.rs) checks encoded-byte parity with the existing
API for serial and parallel compression, caller-owned completion, cancellation
after actual encoder work, preservation of the old cancellation API, and a
padded RGB8 image inside a caller's plan. Foreign `whereat::At<PngError>` errors
use `complete_classified` with an explicit classifier instead of a new wrapper
error. The optional progress dependency also compiles out.

This fixture covers RGB8 with the Fast compression preset; it does not establish
adoption for all pixel formats, presets, animation or streaming. The pinned
encoder emits existing magetypes deprecation warnings with the resolved
0.9.30 dependency. The older [zenav1 measurement](how-far-zenav1-measurement.md)
is a separate dated experiment, not a current integration PR.

The main ergonomic cost left for ordinary libraries is explicit result handoff
and error classification. The prelude supplies the extension traits; the direct
pattern records each operation's result before `?`. Tests should still catch
forgotten completion: neither `?` nor Drop can infer a returned result.
