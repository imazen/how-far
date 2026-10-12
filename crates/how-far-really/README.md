# how-far-really [![CI](https://img.shields.io/github/actions/workflow/status/imazen/how-far/ci.yml?style=flat-square&label=CI)](https://github.com/imazen/how-far/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/how-far-really?style=flat-square)](https://crates.io/crates/how-far-really) [![lib.rs](https://img.shields.io/crates/v/how-far-really?style=flat-square&label=lib.rs&color=blue)](https://lib.rs/crates/how-far-really) [![docs.rs](https://img.shields.io/docsrs/how-far-really?style=flat-square)](https://docs.rs/how-far-really) [![license](https://img.shields.io/crates/l/how-far-really?style=flat-square)](https://github.com/imazen/how-far#license)

[Guide and interactive examples](https://imazen.github.io/how-far/) · [API reference](https://docs.rs/how-far-really)

Opt-in diagnostics and checkpoint tuning for applications and library tests.
This crate depends on `how-far` and `how-far-along`; neither depends on it.
Rust 1.88 and `std` are required. No executor or background thread is started.

For library tests and benchmarks, put this crate in `[dev-dependencies]`.
A diagnostics executable uses it in `[dependencies]`; an application can also
opt into runtime profiling. Wrap the caller's pulse and pass it to the same
library function you already use:

For a library already using `how-far = "0.1.0"`, add:

```toml
[dev-dependencies]
how-far-along = "0.1.0"
how-far-really = "0.1.0"
```

```rust
use how_far::{prelude::*, StopReason, Total, Unstoppable};
use how_far_along::{Phase, PulseTree};
use how_far_really::{diagnostics::{DiagnosticPulse, Options}, profile::{Profiler, StdClock}};

let profiler = Profiler::new(StdClock::new(), 128);
let pulse = DiagnosticPulse::new(
    PulseTree::new(Phase::new("encode", Total::Exact(1)), Unstoppable), &profiler);
let observer = pulse.observer();
let result = pulse.step(1); // or your_library::encode(input, &pulse)
result.finish_phase(pulse)?;
profiler.operation_returned();
let trace = profiler.snapshot().with_progress(observer.snapshot());
for finding in trace.diagnose(&Options::default()) {
    eprintln!("{finding}");
}
# Ok::<(), StopReason>(())
```

`profile` collects bounded spans, source sites, checkpoint/report gaps, callback
costs and cancellation timing. `diagnostics` adds `DiagnosticPulse`, protocol
incidents and findings. Explicitly record the host's cancellation request and
operation return to measure end-to-end cancellation latency. Per-worker spans
are needed to identify workers whose silence a shared phase might conceal.

Observations cannot prove program correctness. A missing result handoff produces
abandonment; a swallowed observed stop produces conflicting evidence; reporting
fewer units than an exact successful total produces a count mismatch. The
library's `Result` is always returned unchanged. A failed child inside a successful
parent may be a recovered attempt. An untouched phase inferred as skipped may be
optional or forgotten required work: no runtime observer can distinguish those
without more information. Findings describe evidence, not replacement errors.

Incident retention is bounded separately from spans, using the same configured
capacity. `dropped_incidents` and `dropped_spans` expose missing evidence. Timing
instrumentation has overhead and is intended for tests, diagnosis and tuning.
Report timing can be disabled. Finding text, thresholds and suggested weights
may evolve; match non-exhaustive `Kind` values with a wildcard.

Cancellation adapters have an observation boundary. A `WithStop` outside a
`DiagnosticPulse` can stop after the inner diagnostic check succeeds; replacing
the policy bypasses that check entirely. For stop timing, instrument the added
source with a profiler span as well. A trace without an observed stop is not proof
that an uninstrumented policy never stopped.

The optional `experimental-stage-suggestions` feature proposes stage groups
and weights from a measured run. Compare representative inputs before adopting
those suggestions. The feature is excluded from the stable compatibility
contract: grouping rules and generated suggestions may change in patch
releases, so tools depending on them should pin an exact version or Git revision.
Shared types such as `Trace`, `Options`, `Finding` and `SiteStats` keep their
normal compatibility guarantees.

Only the `profile` and `diagnostics` modules are public. Import protocol types
from `how_far`, tracker types from `how_far_along`, and diagnostic types here.

```compile_fail,E0432
use how_far_really::Pulse;
```

```compile_fail,E0432
use how_far_really::PulseTree;
```

Trace JSON uses schema version 2 and embeds a complete versioned tracking JSON
document under `progress`. Node/span identifiers are local to the process/trace.
Times and unit totals are strings, to avoid JavaScript precision loss; call
counts and identifiers are JSON numbers.
See the [example crates](https://github.com/imazen/how-far/blob/main/examples/how-far-app/README.md) and
[design and API boundaries](https://github.com/imazen/how-far/blob/main/docs/how-far-design.md).

The [tested integration guide](https://github.com/imazen/how-far/blob/main/docs/how-far-integration.md) connects
library code, caller-owned completion, diagnostics and display smoothing.
