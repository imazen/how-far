# Project notes

## Known Bugs

- Fixed in the cancellation-fixture follow-up: the host pipeline test could
  complete every worker before any observed its polling callback's stop.
  ARM CI exposed the race. The fixture now serializes polling and checks the
  stop after dispatch; the cancellation assertions remain unchanged.

- Fixed in the feature-cost audit: compiler command replays overwrote dependency
  metadata in shared output directories and could fail to resolve the tracker.
  Replays now write separately from Cargo-built dependency artifacts.

- Fixed in callback isolation: Cargo feature unification added an optional
  dispatcher slot and checkpoint branch to every ordinary tree. Private policy
  types now separate ordinary cancellation from callback dispatch; the build-cost
  guard requires branch-free ordinary checkpoint code with callbacks available.
