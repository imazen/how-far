# Smoother cleanup — 2026-10-08

Baseline: `2a4adb1265a2cc7ee72cc3b44cf6bc507730da8f`.
The smoother no longer stores the three-field undo buffer. Count changes with
no positive time interval update the count and latest-change bound while
preserving the learned pace. This intentionally replaces same-timestamp batch
recombination; its former equivalence test now verifies the requested contract.
A separate regression checks the smaller same-time batch bound and confirms
that zero-time reports do not invent a pace. Existing per-stage, terminal,
revised-total, high-water hold, half-remaining and below-one tests are unchanged.
`just check-smooth` passes the full local gates and no-default smoothing tests.

Measured on r5900xt with Rust 1.88.0, its bin first in PATH, disk-backed TMPDIR,
and `python3 dev/bench-how-far-build.py --runs 3`. Diagnostics IR remains 32,808
lines; core and default tracker remain 88 and 20,484. All guard budgets pass.
The guard does not enable `smooth`; these counts check the default build,
not the optional smoother's size. No runtime or allocation speedup is claimed.

## Baseline

```text
rustc 1.88.0 (6b00bc388 2025-06-23)
how-far IR per Stages::run call site: 88 lines (budget 120)
how-far-along IR, std: 20484 lines (budget 22000)
how-far-really IR, diagnostics: 32808 lines (budget 35000)
rustc instructions (millions)                  check   debug release
empty no_std crate                              20.7    22.9    25.5
how-far                                        257.7   605.2  1168.6
how-far-along (std)                            397.2  1073.9  2727.1
how-far-really                                 633.7  1752.5  5358.0
plain function call, per call site               4.6     8.3    46.3
Stages::run, per call site                       6.1    13.3    65.6
run-heavy: done rc=0 42s | peak-RSS 0.16GiB | min-avail 35602MiB | peak-load 23.84
```

## Final

```text
rustc 1.88.0 (6b00bc388 2025-06-23)
how-far IR per Stages::run call site: 88 lines (budget 120)
how-far-along IR, std: 20484 lines (budget 22000)
how-far-really IR, diagnostics: 32808 lines (budget 35000)
rustc instructions (millions)                  check   debug release
empty no_std crate                              20.7    22.9    25.4
how-far                                        257.7   605.1  1168.5
how-far-along (std)                            397.2  1073.7  2727.3
how-far-really                                 633.7  1752.9  5357.7
plain function call, per call site               4.6     8.3    46.3
Stages::run, per call site                       6.0    13.3    65.6
run-heavy: done rc=0 17s | peak-RSS 0.16GiB | min-avail 41625MiB | peak-load 15.89
```

