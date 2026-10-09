# how-far dev commands

# Format + regenerate the public-API surface snapshots (docs/public-api/).
# The snapshot runner lives in the workspace-excluded apidoc/ package, so it
# is never built or run by plain `cargo test` or any CI job.
fmt:
    cargo fmt --all
    cargo test --manifest-path apidoc/Cargo.toml

# Regenerate the public-API surface snapshots only
api-doc:
    cargo test --manifest-path apidoc/Cargo.toml

# Verify the committed snapshots are current
api-doc-check:
    ZEN_API_DOC=check cargo test --manifest-path apidoc/Cargo.toml

# Tests at the 1.88 MSRV, as CI runs them
msrv:
    cargo +1.88 test --workspace --all-features

# Interface boundary and cold-build cost guard, as CI runs it
build-cost:
    python3 dev/bench-how-far-build.py --runs 3

# Local library gates; also exercise individual no-default-feature packages.
check:
    cargo fmt -p how-far -p how-far-along -p how-far-really -p how-far-example-codec -p how-far-example-pipeline -p how-far-example-app -- --check
    cargo test --workspace --all-features --color never
    cargo test -p how-far --no-default-features --color never
    cargo test -p how-far-along --no-default-features --color never
    cargo clippy --workspace --all-targets --all-features --color never -- -D warnings
    cargo hack check --feature-powerset --no-dev-deps -p how-far -p how-far-along -p how-far-really --color never

# Repeat the host concurrency contracts to exercise cancellation scheduling.
check-hosts runs="100":
    #!/usr/bin/env bash
    set -euo pipefail
    for ((run = 1; run <= {{runs}}; run++)); do
        cargo test -p how-far-really --all-features --test hosts --color never
    done

# Resolve the separate harness and every dependency, including path patches.
stage-metadata:
    cargo metadata --manifest-path dev/stage-suggestions/Cargo.toml --format-version 1

# Exercise the display smoother without std or other default features.
check-smooth: check
    cargo test -p how-far-along --no-default-features --features smooth --color never

# Execute the integration guide, caller example, and cross-crate contracts.
check-integration:
    cargo test -p how-far-example-app --all-features --color never
    cargo run -p how-far-example-app --example integration --color never
