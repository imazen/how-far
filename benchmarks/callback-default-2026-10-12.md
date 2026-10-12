# Callbacks without a Cargo feature

Removing the `callback` feature makes `FnPulse` and `Checkpoint` available in
both std and no_std configurations. Their private dispatcher policy remains
separate from ordinary `PulseTree` cancellation. No runtime implementation or
public signature changes in `4c69b481`, a follow-up to `554bb95f`.

The earlier [compile-time comparison](callback-isolation-2026-10-12.md)
measured callback support at an additional 6 ms for check, 9 ms for debug and
13 ms for release on Rust 1.88. Those are median individual rustc invocations
with warm files, not whole cold application builds. The user chose to include
that cost unconditionally.

Validation of this change:

- `just check`: workspace tests, no-default-feature tests, clippy, and feature
  powerset checks pass. Callback tests now run with defaults disabled too.
- `just api-doc` and `just api-doc-check`: callback types move into the default
  API snapshot; their signatures are unchanged.
- Cortex-M and wasm32 builds pass with `--no-default-features --features smooth`.
- `just checkpoint-asm` on Rust 1.88: ordinary `PulseTree::check` is identical
  to the ten instructions recorded in the [previous assembly inspection](callback-asm-2026-10-12.md).
- `rustup run 1.88 just build-cost`: ordinary `check` and `may_stop` retain
  branch-free optimized IR; `Stages::run` remains 88 lines per call site.
  Default tracker IR is 24,376 lines. The user approved changing the default
  cap from 22,000 to 25,000; the final feature audit passes.
  The guard now checks the unconditional configuration, since there is no
  callback-disabled configuration to compare against.

Final `rustup run 1.88 just feature-cost` results (five-sample medians,
warm-file rustc seconds):

| Configuration | Check | Debug | Release | Own IR lines |
| --- | ---: | ---: | ---: | ---: |
| Tracker default, including callbacks | 0.100 | 0.181 | 0.289 | 24,376 |
| Tracker plus smoothing | 0.104 | 0.193 | 0.292 | 26,416 |
| Diagnostics default | 0.142 | 0.241 | 0.493 | 34,602 |
| Diagnostics plus experimental suggestions | 0.150 | 0.262 | 0.579 | 37,639 |

Resource output from that final audit:

```text
run-heavy: done rc=0 30s | peak-RSS 0.17GiB | min-avail 53237MiB | peak-load 0.98
```
