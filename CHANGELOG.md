# Changelog

## [Unreleased]

### QUEUED BREAKING CHANGES

<!-- Breaks that will ship together in one leading-digit bump. None queued. -->

### Fixed

- Synchronize the host cancellation fixture so dropped polling attempts and worker completion cannot hide its requested stop.

### Changed

- Simplified stage grouping and rendering, and derive site intervals from existing check/report timestamps.
- Simplified unit-pace sample traversal without changing report sampling or late-report accounting.
- Requires `enough` and `almost-enough` 0.4.5 from crates.io, the first releases with `enough::AsStopReason`, instead of a git commit of enough's main

### Added

- `how-far` (Rust 1.88, `no_std + alloc`, depends only on `enough`): the `&dyn Pulse` interface libraries accept for cancellation, completed units and weighted nested phases. `Stages`/`Phases` return the library's own `Result`; `complete_with` keeps an early `?` inside the handoff; `share()` gives owned work the full interface without completion rights; `StopOnly` turns a cancellation token into a pulse. Optional `adapters` (`WithStop`) and `checked` (`TryStages`).
- `how-far-along` (Rust 1.88): tracking trees, allocation-free summaries, snapshots, pollers, JSON schema 2, and optional checkpoint callbacks (`FnPulse`).
- `how-far-really` (Rust 1.88, `std`): opt-in profiling and diagnostics of checkpoint cadence, callbacks, protocol misuse and stage weights.
- `how-far-really`: `Kind::UnevenPace` flags a task whose slowest quarter of reported units took at least `Options::uneven_pace_ratio` (default 4) times as long as its fastest, with advice to split the phase into stages or report a unit that tracks cost; spans record `Stats::unit_pace` (`UnitPace`, four quarter durations) whenever report timing is on, from at most 32 sampled reports.
- `how-far-really`: `Kind::SuggestedStages`, behind the `stage-suggestions` feature, proposes stages for a leaf phase or stop-only code whose checkpoints ran in separate stretches, as `PhaseSpec` sample code named by file and line and weighted by each stretch's share of time; outer loops spanning several stretches are left out, and the time after the last check is a stage of its own. `SiteStats` records `time` (intervals ending at the location) and `active` (first and last call).
- Example codec, pipeline and app crates with scenario tests across threads, Rayon and Tokio; browser and Wasm fixtures; a CI guard on compiled code size. Measurements: [2026-10-06 record](benchmarks/how-far-2026-10-06.md).
