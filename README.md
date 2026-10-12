# <img src="https://raw.githubusercontent.com/imazen/how-far/main/site/static/icon.svg" width="36" height="36" alt="" /> how-far [![CI](https://img.shields.io/github/actions/workflow/status/imazen/how-far/ci.yml?style=flat-square&label=CI)](https://github.com/imazen/how-far/actions/workflows/ci.yml) [![MSRV](https://img.shields.io/badge/MSRV-1.88-blue?style=flat-square)](https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field) [![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue?style=flat-square)](#license)

[Documentation and interactive examples](https://imazen.github.io/how-far/) · [API reference](https://docs.rs/how-far)

Add cooperative cancellation and progress to a Rust library through one
argument: `&dyn Pulse`. Your functions keep their ordinary inputs and results.
The caller chooses whether to track progress, display it, or collect diagnostics.

## Start with one function

This library function reports completed bytes and checks for cancellation.
It needs only `how-far`; the application adds `how-far-along` to observe it.

```rust
use how_far::{prelude::*, StopReason};
use how_far_along::{Phase, PulseTree, Total, Unstoppable};

fn checksum(bytes: &[u8], pulse: &dyn Pulse) -> Result<u64, StopReason> {
    pulse.check()?;
    let mut sum = 0;
    for &byte in bytes {
        sum += u64::from(byte);
        pulse.step(1)?; // Count completed work, then check for cancellation.
    }
    Ok(sum)
}

let bytes = b"hello";
let tree = PulseTree::new(
    Phase::new("checksum", Total::Exact(bytes.len() as u64)), Unstoppable);
let observer = tree.observer();
let sum = checksum(bytes, &tree).finish_phase(tree)?;
assert_eq!(sum, 532);
assert_eq!(observer.snapshot().fraction(), Some(1.0));
# Ok::<(), StopReason>(())
```

The library reports work; the caller finishes the root with its result.
`.finish_phase(tree)` records success, failure or cancellation and returns that
same result. Keep it before `?` so errors also reach the tracker. The observer
can be retained by a UI or another thread while work runs.

Pass `&how_far::NoPulse` when you need neither tracking nor cancellation, or
`&how_far::StopOnly::borrowed(&token)` for cancellation alone. A `Pulse` is also
an [`enough::Stop`](https://github.com/imazen/enough), so existing stop-aware
code can use the same handle. For tight loops, `pulse.paced(64)` batches reports
and checks; finish with `pace.finish()` to check the last partial batch.

## Add it to your project

Use the 0.1 release:

```toml
[dependencies]
how-far = "0.1.0"
how-far-along = "0.1.0"
```

For a library, keep only `how-far` in production dependencies; put the tracker
in `dev-dependencies` when it is used only by tests and examples. All three
crates need Rust 1.88. Core is `no_std + alloc`, has no feature flags, and depends
only on `enough`. [Dependency details](docs/how-far-integration.md#dependencies)
cover diagnostics and matching an existing `enough` dependency.

## Grow with the work

Start with checkpoints. Add structure when you have distinct operations to show:

| Your work | Pattern |
| --- | --- |
| Different sequential operations | Bind variables with `Stages::define`, then `stages.begin(decode)` and ordinary function calls. [Example](crates/how-far/README.md#several-operations) |
| Independent branches that join | `stage.fork([specs...])` gives each branch a handle; your threads or async runtime schedule and join them. [Example](docs/how-far-integration.md#forkjoin-with-different-operations) |
| The same operation over a runtime-sized list | `Stages::new` and `begin_next()` inside the loop. [Example](examples/how-far-pipeline/src/lib.rs) |

Each active handle uses the same `.finish_phase(handle)?` result handoff.
Stages need no closures; `run` and `complete_with` remain optional conveniences.
[The integration guide](docs/how-far-integration.md) explains custom errors,
optional stages, cancellation and caller-owned completion.

| Crate | Use it for |
| --- | --- |
| [`how-far`](crates/how-far/README.md) | Library-facing cancellation, counters, weighted stages and result handoffs. `no_std + alloc`. |
| [`how-far-along`](crates/how-far-along/README.md) | Application progress trees, snapshots, callbacks and optional display smoothing. `no_std + alloc`; `std` is the default. |
| [`how-far-really`](crates/how-far-really/README.md) | Tests and tuning: find missing handoffs, checkpoint gaps and uneven progress. Requires `std`. |

Try the [cross-crate example](examples/how-far-app/README.md) from this checkout:

```sh
cargo run -p how-far-example-app --example integration
```

The quickstart and integration guide run as doctests. See
[design and API boundaries](docs/how-far-design.md),
[testing and tuning](docs/how-far-testing-and-tuning.md),
[validation coverage](docs/how-far-validation.md), and
[compile-cost measurements](benchmarks/stage-reservations-2026-10-12.md).

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE),
at your option.
