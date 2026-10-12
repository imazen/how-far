# how-far

A small `no_std + alloc` protocol for cooperative cancellation and approximate
progress. Libraries accept `&dyn Pulse` and return their own ordinary `Result`.
No tracker, clock, executor, serialization, or diagnostic collector is required.
Rust 1.88, like the other how-far crates; pointer atomics and an allocator are
required.

## One operation

Accept a pulse, check before starting, and report work after doing it:

```rust
use how_far::{prelude::*, StopReason};

fn checksum(bytes: &[u8], pulse: &dyn Pulse) -> Result<u64, StopReason> {
    pulse.check()?;
    let mut sum = 0;
    for &byte in bytes {
        sum += u64::from(byte);
        pulse.step(1)?;
    }
    Ok(sum)
}
assert_eq!(checksum(b"hello", &how_far::NoPulse), Ok(532));
```

Use the same function with `&how_far::NoPulse`, a tracker from `how-far-along`,
or `&StopOnly::borrowed(&token)` for cancellation alone. The library never
finishes the borrowed input pulse: its caller owns that result handoff.
See the [application quickstart](../how-far-along/README.md).

## Several operations

Define the stages once, bind their reservations to variables, and pass each
active handle to its operation. The operations can take different inputs and
return different types:

```rust
use how_far::{prelude::*, PhaseSpec, Stages, StopReason, Total};

fn prepare(bytes: &mut [u8], pulse: &dyn Pulse) -> Result<u64, StopReason> {
    let (mut stages, [invert, checksum]) = Stages::define(pulse, [
        PhaseSpec::new("invert", 1, Total::Exact(bytes.len() as u64)),
        PhaseSpec::new("checksum", 1, Total::Exact(bytes.len() as u64)),
    ]);
    let invert = stages.begin(invert);
    invert_bytes(bytes, &invert).finish_phase(invert)?;
    let checksum = stages.begin(checksum);
    let sum = sum_bytes(bytes, &checksum).finish_phase(checksum)?;
    stages.complete(Ok(sum))
}

fn invert_bytes(bytes: &mut [u8], pulse: &dyn Pulse) -> Result<(), StopReason> {
    pulse.check()?;
    for byte in bytes {
        *byte = !*byte;
        pulse.step(1)?;
    }
    Ok(())
}

fn sum_bytes(bytes: &[u8], pulse: &dyn Pulse) -> Result<u64, StopReason> {
    pulse.check()?;
    let mut sum = 0;
    for &byte in bytes {
        sum += u64::from(byte);
        pulse.step(1)?;
    }
    Ok(sum)
}
assert_eq!(prepare(&mut [0, 255], &how_far::NoPulse), Ok(255));
```

`Stages::define` returns the plan and an array of `StageKey` reservations.
Bind them to variables; `stages.begin(key)` gives the next active handle.
Pass it to an ordinary function and hand the result back with
`.finish_phase(stage)?`. Keys are specific to one plan and checked in order;
a mismatch records `StageMismatch` and runs untracked without consuming the
next declaration. Labels are only for display. A failed or cancelled handoff
makes remaining stages `NotRun`, even when `?` returns immediately. Call
`stages.complete(Ok(value))` on success to mark unused optional stages `Skipped`.

Within an active stage, `stage.fork([specs...])` starts independent branch
handles. Each takes its own data and uses the same result handoff. Schedule
and join them with your threading or async library, then hand the combined
result to the enclosing stage. Branch failure does not stop siblings by itself.
For runtime-sized repeated work, `Stages::new` plus `begin_next()` uses the
same handles without named reservations. See the
[fork–join example](../../docs/how-far-integration.md#forkjoin-with-different-operations).

`Complete::complete(result)` and `ResultExt::finish_phase(owner)` are explicit
result handoffs, returning the original value/error unchanged. Untouched phases
become `Skipped` on success or `NotRun` on failure. Drop without a result records
`Abandoned`. An unfinished active stage also marks the remaining sequence
`NotRun`. Put work that can fail inside the operation whose result is handed
over: a bare `?` before that handoff cannot classify its error for the tracker.
A borrowed pulse belongs to its caller: complete your child owners and plan,
never the borrowed input.

## Adapting existing code

`Stages::run` and `Complete::complete_with` remain optional closure conveniences.
The latter hands over a multi-step body's result, including early `?` returns.

For a custom error, implement `From<StopReason>` for `?` and `enough::AsStopReason`
(re-exported here) for cancellation classification; the latter only inspects
the error. Forgetting it is a compile error that names the fix. Bare
`StopReason` works directly. For a foreign error type you cannot implement
`AsStopReason` for, pass a classifier to `run_classified`, `complete_classified` or
`complete_with_classified`.

`check` invokes cancellation/checkpoint policy; `advance` only counts; `step`
counts then checks. One `Paced` per worker batches reports. Its `finish()` flushes
and checks the final partial batch. Its Drop only flushes; it cannot return a
cancellation error. Counters do not synchronize application data.

`Phases` runs independently selected phases, allowing a library to catch a failed
attempt and run a fallback. These plan helpers introduce no planning error into the
work's result. Plans rejected by an observer retain cancellation/checkpoint
behavior through untracked children. `record_issue` lets an optional diagnostic
wrapper retain evidence of rejected observations.

`share()` preserves the complete Pulse interface for owned tasks but grants no
completion rights. Join workers before completing their original owner.
Unsupported sharing is an explicit `NotShareable` error, never a silent loss of
cancellation. `Pulse::split`, `start`, `set_total`,
and `Child::finish` are the explicitly fallible administration layer.
`TryStages` and its `RunError` wrapper provide an explicitly fallible runner.
All core APIs are available without Cargo features.

`WithStop` preserves an additional stop policy through children and owned views.
`WithStop::borrowed(pulse, stop)`
combines a caller's pulse with a local `&dyn Stop`; `replacing_borrowed` uses only
the new policy (bypassing the inner check and its callbacks). Borrowed policies
work with scoped workers; owned `new`/`replacing` policies can use `share()`. Tracking and lazy callbacks live in
`how-far-along`; profiling and diagnostics live in `how-far-really`.

Runnable, cross-crate usage and intentional mistakes are in
[the example crates](../../examples/how-far-app/README.md).

See the [tested integration guide](../../docs/how-far-integration.md) for dependency setup, result
handoffs, callbacks, diagnostics, and caller-side display smoothing. Run it
with `just check-integration` from the workspace root.
