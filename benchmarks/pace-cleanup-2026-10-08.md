# Unit-pace traversal cleanup — 2026-10-08

The baseline is `2f2bb8736c1730cc677c3339fd0824c549e1bc54`. The change
keeps report sampling and late-report accounting semantics unchanged.

Measurements use Rust 1.88.0 on r5900xt, with its toolchain's `bin` first in
`PATH`, a disk-backed `TMPDIR`, and:

```sh
python3 dev/bench-how-far-build.py --runs 3
```

The baseline diagnostics crate emits 34,287 lines of unoptimized LLVM IR
against the 35,000-line budget. Core emits 88 lines per stage call site;
the default tracker emits 20,484 lines.

A mutable-slice traversal for late reports plus alternating `Vec::retain`
compaction emitted 34,740 diagnostics IR lines. Its resource monitor reported:

```text
run-heavy: done rc=0 97s | peak-RSS 0.17GiB | min-avail 36427MiB | peak-load 37.08
```

This experiment preserves sampling behavior but adds compiled machinery;
the final comparison tests counted in-place compaction instead.

The final implementation uses a mutable-slice `for` loop for late reports
and a counted `for` loop for in-place compaction. It emits 34,371 diagnostics
IR lines. Core and default tracker IR remain 88 and 20,484 respectively.
The counted compaction avoids 369 IR lines compared with `Vec::retain`;
it also expresses the fixed half-length result without a mutable loop bound.
The final implementation is 84 IR lines above the baseline.

These are compile-cost measurements, not runtime speed measurements.

## Baseline

```text
rustc 1.88.0 (6b00bc388 2025-06-23)
how-far IR per Stages::run call site: 88 lines (budget 120)
how-far-along IR, std: 20484 lines (budget 22000)
how-far-really IR, diagnostics: 34287 lines (budget 35000)
rustc instructions (millions)                  check   debug release
empty no_std crate                              20.7    22.9    25.5
how-far                                        257.7   605.1  1168.8
how-far-along (std)                            397.1  1074.0  2727.1
how-far-really                                 671.9  1836.5  5678.2
plain function call, per call site               4.6     8.3    46.3
Stages::run, per call site                       6.1    13.3    65.6
run-heavy: done rc=0 102s | peak-RSS 0.17GiB | min-avail 34076MiB | peak-load 51.79
```

## Mutable slice and retain experiment

```text
rustc 1.88.0 (6b00bc388 2025-06-23)
how-far IR per Stages::run call site: 88 lines (budget 120)
how-far-along IR, std: 20484 lines (budget 22000)
how-far-really IR, diagnostics: 34740 lines (budget 35000)
rustc instructions (millions)                  check   debug release
empty no_std crate                              20.7    22.9    25.4
how-far                                        257.7   605.1  1168.4
how-far-along (std)                            397.1  1073.8  2727.0
how-far-really                                 670.9  1847.7  5685.3
plain function call, per call site               4.6     8.3    46.3
Stages::run, per call site                       6.1    13.3    65.5
run-heavy: done rc=0 97s | peak-RSS 0.17GiB | min-avail 36427MiB | peak-load 37.08
```

## Final mutable slice and counted compaction

```text
rustc 1.88.0 (6b00bc388 2025-06-23)
how-far IR per Stages::run call site: 88 lines (budget 120)
how-far-along IR, std: 20484 lines (budget 22000)
how-far-really IR, diagnostics: 34371 lines (budget 35000)
rustc instructions (millions)                  check   debug release
empty no_std crate                              20.8    22.9    25.4
how-far                                        257.7   605.1  1168.5
how-far-along (std)                            397.1  1073.2  2727.3
how-far-really                                 671.8  1839.6  5673.6
plain function call, per call site               4.6     8.3    46.3
Stages::run, per call site                       6.1    13.3    65.5
run-heavy: done rc=0 80s | peak-RSS 0.17GiB | min-avail 36826MiB | peak-load 34.22
```
