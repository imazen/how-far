# how-far-along [![CI](https://img.shields.io/github/actions/workflow/status/imazen/how-far/ci.yml?style=flat-square&label=CI)](https://github.com/imazen/how-far/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/how-far-along?style=flat-square)](https://crates.io/crates/how-far-along) [![lib.rs](https://img.shields.io/crates/v/how-far-along?style=flat-square&label=lib.rs&color=blue)](https://lib.rs/crates/how-far-along) [![docs.rs](https://img.shields.io/docsrs/how-far-along?style=flat-square)](https://docs.rs/how-far-along) [![license](https://img.shields.io/crates/l/how-far-along?style=flat-square)](https://github.com/imazen/how-far#license)

[Guide and interactive examples](https://imazen.github.io/how-far/) · [API reference](https://docs.rs/how-far-along)

Tracking and observation for `how-far`. This crate records declared work;
applications choose when to observe it. It starts no threads and reads no clocks.

```toml
[dependencies]
how-far-along = "0.1.0"
```

The library accepts `&dyn Pulse`; the application owns the tree and its final
result. Keep an observer for your display, then hand the result back before `?`:

```rust
use how_far_along::{prelude::*, Phase, PulseTree, StopReason, Total, Unstoppable};

fn checksum(bytes: &[u8], pulse: &dyn Pulse) -> Result<u64, StopReason> {
    pulse.check()?;
    let mut sum = 0;
    for &byte in bytes {
        sum += u64::from(byte);
        pulse.step(1)?;
    }
    Ok(sum)
}

let tree = PulseTree::new(Phase::new("checksum", Total::Exact(3)), Unstoppable);
let observer = tree.observer();
let result = checksum(&[1, 2, 3], &tree).finish_phase(tree)?;
assert_eq!(result, 6);
assert_eq!(observer.snapshot().fraction(), Some(1.0));
# Ok::<(), StopReason>(())
```

Replace `Unstoppable` with your application's stop token. The library's code
stays the same. [The integration guide](https://github.com/imazen/how-far/blob/main/docs/how-far-integration.md)
shows cancellation callbacks, nested stages and display smoothing.

Every phase starts `Status::NotStarted`, becomes `Running` when entered or
reported to, and eventually becomes `Finished(Outcome)`. Outcomes distinguish
`Succeeded`, `Failed`, `Cancelled`, `Skipped`, `NotRun`, and `Abandoned`.
A successful parent may retain a failed attempt: the library's result decides
whether it recovered. `completion_inferred` identifies outcomes resolved by the
parent or a plan helper, rather than explicitly reported by that child.

A result handoff freezes the parent's observation. Started but uncompleted
children become abandoned; untouched children are skipped on success and not
run on failure. Retained child owners can close later but cannot rewrite that
frozen parent observation. Join workers first. Drop supplies abandonment, never
an inferred successful result.

`Observer::summary()` reads fraction, unresolved share and status without
allocating. `snapshot()` copies the tree. `try_summary` / `try_snapshot` avoid
waiting on standard metadata locks; on no_std they enter the host's critical
section and do not promise wait-free access. Counting never locks metadata.
Weighted parents show known progress even while other stages have unknown
totals. Unknown children contribute zero but keep their assigned weight;
`unresolved_fraction` exposes their reserved share. A leaf with an unknown or
overflowed denominator still returns `None`. Fractions can regress after total
revisions. Success discharges a parent's obligation without rewriting failed
attempts.

The only default feature is `std`. Snapshot JSON, `WithStop`, `FnPulse` and lazy
`Checkpoint` are always available. With defaults disabled, tracking uses
`alloc`, pointer atomics, and the application's critical-section provider. That
provider must exclude every participating core. Native-width counters saturate
and expose overflow on targets without 64-bit atomics. Callbacks need neither
std nor 64-bit atomics. Callback state belongs only to callback-backed pulses;
ordinary `PulseTree` nodes have no callback field or dispatch branch.
The stable, opt-in `smooth` feature adds
`ProgressSmoother`, which smooths a display's fraction between reports at each
running stage's own pace; it needs neither std nor a clock of its own.

Callbacks run at checkpoints, never from counting, completion or Drop. Use
`poll::LocalPoller` for UI-thread or borrowed callbacks. Shared workers retain
full capabilities through `share()`; completion rights remain with the owner.

Only selected `how-far` author-facing types and its trait prelude are re-exported.
Implementor and checked-runner types are imported directly from `how_far`.
Profiling and tuning belong in `how_far_really::{profile, diagnostics}`; neither
that crate nor its types are re-exported here.

```compile_fail,E0432
use how_far_along::diagnostics::DiagnosticPulse;
```

Tracking JSON uses schema version 2 (`NotStarted` and inferred-completion evidence).
See [the example application](https://github.com/imazen/how-far/blob/main/examples/how-far-app/README.md) and
[design and API boundaries](https://github.com/imazen/how-far/blob/main/docs/how-far-design.md).

The [tested integration guide](https://github.com/imazen/how-far/blob/main/docs/how-far-integration.md) connects
library code, caller-owned completion, diagnostics and display smoothing.
