# Callback isolation — 2026-10-12

Library source: `ea85c915d613ed768175b2cdfb3525ef39433117`.
The guard additionally supports both Rust legacy and v0 symbol mangling.
Measured on r5900xt, Rust 1.88.0 first in PATH, disk-backed TMPDIR, with
`run-heavy --mem 16G -- just feature-cost`. Compiler times are medians of five
rustc invocations with warm dependency files and no incremental compilation;
they are not whole-application build times. No default IR budget was raised.

## Implementation and evidence

`PulseTree` holds the private policy `Arc<dyn Stop>`. `FnPulse` holds the
private dispatcher policy. The shared ownership implementation is generic over
that policy internally; no public type becomes generic and no public signatures
change. Children clone the owner's policy and shared views retain the same
state. Ordinary state has no optional callback field in either feature build.

The build-cost guard emits release IR in isolated builds with and without
`callback`. It requires ordinary `PulseTree::check` and `may_stop` bodies to
match after removing metadata identifiers and requires both to have no
conditional branches. This guard runs in the existing CI build-cost job.
It verifies code generation, not an end-to-end runtime speedup.

```text
run-heavy: mem-cap=16G (cgroup, hard) jobs=28/32 avail=52890MiB :: just feature-cost
python3 dev/bench-how-far-build.py --runs 5 --feature-audit --wall-time
rustc 1.88.0 (6b00bc388 2025-06-23)
PulseTree check/may_stop: identical branch-free release IR with callback off/on
how-far IR per Stages::run call site: 88 lines (budget 120)
how-far-along IR, std: 20397 lines (budget 22000)
how-far-really IR, diagnostics: 34602 lines (budget 35000)
how-far own IR, default: 13276 lines
how-far-along own IR, callback: 24376 lines (audit, not a default-budget gate)
how-far-along own IR, smooth: 22437 lines (audit, not a default-budget gate)
how-far-really own IR, stage-suggestions: 37639 lines (audit, not a default-budget gate)
rustc elapsed seconds (warm files)             check   debug release
empty no_std crate                             0.014   0.017   0.017
how-far                                        0.081   0.147   0.215
how-far-along (std)                            0.096   0.176   0.281
how-far-really                                 0.141   0.241   0.504
plain function call, per call site             0.001   0.001   0.006
Stages::run, per call site                     0.001   0.002   0.007
callback                                       0.102   0.185   0.294
smooth                                         0.102   0.179   0.287
stage-suggestions                              0.154   0.266   0.587
run-heavy: done rc=0 35s | peak-RSS 0.17GiB | min-avail 52629MiB | peak-load 0.57
```

Callback compilation stays opt-in: its 24,376 IR lines exceed the tracker
22,000-line default budget. It no longer changes the ordinary tree's callback
state or checkpoint branching. This supersedes the callback rationale in the
[earlier feature audit](features-2026-10-12.md).

## Runtime benchmark limitation

Two baseline attempts at the existing overhead benchmark reported only four
rounds for `check_isolated` after more than two minutes. Both were interrupted;
no reliable before/after runtime comparison is claimed. The first invocation's
`--help` was not a help mode for this harness. The second attempt used the
existing baseline binary directly; `ZENBENCH_NO_BUSY_GATE=1` is not recognized
by the installed zenbench 0.1.9, so it did not disable gating.

Full logs: [first attempt](callback-baseline-attempt-1-2026-10-12.log),
[second attempt](callback-baseline-attempt-2-2026-10-12.log). The baseline binary
is preserved at `/home/lilith/tmp/howfar-callback-original/overhead`, SHA-256
`2cc8ad2474872b8400fa7e0e988c71a08c2352302e623e4f51e68af853efc9a8`.
Its disassembly shows a callback test and conditional jump in ordinary
`PulseTree::may_stop` and dispatch logic in `check`; the new IR guard confirms
neither is conditional on callback availability anymore.

## Behavior checks

Workspace all-feature tests, no-default core/tracker tests, Clippy and the
feature powerset pass. The public API snapshot check is unchanged and passes.
Callback-enabled no_std builds pass for `thumbv7em-none-eabihf` and
`wasm32-unknown-unknown`. The added regression exercises root, child, grandchild,
and shared-view checkpoints, checks both phase identity and root observation,
and verifies finishing suppresses subsequent callbacks. Existing stop latching,
reentrant observation, abandonment and checkpoint-only dispatch tests remain
unchanged.
