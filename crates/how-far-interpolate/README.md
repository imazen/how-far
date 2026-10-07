# how-far-interpolate

Progress between reports, estimated on the polling side, for displays built on
[`how-far-along`](../how-far-along/README.md). An accessory crate: neither
`how-far` nor `how-far-along` depends on it. `no_std + alloc`, Rust 1.88.

A library reports at its own granularity, and should not be asked to
report more often only so that a display moves smoothly. A display polled
faster than the reports sees the fraction jump at each one and stand still
between them. `ProgressSmoother` smooths that without touching the library:
it remembers, for each running stage, when it first saw each new count and
how far apart the counts have come, and between reports moves the stage on
at that pace. That is extrapolation: it shows work the library has not yet
reported.

The rules:

- Between reports a running stage moves at its own average pace, at most
  the latest change past its recorded count and at most half the work it
  has left, so extrapolation alone never completes a stage. A shorter gap
  between changes sets the pace at once; a longer one, such as a stall,
  moves it a quarter of the way.
- Each stage keeps its own pace, so a fast stage followed by a slow one is
  not extrapolated at the fast pace. A stage that has not yet been seen to
  change has no pace and shows its count.
- While a stage runs under the same total, its display never goes back.
- Recorded outcomes win. A finished stage shows what the snapshot records,
  so a stage that fails or is cancelled steps the display back to the work
  it did, and a revised total starts the stage over from its count.
- Count changes seen at the same time are one change; an earlier time
  counts as the same time.

The estimate is only as good as the reporting. It assumes a stage's reports
are roughly evenly spaced, and it cannot find stage boundaries a library did
not declare: a flat phase whose units get slower runs up to one change ahead
before its pace adapts. Work whose cost per unit changes should be split into
stages. A change is timed when a poll first sees it, so the pace and its
bound are only as fine as the polling: polled once a second, a stage that
reports a thousand times a second moves in changes of about a thousand units.

Keep one `ProgressSmoother` per observed tree of one operation, and start a
new one for another operation or another subtree: node ids repeat across
operations. The tracker reads no clock, so the caller passes the time. The
result is for display; decisions and final status should use the snapshot.

```rust
use core::time::Duration;
use how_far_along::{Phase, Report, Total};
use how_far_interpolate::ProgressSmoother;

let job = Phase::new("decode", Total::Exact(10));
let reporter = job.reporter();
let mut smooth = ProgressSmoother::new();
let at = |ms| Duration::from_millis(ms);

smooth.display_fraction(&job.observer().snapshot(), at(0));
reporter.advance(1); // one row every 100 ms
smooth.display_fraction(&job.observer().snapshot(), at(100));
reporter.advance(1);
smooth.display_fraction(&job.observer().snapshot(), at(200));
// Half-way to the next row, the display is half-way there too.
let shown = smooth.display_fraction(&job.observer().snapshot(), at(250)).unwrap();
assert!((shown - 0.25).abs() < 1e-9);
// A late row holds the display at the next expected count.
let shown = smooth.display_fraction(&job.observer().snapshot(), at(900)).unwrap();
assert!((shown - 0.3).abs() < 1e-9);
```
