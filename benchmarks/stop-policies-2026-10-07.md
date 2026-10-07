# enough stop policies by check pattern — 2026-10-07

Source: how-far `26e86b1` with enough 0.4.5, almost-enough 0.4.5 and
enough-tokio 0.5.1 from crates.io. Command:
`python3 dev/how-far-checkpoint-cost/measure.py stop`. This replaces the
stop-policy tables in `how-far-2026-10-06.md`, which measured an earlier enough.

The harness runs the same `#[inline(never)]` defilter over a 256 KiB buffer,
checks each stop policy six ways, and compares with the loop without checks.
Counts are the slope between 200 and 1000 iterations, medians of five
interleaved runs. `TokioStop` wraps a tokio-util `CancellationToken`;
`PollMeter` is the opt-in poll-latency instrument; `WithTimeout` and
`DebouncedTimeout` wrap a `Stopper` with an hour-long deadline, so they read
the clock but never fire. Instruction counts are deterministic; cycles depend
on machine load.

## x86-64: AMD Ryzen 9 5900XT (`r5900xt`), rustc 1.99.0

enough stop policies x check patterns, 1024 bytes of work per check (256 per 256 KiB buffer). Without them: 398,117 instructions serially, 432,906 on four workers.

Extra instructions per buffer (and overhead):

| | `&dyn Stop`, `check()?` | generic `impl Stop` | `should_stop()` | `may_stop()` hoisted | every 16th chunk | 4 workers share it |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `Unstoppable` | 1,560 (+0.4%) | -240 (-0.1%) | 1,563 (+0.4%) | 35 (+0.0%) | 1,419 (+0.4%) | -783 (-0.2%) |
| `Stopper` | 2,585 (+0.6%) | 531 (+0.1%) | 2,333 (+0.6%) | 2,597 (+0.7%) | 1,486 (+0.4%) | 250 (+0.1%) |
| `SyncStopper` | 2,586 (+0.6%) | 531 (+0.1%) | 2,333 (+0.6%) | 2,598 (+0.7%) | 1,486 (+0.4%) | 231 (+0.1%) |
| `StopRef` (of a `StopSource`) | 2,585 (+0.6%) | 531 (+0.1%) | 2,333 (+0.6%) | 2,598 (+0.7%) | 1,486 (+0.4%) | 252 (+0.1%) |
| `ChildStopper`, depth 2 | 6,938 (+1.7%) | 5,915 (+1.5%) | 6,685 (+1.7%) | 6,950 (+1.7%) | 1,759 (+0.4%) | 4,604 (+1.1%) |
| `FnStop` (cold function) | 3,610 (+0.9%) | 1,548 (+0.4%) | 2,333 (+0.6%) | 3,622 (+0.9%) | 1,549 (+0.4%) | 1,287 (+0.3%) |
| `OrStop` of two `Stopper`s | 3,609 (+0.9%) | 1,563 (+0.4%) | 3,613 (+0.9%) | 3,621 (+0.9%) | 1,550 (+0.4%) | 1,280 (+0.3%) |
| `WithTimeout<Stopper>` | 28,698 (+7.2%) | 25,628 (+6.4%) | 28,445 (+7.1%) | 28,710 (+7.2%) | 3,118 (+0.8%) | 26,358 (+6.1%) |
| `DebouncedTimeout<Stopper>` | 4,770 (+1.2%) | 2,970 (+0.7%) | 4,997 (+1.3%) | 4,782 (+1.2%) | 1,622 (+0.4%) | 2,445 (+0.6%) |
| `StopToken` of a `Stopper` | 3,866 (+1.0%) | 798 (+0.2%) | 3,613 (+0.9%) | 3,878 (+1.0%) | 1,566 (+0.4%) | 1,527 (+0.4%) |
| `PollMeter<Stopper>` | 79,130 (+19.9%) | 76,571 (+19.2%) | 78,877 (+19.8%) | 79,142 (+19.9%) | 6,270 (+1.6%) | 77,679 (+17.9%) |
| `TokioStop` | 3,866 (+1.0%) | 1,562 (+0.4%) | 3,357 (+0.8%) | 3,878 (+1.0%) | 1,566 (+0.4%) | 1,621 (+0.4%) |

Extra cycles per buffer (and overhead):

| | `&dyn Stop`, `check()?` | generic `impl Stop` | `should_stop()` | `may_stop()` hoisted | every 16th chunk | 4 workers share it |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `Unstoppable` | -4,080 (-5.3%) | -1,293 (-1.7%) | -1,801 (-2.3%) | -148 (-0.2%) | -1,199 (-1.6%) | 3,683 (+1.8%) |
| `Stopper` | 3,413 (+4.4%) | -5,759 (-7.4%) | 2,439 (+3.2%) | -1,644 (-2.1%) | -791 (-1.0%) | -3,459 (-1.7%) |
| `SyncStopper` | 637 (+0.8%) | -789 (-1.0%) | -1,963 (-2.5%) | 5,194 (+6.7%) | -2,723 (-3.5%) | -4,236 (-2.1%) |
| `StopRef` (of a `StopSource`) | 230 (+0.3%) | -5,340 (-6.9%) | -216 (-0.3%) | 1,444 (+1.9%) | -2,444 (-3.2%) | -5,732 (-2.8%) |
| `ChildStopper`, depth 2 | -383 (-0.5%) | 1,228 (+1.6%) | -2,101 (-2.7%) | 132 (+0.2%) | -2,818 (-3.6%) | -3,046 (-1.5%) |
| `FnStop` (cold function) | 347 (+0.4%) | 36 (+0.0%) | 1,867 (+2.4%) | -2,639 (-3.4%) | -1,114 (-1.4%) | -5,988 (-3.0%) |
| `OrStop` of two `Stopper`s | 482 (+0.6%) | -1,646 (-2.1%) | 2,779 (+3.6%) | 1,265 (+1.6%) | -1,685 (-2.2%) | -7,812 (-3.9%) |
| `WithTimeout<Stopper>` | 16,403 (+21.2%) | 15,745 (+20.4%) | 14,176 (+18.3%) | 18,813 (+24.3%) | -2,182 (-2.8%) | 7,759 (+3.8%) |
| `DebouncedTimeout<Stopper>` | 3,994 (+5.2%) | -369 (-0.5%) | 1,013 (+1.3%) | 1,753 (+2.3%) | -4,213 (-5.4%) | 2,361 (+1.2%) |
| `StopToken` of a `Stopper` | -1,505 (-1.9%) | 2,525 (+3.3%) | 3,390 (+4.4%) | -1,345 (-1.7%) | 1,347 (+1.7%) | -5,399 (-2.7%) |
| `PollMeter<Stopper>` | 34,362 (+44.4%) | 34,740 (+44.9%) | 34,252 (+44.3%) | 35,074 (+45.3%) | -659 (-0.9%) | 37,525 (+18.6%) |
| `TokioStop` | -2,493 (-3.2%) | -4,077 (-5.3%) | 87 (+0.1%) | -313 (-0.4%) | 2,279 (+2.9%) | -4,353 (-2.2%) |
run-heavy: done rc=0 28s | peak-RSS 0.16GiB | min-avail 53187MiB |

## aarch64: Ampere Altra (Neoverse-N1, `zen-arm-xl`), rustc 1.97.1

A default build: `CARGO_ENCODED_RUSTFLAGS=-Ctarget-cpu=generic`, overriding
the host environment's `-C target-cpu=neoverse-n1`. `perf` ran through `sudo`,
because the host's `perf_event_paranoid` is 4. A shared 16-vCPU cloud host
under a niced background load: instruction counts are unaffected, cycles
somewhat.

enough stop policies x check patterns, 1024 bytes of work per check (256 per 256 KiB buffer). Without them: 408,095 instructions serially, 448,792 on four workers.

Extra instructions per buffer (and overhead):

| | `&dyn Stop`, `check()?` | generic `impl Stop` | `should_stop()` | `may_stop()` hoisted | every 16th chunk | 4 workers share it |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `Unstoppable` | 1,806 (+0.4%) | -503 (-0.1%) | 1,305 (+0.3%) | -232 (-0.1%) | 914 (+0.2%) | 1,570 (+0.3%) |
| `Stopper` | 2,830 (+0.7%) | 14 (+0.0%) | 2,073 (+0.5%) | 2,844 (+0.7%) | 980 (+0.2%) | 2,599 (+0.6%) |
| `SyncStopper` | 3,086 (+0.8%) | 14 (+0.0%) | 2,329 (+0.6%) | 3,100 (+0.8%) | 996 (+0.2%) | 2,854 (+0.6%) |
| `StopRef` (of a `StopSource`) | 2,829 (+0.7%) | 14 (+0.0%) | 2,073 (+0.5%) | 2,844 (+0.7%) | 979 (+0.2%) | 2,592 (+0.6%) |
| `ChildStopper`, depth 2 | 5,134 (+1.3%) | 4,116 (+1.0%) | 4,633 (+1.1%) | 5,147 (+1.3%) | 1,124 (+0.3%) | 4,904 (+1.1%) |
| `FnStop` (cold function) | 4,878 (+1.2%) | 1,803 (+0.4%) | 2,585 (+0.6%) | 4,892 (+1.2%) | 1,107 (+0.3%) | 4,638 (+1.0%) |
| `OrStop` of two `Stopper`s | 3,597 (+0.9%) | 786 (+0.2%) | 2,841 (+0.7%) | 3,611 (+0.9%) | 1,027 (+0.3%) | 3,355 (+0.7%) |
| `WithTimeout<Stopper>` | 30,485 (+7.5%) | 25,917 (+6.4%) | 29,220 (+7.2%) | 30,504 (+7.5%) | 2,708 (+0.7%) | 30,249 (+6.7%) |
| `DebouncedTimeout<Stopper>` | 7,819 (+1.9%) | 4,235 (+1.0%) | 7,303 (+1.8%) | 7,834 (+1.9%) | 1,399 (+0.3%) | 8,856 (+2.0%) |
| `StopToken` of a `Stopper` | 3,853 (+0.9%) | 276 (+0.1%) | 3,097 (+0.8%) | 3,870 (+0.9%) | 1,043 (+0.3%) | 3,615 (+0.8%) |
| `PollMeter<Stopper>` | 86,809 (+21.3%) | 82,716 (+20.3%) | 86,048 (+21.1%) | 86,825 (+21.3%) | 6,228 (+1.5%) | 89,571 (+20.0%) |
| `TokioStop` | 4,365 (+1.1%) | 1,552 (+0.4%) | 3,097 (+0.8%) | 4,380 (+1.1%) | 1,076 (+0.3%) | 4,127 (+0.9%) |

Extra cycles per buffer (and overhead):

| | `&dyn Stop`, `check()?` | generic `impl Stop` | `should_stop()` | `may_stop()` hoisted | every 16th chunk | 4 workers share it |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `Unstoppable` | 4,020 (+2.1%) | 3,074 (+1.6%) | 2,882 (+1.5%) | 3,775 (+1.9%) | 4,970 (+2.6%) | -12,113 (-2.6%) |
| `Stopper` | 5,103 (+2.6%) | 3,619 (+1.9%) | 2,353 (+1.2%) | 5,426 (+2.8%) | 4,735 (+2.4%) | -11,657 (-2.5%) |
| `SyncStopper` | 4,439 (+2.3%) | 1,640 (+0.8%) | 2,838 (+1.5%) | 2,257 (+1.2%) | 4,714 (+2.4%) | 518 (+0.1%) |
| `StopRef` (of a `StopSource`) | 5,192 (+2.7%) | 3,593 (+1.9%) | 2,311 (+1.2%) | 5,118 (+2.6%) | 4,764 (+2.5%) | -15,723 (-3.4%) |
| `ChildStopper`, depth 2 | 4,727 (+2.4%) | 5,038 (+2.6%) | 4,402 (+2.3%) | 4,636 (+2.4%) | 4,842 (+2.5%) | 6,962 (+1.5%) |
| `FnStop` (cold function) | 3,488 (+1.8%) | 4,481 (+2.3%) | 4,183 (+2.2%) | 2,574 (+1.3%) | 4,784 (+2.5%) | 14,302 (+3.1%) |
| `OrStop` of two `Stopper`s | 3,850 (+2.0%) | 4,056 (+2.1%) | 4,019 (+2.1%) | 4,762 (+2.5%) | 4,804 (+2.5%) | -135 (-0.0%) |
| `WithTimeout<Stopper>` | 27,040 (+13.9%) | 20,440 (+10.5%) | 23,279 (+12.0%) | 26,437 (+13.6%) | 6,282 (+3.2%) | 15,272 (+3.3%) |
| `DebouncedTimeout<Stopper>` | 5,014 (+2.6%) | 4,422 (+2.3%) | 5,485 (+2.8%) | 4,592 (+2.4%) | 4,985 (+2.6%) | 39,013 (+8.5%) |
| `StopToken` of a `Stopper` | 5,072 (+2.6%) | 1,947 (+1.0%) | 4,531 (+2.3%) | 4,896 (+2.5%) | 4,729 (+2.4%) | -232 (-0.1%) |
| `PollMeter<Stopper>` | 47,574 (+24.5%) | 47,394 (+24.4%) | 45,449 (+23.4%) | 47,914 (+24.7%) | 7,258 (+3.7%) | 152,738 (+33.2%) |
| `TokioStop` | 4,537 (+2.3%) | 4,773 (+2.5%) | 2,797 (+1.4%) | 4,720 (+2.4%) | 4,949 (+2.6%) | -4,170 (-0.9%) |

## Per check

Through `&dyn Stop` on x86-64: `Stopper`, `SyncStopper` and `StopRef` cost
10 instructions, `Unstoppable` 6 (one call), `StopToken` of a `Stopper` and
`TokioStop` 15, `FnStop` and `OrStop` 14, `DebouncedTimeout` 19, `ChildStopper`
at depth 2 27, `WithTimeout` 112 (it reads the clock every check) and
`PollMeter` 309. On Neoverse-N1 the same are 11, 7, 15 and 17, 19 and 14, 31,
20, 119 and 339, and `SyncStopper`'s Acquire load costs one instruction more
than `Stopper`. A generic loop removes most of the call: `Stopper` costs 2
instructions per check on x86-64 and none measurable on N1. Asking
`may_stop()` once before the loop removes `Unstoppable`'s call too, and
changes nothing for a stop that can fire. Checking every 16th chunk instead
of every chunk keeps every policy except `PollMeter` under 1% on both.

## Since 2026-10-06 (x86-64)

Instructions per check, from the two records' tables (256 checks per buffer).
The `TokioStop` and `PollMeter` changes landed before `7e3fd28` and match
enough's CHANGELOG; the other rows come from building the harness against
enough at each commit from `7e3fd28` to `ca2a13c` (the 0.4.5 release):

| | 2026-10-06 | 0.4.5 | Change in enough |
| --- | ---: | ---: | --- |
| `TokioStop`, `&dyn` | 47 | 15 | #28 (enough's CHANGELOG: 47 → 15) |
| `PollMeter`, `&dyn` | 693 | 309 | #32 (enough's CHANGELOG: 693 → 308) |
| `ChildStopper` depth 2, `&dyn` | 36 | 27 | 53 at `7e3fd28`, 27 from #38 (`5283eda`) |
| `DebouncedTimeout`, `&dyn` | 23.5 | 18.6 | #31 (`e152409`): 23.6 → 18.5 |
| `StopToken`, generic loop | 2.1 | 3.1 | #36 (`5d53d06`), see below |
| `BoxedStop` | 16.1 | — | deprecated in 0.4.5; it wraps `StopToken`, so the harness dropped it |

`StopToken`'s check is the same three instructions before and after #36 (load
the flag byte, test, branch). The extra instruction is a register move in this
loop: with #36's branches instead of a jump table, the loop keeps the buffer
pointer in the argument register and saves and restores it around the
`sub_defilter` call (13 instructions per iteration, was 12). Through `&dyn` the
cost is unchanged at 15. The 2026-10-06 N1 tables don't compare with these:
that run's build flags weren't recorded, and the host's environment builds for
`neoverse-n1` unless overridden.
