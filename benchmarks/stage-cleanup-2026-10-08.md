# Stage-analysis cleanup — 2026-10-08

The baseline is `8eb1991e034fddd98ef6a141c3a6d3bda88d3d5a`.
The final tree also merges the unit-pace cleanup `333d297dacf28be0ba32788cc8aaaee6187d1da0`.
Stage grouping remains in `how-far-really` behind `stage-suggestions`, with
separate grouping and rendering and optional site indices. Intervals derive
from the maximum of the previous check and timed-report timestamps, avoiding
a redundant field. An interleaved, out-of-order timestamp regression verifies
that checks and reports neither double-count intervals nor rewind their start.
Existing stage-output expectations are unchanged.

Measured on r5900xt with Rust 1.88.0, its toolchain bin first in PATH,
a disk-backed TMPDIR, and `python3 dev/bench-how-far-build.py --runs 3`.
Diagnostics IR changes from 34,620 to 34,690 lines including the merged cleanup;
the limit remains 35,000. Core and tracker remain 88 and 20,484 lines.
The guard measures default diagnostics; the optional grouping feature is
covered by workspace tests and feature-powerset checks, not this IR budget.
These are compile-cost measurements, not runtime performance claims.

## Baseline

```text
rustc 1.88.0 (6b00bc388 2025-06-23)
how-far IR per Stages::run call site: 88 lines (budget 120)
how-far-along IR, std: 20484 lines (budget 22000)
how-far-really IR, diagnostics: 34620 lines (budget 35000)
rustc instructions (millions)                  check   debug release
empty no_std crate                              20.7    22.9    25.5
how-far                                        257.7   605.1  1168.5
how-far-along (std)                            397.1  1073.8  2727.3
how-far-really                                 682.2  1857.8  5771.5
plain function call, per call site               4.6     8.3    46.3
Stages::run, per call site                       6.1    13.3    65.5
run-heavy: done rc=0 68s | peak-RSS 0.17GiB | min-avail 35690MiB | peak-load 34.98
```

## Final

```text
rustc 1.88.0 (6b00bc388 2025-06-23)
how-far IR per Stages::run call site: 88 lines (budget 120)
how-far-along IR, std: 20484 lines (budget 22000)
how-far-really IR, diagnostics: 34690 lines (budget 35000)
rustc instructions (millions)                  check   debug release
empty no_std crate                              20.8    22.9    25.5
how-far                                        257.7   604.9  1168.6
how-far-along (std)                            397.1  1073.7  2727.2
how-far-really                                 682.1  1860.0  5764.0
plain function call, per call site               4.6     8.3    46.3
Stages::run, per call site                       6.1    13.3    65.5
run-heavy: done rc=0 51s | peak-RSS 0.17GiB | min-avail 34665MiB | peak-load 25.91
```

## Cancellation-fixture follow-up

[ARM CI](https://github.com/imazen/how-far/actions/runs/37716610631)
exposed a pre-existing host-test race: a polling callback could request a stop
after every worker's last check, while contended poll attempts could also be
dropped. The fixture now serializes dispatch and checks after it. All existing
assertions remain. `just check-hosts 100 check` passed (100 repetitions plus the
full local gates). The pre-fix local 100-repeat run did not reproduce the ARM
failure. Library source is unchanged from `8dc920c9031b53a114e39682de9c86a709f4bbec`;
the repeat guard confirms the same IR counts.

```text
rustc 1.88.0 (6b00bc388 2025-06-23)
how-far IR per Stages::run call site: 88 lines (budget 120)
how-far-along IR, std: 20484 lines (budget 22000)
how-far-really IR, diagnostics: 34690 lines (budget 35000)
rustc instructions (millions)                  check   debug release
empty no_std crate                              20.8    22.9    25.4
how-far                                        257.7   605.0  1168.8
how-far-along (std)                            397.2  1073.6  2726.9
how-far-really                                 682.2  1860.4  5764.3
plain function call, per call site               4.6     8.3    46.3
Stages::run, per call site                       6.1    13.3    65.6
run-heavy: done rc=0 60s | peak-RSS 0.17GiB | min-avail 36626MiB | peak-load 29.03
```
