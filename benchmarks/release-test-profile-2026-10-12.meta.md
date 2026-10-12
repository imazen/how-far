# Release gate: test-profile benchmark execution

This is the benchmark output produced by `cargo test --all-targets`, preserved
for the release record. It uses the **unoptimized test profile** and is not a
measurement of release-build overhead. Use the existing release measurements
in the other benchmark records for performance evaluation.

- Library and benchmark source: `38ebee79bf64b1dd70853f846275176dd8219f71`.
- Working copy also contained the new documentation site and its example trace
  generator. Neither changes the library or benchmark source.
- Host: r5900xt; Cargo 1.99.0; resolved zenbench 0.1.9.
- Command: `TMPDIR="$HOME/tmp" run-heavy --mem 16G -- env CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo test --workspace --all-features --all-targets`.
- Entire command succeeded, including the library/consumer tests and all benchmark groups.
- Resource record: `run-heavy: done rc=0 742s | peak-RSS 0.17GiB | min-avail 53406MiB | peak-load 0.72`.

[Raw output](release-test-profile-2026-10-12.txt) is the benchmark's complete
saved report; no values were extrapolated into release-build claims.
