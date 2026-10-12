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

# Compare optional APIs with the default crates without relaxing default budgets.
feature-cost:
    python3 dev/bench-how-far-build.py --runs 5 --feature-audit --wall-time

# Inspect checkpoint dispatch on the same toolchain as the compile-cost guard.
checkpoint-asm features="std" function="<how_far_along::pulse::PulseTree as enough::Stop>::check":
    cargo +1.88 asm -p how-far-along --lib --no-default-features --features "{{features}}" --no-color --simplify "{{function}}"

# Local library gates; also exercise individual no-default-feature packages.
check:
    cargo fmt -p how-far -p how-far-along -p how-far-really -p how-far-example-codec -p how-far-example-pipeline -p how-far-example-app -- --check
    cargo test --workspace --all-features --no-fail-fast --color never
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

# Compile all README/guide examples and check rustdoc links and warnings.
check-docs:
    cargo test --workspace --all-features --doc --color never
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --color never

# Materialize a new fixture directory; existing directories are protected.
prepare-encoder source destination:
    python3 dev/adopt-zenpng.py "{{source}}" "{{destination}}"

# Test a materialized real encoder with progress enabled and disabled.
check-encoder destination:
    cargo test --manifest-path "{{destination}}/probe/Cargo.toml" --features progress
    cargo check --manifest-path "{{destination}}/probe/Cargo.toml" --no-default-features

# Stage reservation, fork lifecycle and forwarding contracts.
check-direct-stages:
    cargo test -p how-far-along --test direct_stages

# Embedded tracker configurations with all optional tracker APIs.
check-embedded:
    cargo build -p how-far-along --target thumbv7em-none-eabihf --no-default-features --features smooth
    cargo build -p how-far-along --target wasm32-unknown-unknown --no-default-features --features smooth
