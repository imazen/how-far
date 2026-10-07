# how-far [![CI](https://img.shields.io/github/actions/workflow/status/imazen/how-far/ci.yml?style=flat-square&label=CI)](https://github.com/imazen/how-far/actions/workflows/ci.yml) [![MSRV](https://img.shields.io/badge/MSRV-1.88-blue?style=flat-square)](https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field) [![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue?style=flat-square)](#license)

Cooperative cancellation and progress for library code. A library accepts
`&dyn Pulse`, checks it for cancellation, reports finished units of work and
weighted nested phases, and returns its own ordinary `Result`. The caller
decides whether anything tracks, displays or profiles that progress.

Cancellation comes from [`enough`](https://github.com/imazen/enough): a
`Pulse` is an `enough::Stop`, so the same value stops the work and reports on
it.

```rust
use how_far::{prelude::*, PhaseSpec, Stages, StopReason, Total};

fn encode(rows: &[u8], pulse: &dyn Pulse) -> Result<(), StopReason> {
    let mut stages = Stages::new(pulse, &[
        PhaseSpec::new("encode", 1, Total::Exact(rows.len() as u64)).units("rows"),
        PhaseSpec::new("optional metadata", 1, Total::Exact(1)),
    ]);
    let result = stages.run(|stage| {
        stage.check()?;
        let mut pace = stage.paced(64);
        for _row in rows {
            // Complete the row before reporting it.
            pace.step(1)?;
        }
        pace.finish()
    });
    result.finish_phase(stages)
}
```

| Crate | For | |
| --- | --- | --- |
| [`how-far`](crates/how-far/README.md) | libraries | The `&dyn Pulse` interface: cancellation, completed units, weighted phases. `no_std + alloc`, depends only on `enough`. |
| [`how-far-along`](crates/how-far-along/README.md) | applications | Progress trees, snapshots, pollers, JSON, optional checkpoint callbacks. `no_std + alloc`. |
| [`how-far-really`](crates/how-far-really/README.md) | tests and tuning | Opt-in profiling of checkpoint cadence and diagnostics of protocol misuse and stage weights. `std`. |
| [`how-far-interpolate`](crates/how-far-interpolate/README.md) | displays | Accessory: `ProgressSmoother` smooths a polled tree's fraction between reports, at each running stage's own pace. `no_std + alloc`. |

All four need Rust 1.88.

- [Design](docs/how-far-design.md)
- [Testing and tuning](docs/how-far-testing-and-tuning.md)
- [Validation](docs/how-far-validation.md)
- [Cross-crate examples](examples/how-far-app/README.md)
- [Measurements](benchmarks/how-far-2026-10-06.md)

## Status

Not yet published. `how-far` uses `enough::AsStopReason`, which is on
`enough`'s main (imazen/enough#40) but in no release yet; until it is, the
workspace takes `enough` from that commit of its main.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE),
at your option.
