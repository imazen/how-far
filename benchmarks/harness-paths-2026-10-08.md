# Harness path cleanup — 2026-10-08

Baseline: `a943bc145a02adade504f6a6021ed554a5c4751f`.
The final tree merges `dad9908868eda922b7ed8979816bb5e867b19697`, carrying
the unit-pace, stage-analysis and cancellation-fixture cleanups forward.
Eleven dependency and patch paths now resolve relative to the harness manifest.
`just stage-metadata` resolved the full graph: 69 packages and 69 nodes.
The [harness README](../dev/stage-suggestions/README.md) records the checkout layout.
No codec benchmarks were rerun for this path-only change.

Measured on r5900xt with Rust 1.88.0, its bin first in PATH, disk-backed TMPDIR,
and `python3 dev/bench-how-far-build.py --runs 3`. Diagnostics IR is 34,620
before and 34,690 after including the merged library cleanups. The harness
manifest itself is outside the workspace measured by this guard. Core and
tracker remain 88 and 20,484 lines; all existing budgets pass.
These are compile-cost measurements, not runtime speed measurements.

## Baseline

```text
rustc 1.88.0 (6b00bc388 2025-06-23)
how-far IR per Stages::run call site: 88 lines (budget 120)
how-far-along IR, std: 20484 lines (budget 22000)
how-far-really IR, diagnostics: 34620 lines (budget 35000)
rustc instructions (millions)                  check   debug release
empty no_std crate                              20.7    22.9    25.4
how-far                                        257.7   605.0  1168.8
how-far-along (std)                            397.1  1073.7  2727.1
how-far-really                                 682.3  1857.2  5772.6
plain function call, per call site               4.6     8.3    46.3
Stages::run, per call site                       6.1    13.3    65.5
run-heavy: done rc=0 61s | peak-RSS 0.17GiB | min-avail 36661MiB | peak-load 31.70
```

## Final

```text
rustc 1.88.0 (6b00bc388 2025-06-23)
how-far IR per Stages::run call site: 88 lines (budget 120)
how-far-along IR, std: 20484 lines (budget 22000)
how-far-really IR, diagnostics: 34690 lines (budget 35000)
rustc instructions (millions)                  check   debug release
empty no_std crate                              20.8    22.9    25.5
how-far                                        257.7   605.0  1168.5
how-far-along (std)                            397.2  1073.7  2727.1
how-far-really                                 682.1  1859.3  5764.1
plain function call, per call site               4.6     8.3    46.3
Stages::run, per call site                       6.1    13.3    65.6
run-heavy: done rc=0 44s | peak-RSS 0.17GiB | min-avail 35914MiB | peak-load 23.83
```

