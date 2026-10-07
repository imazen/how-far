# Changelog

## [Unreleased]

### QUEUED BREAKING CHANGES

<!-- Breaks that will ship together in one leading-digit bump. None queued. -->

### Changed

- Requires `enough` and `almost-enough` 0.4.5 from crates.io, the first releases with `enough::AsStopReason`, instead of a git commit of enough's main

### Added

- `how-far` (Rust 1.88, `no_std + alloc`, depends only on `enough`): the `&dyn Pulse` interface libraries accept for cancellation, completed units and weighted nested phases. `Stages`/`Phases` return the library's own `Result`; `complete_with` keeps an early `?` inside the handoff; `share()` gives owned work the full interface without completion rights; `StopOnly` turns a cancellation token into a pulse. Optional `adapters` (`WithStop`) and `checked` (`TryStages`).
- `how-far-along` (Rust 1.88): tracking trees, allocation-free summaries, snapshots, pollers, JSON schema 2, and optional checkpoint callbacks (`FnPulse`).
- `how-far-interpolate` (Rust 1.88, `no_std + alloc`), an accessory crate: `Interpolator` smooths a polled `how-far-along` tree's fraction between reports on the display side, at each running leaf's own pace, capped one report ahead, never backwards. Neither `how-far` nor `how-far-along` depends on it.
- `how-far-really` (Rust 1.88, `std`): opt-in profiling and diagnostics of checkpoint cadence, callbacks, protocol misuse and stage weights.
- Example codec, pipeline and app crates with scenario tests across threads, Rayon and Tokio; browser and Wasm fixtures; a CI guard on compiled code size. Measurements: [2026-10-06 record](benchmarks/how-far-2026-10-06.md).
