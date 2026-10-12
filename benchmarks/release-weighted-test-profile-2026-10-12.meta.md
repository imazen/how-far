# Release gate after the weighted-progress fix

This is the complete benchmark report emitted by the all-target test gate for
`571bf1a2fc92e5b99c3777f8ff81bafab7666a75`. It uses the **unoptimized test
profile**, not a release-performance measurement. The library and benchmark
source stayed unchanged throughout; subsequent working-copy edits only refined
the website and its illustrative trace generator.

- Host: r5900xt; Cargo 1.99.0; zenbench 0.1.9.
- Command: `TMPDIR="$HOME/tmp" run-heavy --mem 16G -- env CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 just release-check`.
- All-target tests, doctests, Clippy, API compatibility against the reviewed API,
  and all three package dry runs passed.
- Resource record: `run-heavy: done rc=0 747s | peak-RSS 0.17GiB | min-avail 53604MiB | peak-load 1.44`.

[Raw benchmark report](release-weighted-test-profile-2026-10-12.txt).
