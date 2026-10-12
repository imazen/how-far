# Stage reservations and fork handles

Core source: `699c683baad933de6f7b66d6780fba10c40caa79`.
Harness: `4fda05e3d01b6d485d3b4765626283bec783da24`.
Command: `rustup run 1.88 just feature-cost`, through `run-heavy --mem 16G`.
Measured on r5900xt. No target-cpu=native flag was used.

`Stages::define` returns variable-bound reservations and `begin(key)` checks
plan identity and position. A nonempty definition uses one `Arc` identity
allocation, replacing the initial API's per-label copies. Keys keep that
identity alive across plan moves and after plan destruction; they own no
completion rights. The active stage still uses an exclusive borrow, without
allocated mutable coordination state. `Stage::fork` returns existing child
owners, starts them, and leaves scheduling and joining to the caller.

The existing seven-word plan bound and three-word active-handle assertion
pass. A reservation is checked at two machine words. These are type-layout
assertions, not heap or process-memory measurements. Allocation counts above
come from the implementation, not a runtime allocator benchmark.

The harness compiles 8 and 32 distinct operations with ordinary calls,
closure runners, reserved stages, and fork branches. The IR delta counts
how-far functions instantiated in the consumer, excluding its own operation
bodies and standard-library functions. A zero delta means these fixtures
reuse the same handoff instantiations; it does not mean zero call-site code,
allocation, or runtime cost. Every fixture uses the same result/error types.
The fork fixture measures ownership and result handoff without a scheduler.

Rustc timings are medians of five invocations with warm files/dependencies
and incremental compilation disabled. They include constant-size declaration
and handoff instantiations in each consumer. They do not measure whole cold
application builds, runtime cycles, or thread scheduling. The
[initial string-based API measurement](named-stages-2026-10-12.md) is a separate
record with the same host and toolchain, before reservations replaced labels.

All existing compile-size limits passed without threshold changes. Ordinary
tracker checkpoint functions remain branch-free in optimized IR. The fork
probe uses the same per-call budget as the other handoff probes.

```text
rustc 1.88.0 (6b00bc388 2025-06-23)
PulseTree check/may_stop: branch-free release IR with unconditional callbacks
how-far IR per Stages::run call site: 91 lines (budget 120)
how-far IR per reserved stage call site: 0 lines (budget 120)
how-far IR per fork branch call site: 0 lines (budget 120)
how-far-along IR, std: 24376 lines (budget 25000)
how-far-really IR, diagnostics: 34608 lines (budget 35000)
how-far own IR, default: 15772 lines
how-far-along own IR, smooth: 26416 lines (audit, not a default-budget gate)
how-far-really own IR, stage-suggestions: 37645 lines (audit, not a default-budget gate)
rustc elapsed seconds (warm files)             check   debug release
empty no_std crate                             0.014   0.017   0.017
how-far                                        0.087   0.158   0.223
how-far-along (std)                            0.099   0.187   0.292
how-far-really                                 0.144   0.248   0.492
plain function call, per call site             0.001   0.001   0.006
reserved stage, per call site                  0.001   0.002   0.013
fork branch, per call site                     0.001   0.001   0.009
Stages::run, per call site                     0.001   0.002   0.007
smooth                                         0.106   0.193   0.296
stage-suggestions                              0.152   0.269   0.586
run-heavy: done rc=0 39s | peak-RSS 0.17GiB | min-avail 52677MiB | peak-load 3.45
```
