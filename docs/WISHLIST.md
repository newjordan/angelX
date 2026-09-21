# Wishlist — private notes

Not promises. Things worth building, in rough priority order, so they stop
living in one head.

## R&D

- **Internal model strength ranking for efficient modular club/formation R&D.**
  Measure each model's real strength on *this harness's* workloads — not a
  public benchmark — and keep a running internal ranking. Then club/formation
  research becomes modular and efficient: swap seats by measured strength per
  task class, spend GPU seats where the ranking says they pay, and let
  formation search start from ranked defaults instead of guesses. The paired-
  sample benchmark machinery already exists; what's missing is the persistent
  per-model ladder and a picker that reads it.

## Accepted direction, not yet scheduled

- Charts inside the world pane: the site's chart plates are their own panels
  today; the composition (miniviz world + live plot/bars in one frame) is a
  cockpit layout change, not a capture change.
- `verify-frames.mjs` should take the server lifecycle with it (spawn/teardown
  its own http.server) so the gate is one command.

## Small

- `frames-to-png.mjs` braille/blocks font check as part of the gate (fail if
  the fallback font lacks braille coverage).
- A `/wishlist` command that prints this file's open items.
