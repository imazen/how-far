# Changelog

## [Unreleased]

### QUEUED BREAKING CHANGES

None.

## how-far

### [0.1.0] - 2026-10-12

#### Added

- `no_std + alloc` cancellation, counting and weighted phases through `&dyn Pulse`, with ordinary application results and caller-owned completion (`be8cd93`).
- Variable-bound stages via `Stages::define` and `begin`, result handoff via `finish_phase`, and independent fork–join handles via `Stage::fork`; runtime-sized sequences use `begin_next` (`7b88ef7b`).
- Cancellation adapters, shared worker views, optional closure helpers and checked planning APIs are available without Cargo feature gates (`be613b9f`).
- Standalone adoption examples, registry dependency setup and a link to the public guide (`13d787d7`, `38ebee79`).

## how-far-along

### [0.1.0] - 2026-10-12

#### Added

- Tracking trees, allocation-free summaries, snapshots, pollers and schema-version-2 JSON (`be8cd93`).
- Always-available checkpoint callbacks through `FnPulse`, isolated from ordinary trees' state and checkpoint dispatch (`ea85c915`, `4c69b481`).
- Opt-in `smooth` display interpolation, preserving observed outcomes and revised totals (`845d9d1`, `94948ac`).
- `std` by default, with `no_std + alloc` tracking through the application's critical-section provider; no deprecated `enough/std` forwarding (`be8cd93`, `bd7ab30`).
- A standalone application quickstart and links to the public guide (`13d787d7`, `38ebee79`).

#### Fixed

- Weighted parents expose known progress while unknown children retain their reserved share in `unresolved_fraction`; snapshots, summaries and smoothing agree (`94cb4796`).

## how-far-really

### [0.1.0] - 2026-10-12

#### Added

- Opt-in profiling and diagnostics of checkpoint cadence, callbacks, protocol misuse and stage weights, with bounded trace retention (`be8cd93`).
- `UnevenPace` findings and quarter-duration samples for work whose unit costs vary (`72b29a9`).
- Experimental stage suggestions behind `experimental-stage-suggestions`, plus source-site time and activity intervals; shared types retain their normal compatibility guarantees (`742aa83`, `2973961`, `be613b9f`).
- Development-dependency setup, a complete diagnostic example and links to the public guide (`13d787d7`, `38ebee79`).

## Workspace

### [0.1.0] - 2026-10-12

#### Added

- All-target and doctest CI across the platform matrix, package dry runs, API compatibility checks and tested GitHub Pages deployment (`43efef4e`).
- Public guide with highlighted examples and six interactive tracker scenarios: export, cancellation, fork–join, skipping, smoothing and diagnostics (`c1959b7f`, `8bea729f`).
- Cross-crate codec, pipeline and application examples across threads, Rayon and Tokio; browser/Wasm fixtures and a compiled-size CI guard ([measurement record](benchmarks/how-far-2026-10-06.md), `be8cd93`).
- Tested integration-guide examples and a real PNG encoder fixture checking output parity, cancellation, caller-owned completion and padded input (`04398811`, `fa1b4d2c`, `7e1fba51`).

#### Fixed

- Include both license texts in each published package (`e29bcc46`).

#### Changed

- Require Rust 1.88 and registry `enough` / `almost-enough` 0.4.5 (`d174270`).
