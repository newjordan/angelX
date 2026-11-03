# angelX 0.2.6

angelX 0.2.6 is the Heartglass build: the October dev landing and the
Heartglass campaign panel arrive with a repaired post-edit harness. It keeps
the window and showcase from 0.2.5, adds the heartbeat, boards, and stage
cycle from Heartglass, and puts the settled loop and advisory systems under
an operator lock.

## Upgrade

```sh
angelX update
```

Or install fresh with the one line in the [README](../README.md). From a source
checkout, `git pull` and run `./bin/angelX` again.

## Heartglass

- **The panel.** `/heart` shows or hides the Heartglass in angelX's window.
  It follows the Virtual Embryo campaign's boards, cell cloud, stage cycle,
  phase, countdown, and newest event without writing to the campaign folder.
- **Floating by default.** `/heart` spreads its modules over the window.
  Floating modules can be dragged, resized, hidden, and restored with
  `/heart reset`; their layout persists in `~/.angelX/heartglass-hud.json`.
  `/heart plate` explicitly selects the leaned panel instead.
- **Launch setting.** `ANGEL_GLASS_HEART=1` or `=float` opens the floating
  HUD at startup; `=plate` explicitly opens the leaned plate. It stays off by default,
  and terminal sessions only explain that the panel lives in the window.
- **Bounded reader.** Campaign boards and cell clouds remain untrusted input:
  size, schema, digest, group, stage, point, and score bounds are checked
  before anything is drawn.

See [The Heartglass](HEARTGLASS.md).

## The harness

- **Post-edit review restored.** The first successful edit in a turn earns the
  post-edit logic review once. It asks the seat to trace state transitions,
  invariants, cleanup and empty cases, and error paths before testing; prefer
  the implementing code over generated artifacts; and use the smallest
  relevant pre-existing verifier without inventing proof.
- **Advice names itself.** The restored page says it is harness advice, not an
  operator or user instruction. It no longer tells the seat to finish, and it
  cannot by itself end a turn or a loop.
- **Loop stopping stays operator-owned.** With no acceptance check bound, a
  `LOOP_DONE` claim still ends a loop only when an operator note asks the run
  itself to wrap up or stop.
- **Operator lock.** The settled advisory and loop-stop files are hashed in
  `locks/locked.sha256`. The cockpit gate checks those bytes before compiling;
  changing them requires the operator to approve the exact change and re-seal
  the manifest.
- **Working build profile.** `ANGEL_FAST=1` again selects the fast Cargo
  profile in the launcher, so fleet sessions configured for fast working
  builds run the intended image.

## Source and fleet

The 0.2.6 line combines the October 24 dev landing with the harness repair.
`angelX --build-info --json` reports package version `0.2.6`, the executable
hash, and the bound source digest for a clean committed build.
