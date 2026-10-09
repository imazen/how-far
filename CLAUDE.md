# Project notes

## Known Bugs

- Fixed in the cancellation-fixture follow-up: the host pipeline test could
  complete every worker before any observed its polling callback's stop.
  ARM CI exposed the race. The fixture now serializes polling and checks the
  stop after dispatch; the cancellation assertions remain unchanged.
