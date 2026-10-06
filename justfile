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
