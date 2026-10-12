# Releasing the workspace

The three published crates share version 0.1.0. Example and development harness
packages are not published.

1. Update the three package versions, workspace dependency versions, README
   dependency snippets, and the versioned API links in `site/templates/`.
   Update the per-crate entries in `CHANGELOG.md`, preserving the Unreleased
   breaking-change queue.
2. Review the public API and all three crate READMEs. Obtain the maintainer's
   approval to publish the reviewed READMEs.
3. Run `just release-check`, `just check-docs`, and `just site-check` from a clean
   committed checkout. `release-check` executes all targets (including the
   benchmark harness), doctests, Clippy, API compatibility checks and a verified
   multi-package publishing dry run. Benchmark execution here uses the test
   profile; do not treat those timings as release-performance measurements.
4. Push the release commit. Require both CI and Guide workflows to pass on that
   exact commit, including Windows ARM, Intel macOS and cross-compiled i686.
   Verify the public guide at https://imazen.github.io/how-far/.
5. Tag the verified commit as `v0.1.0`, push the tag, and create its GitHub release
   with generated notes. Publish from a clean checkout of that same commit using
   an authenticated Cargo installation:

   ```sh
   cargo publish -p how-far -p how-far-along -p how-far-really
   ```

   Current Cargo stages workspace packages in dependency order. Do not bypass
   package verification or dirty-tree checks. If publication fails, inspect the
   registry before retrying; some packages may already have been accepted.
6. Confirm each registry version, README, repository/homepage link and docs.rs
   build. Keep the release tag on the commit that was actually published.

For the first release, `cargo semver-checks` compares against the approved merged
API at `7b88ef7b55a2253422e3728106aafd8437724328`; this is not a previously published
compatibility baseline. For later releases, also compare against the latest
published version. Review the opt-in experimental stage-suggestion API separately
from the shared types whose normal compatibility guarantees still apply.
