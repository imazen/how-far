# Named stage handles

Core source: `0e901969c7ee856dcc1c1c90500fca72b6a1458b`, following `820c47b6`.
Command: `rustup run 1.88 just feature-cost`, through `run-heavy --mem 16G`.
Measured on r5900xt. The harness is `dev/bench-how-far-build.py`.

`Stages::begin(name)` returns a borrowed owner for direct operation calls and
result handoffs. The existing seven-word plan size bound passes; the new
`Stage` is checked at three machine words. These are type-layout assertions,
not process-memory measurements. Names are copied into owned strings at plan
construction, one allocation per nonempty label, so temporary specifications
are safe. The handle itself needs no allocated coordination state. Dropping
an active handle abandons that stage and marks remaining work `NotRun`.

The compile-cost harness now probes both the retained closure API and direct
calls with 8 and 32 distinct operations. The reported IR delta counts how-far
functions instantiated in the caller. The direct API adds zero further such
lines per additional call in this fixture: it reuses the same handoff for the
same result/error types. This is **not** zero call-site code or zero runtime
cost. Different result/error types may instantiate additional handoffs.
Runtime cycles and whole cold application builds were not measured.

Rustc timings are medians of five invocations with warm files/dependencies and
incremental compilation disabled. No target-cpu=native flag was used.
All existing size budgets pass without changing their limits.

```text
rustc 1.88.0 (6b00bc388 2025-06-23)
PulseTree check/may_stop: branch-free release IR with unconditional callbacks
how-far IR per Stages::run call site: 93 lines (budget 120)
how-far IR per named stage call site: 0 lines (budget 120)
how-far-along IR, std: 24148 lines (budget 25000)
how-far-really IR, diagnostics: 34479 lines (budget 35000)
how-far own IR, default: 16073 lines
how-far-along own IR, smooth: 26188 lines (audit, not a default-budget gate)
how-far-really own IR, stage-suggestions: 37516 lines (audit, not a default-budget gate)
rustc elapsed seconds (warm files)             check   debug release
empty no_std crate                             0.014   0.016   0.017
how-far                                        0.083   0.154   0.228
how-far-along (std)                            0.101   0.178   0.289
how-far-really                                 0.144   0.244   0.498
plain function call, per call site             0.001   0.001   0.006
named stage, per call site                     0.001   0.001   0.009
Stages::run, per call site                     0.001   0.002   0.007
smooth                                         0.107   0.192   0.295
stage-suggestions                              0.153   0.270   0.583
run-heavy: done rc=0 34s | peak-RSS 0.17GiB | min-avail 53140MiB | peak-load 0.93
```
