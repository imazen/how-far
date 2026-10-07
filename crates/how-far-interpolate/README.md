# how-far-interpolate

Progress between reports, estimated on the polling side, for displays built on
[`how-far-along`](../how-far-along/README.md). An accessory crate: neither
`how-far` nor `how-far-along` depends on it. `no_std + alloc`, Rust 1.88.

A library reports at its own granularity, and should not be asked to
report more often only so that a display moves smoothly. A display polled
faster than the reports sees the fraction jump at each one and stand still
between them. `Interpolator` smooths that without touching the library:
it remembers, for each running leaf, when it first saw each new count and
how far apart the counts have come, and between reports advances the leaf
at that pace.

It assumes that a phase's reports are roughly evenly spaced, as they are
for a loop over rows, tiles or chunks. Each phase keeps its own pace, so
a fast stage followed by a slow one is not extrapolated at the fast pace.
A shorter gap between changes sets the pace at once; a longer one, such
as a stall, moves it a quarter of the way. An estimate never passes the
next expected change, the phase's total, or the snapshot's own fraction
of 1, and never goes backwards. A leaf that has not yet been seen to
change has no pace and shows its count.

A change is timed when a poll first sees it, so the pace, and the cap of
one change ahead, are only as fine as the polling: polled once a second,
a phase that reports a thousand times a second can run a thousand units
ahead of its count.

The tracker reads no clock, so the caller passes the time. The result is
for display; decisions should use the snapshot's own fraction.

```rust
use core::time::Duration;
use how_far_along::{Phase, Report, Total};
use how_far_interpolate::Interpolator;

let job = Phase::new("decode", Total::Exact(10));
let reporter = job.reporter();
let mut smooth = Interpolator::new();
let at = |ms| Duration::from_millis(ms);

smooth.fraction(&job.observer().snapshot(), at(0));
reporter.advance(1); // one row every 100 ms
smooth.fraction(&job.observer().snapshot(), at(100));
reporter.advance(1);
smooth.fraction(&job.observer().snapshot(), at(200));
// Half-way to the next row, the display is half-way there too.
let shown = smooth.fraction(&job.observer().snapshot(), at(250)).unwrap();
assert!((shown - 0.25).abs() < 1e-9);
// A late row holds the display at the next expected count.
let shown = smooth.fraction(&job.observer().snapshot(), at(900)).unwrap();
assert!((shown - 0.3).abs() < 1e-9);
```
