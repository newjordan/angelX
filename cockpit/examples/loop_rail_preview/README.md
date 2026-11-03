# Border racer, without a display

No Host, window, agent or loop is opened. The Rust example draws the production
hand-pixel toy, plus the production loop ticker text, at a 1500×880 window / 150×44
cells (10×20 pixels). Python composites an explicitly labelled cockpit fixture;
these are **not screenshots of the live window**. The reviewer owns that check.

From the repository root:

```sh
CARGO_BUILD_JOBS=4 \
  cargo run --manifest-path cockpit/Cargo.toml --profile fast --features glass \
  --example loop_rail_preview
python3 cockpit/examples/loop_rail_preview/compose.py
```

Outputs default to `$TMPDIR/loop-rail/shots` (`/tmp` when unset); override `ANGEL_RAIL_OUT` for both
commands. Python uses installed Pillow/fontconfig, with `ANGEL_RAIL_FONT` as an
optional font path. No Cargo dependency was added.

- `full-lap.gif`: full native-size clockwise lap, using production corner pacing.
- `shot-*.png`: full-size stills; `frame-*.png`: all 240 native frames.
- `review-*.png`: all 240 toy crops, nearest-neighbour enlarged, numbered.
- `model-comparison.png`: hand pixels vs race raster at border scale.
- `state-*.png`: working/waiting/operator/finish toy states.
- `producer-cost.txt`: producer-only active/idle microbench (not full-window CPU).

Production is glass-only. `ANGEL_LOOP_RAIL=0` disables at startup;
`/world rail [on|off]` is the Realm display toggle (no argument toggles).
`ANGEL_TUI_MOTION=reduced|off` also suppresses it. It leaves plain terminals and
small windows/fonts unchanged. It reserves 2 columns / 1 row of outer chrome only
while visible and uses no input targets. Completed runs slow to chequers for six
seconds, then disappear; a restored already-finished run does not replay them.
