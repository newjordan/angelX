# Part 2 implementation handoff

Baseline: `6689fc235b014b6ea8728ff16fb675ee30739d9e`. All implementation edits are
under `tools/sprite-rig`. Rust, the canonical table, and heroes/bar are untouched.

## Findings and changes

The previous comparison took unrelated brasswake/arcade images and creative dolls,
cropped each separately, and resized each to its cell. It could not prove native
silhouettes, original pixel preservation, game attachment origins, or sizes.
Procedural capsule bodies and metre-based sockets were advertised as the supported
fast path even though they do not match the game's integer Body/Part contract.
The creative source parser and cleanup could alter small but important native gaps.

Implemented:

- `spriterig/native.py`: strict native-export loader, exact canonical version-1 fit
  table reader, unchanged native PNG/ink-row export, common-integer preview, static
  worn compositing, and PNG-strip authoring to Rust-compatible fallback Sprite JSON.
- One all-frame union crop; Rust-compatible f32 dimension shrinking/rounding;
  integer-only growth; floor-index sampling; exact odd/bottom origins; six-tick
  animation selection; ordered under/body/over passes; symmetric side/top padding;
  fixed floor; final whole-result mirroring. Native art receives no cleanup.
- Authoring repair does not enlarge an item. Palette import uses explicit alpha,
  nearest realm RGB, 1–4 equal horizontal cells. Empty/lost silhouettes fail.
- Native frame fits resolve placement. The table profile selects only the authoring
  budget. Missing/invalid inputs/profiles/inks fail instead of fabricating geometry.
- `tools/compare_game.py`: both inputs required, exact PNG RGBA + rows + dimensions
  + fit + mirror + frame-key coverage, same integer zoom for original/output/diff.
  Missing/corrupt PNGs, omitted/extra selected-kit frames, and every pixel mismatch
  are failures. All frames asserted; only 16 pairs pictured.
- Default `python -m spriterig` is native. Generic animation remains explicitly
  `creative` (legacy positional input still accepted with a notice). README and
  DESIGN distinguish authoritative game data from creative skeletons/animations;
  old creative instructions retained in CREATIVE.md with non-parity labels.
- Removed the unused experimental mesh renderer, its flags, both skel/dances
  imports/branches, dead FBX skin loader, mesh tests, two experimental GIFs, and
  the misleading v6-vs-game comparison PNG. Clip import and 2D dance tools remain.
- Included an asymmetric transparent worn-item sample and extensive unit/integration
  tests. No new runtime dependencies beyond numpy/Pillow, no GPU/models.

## Inputs required from Part 1

The accepted table is `{version:1, profiles:{NAME:{bounds,head,face,shoulders,body,
back,left,right,feet,aura}}}`: Body fields directly, **not nested `parts`**.
Read from `--angelx` / `ANGELX_ROOT` / enclosing checkout. No tool-owned fit table.

Native frames are `{frames:[{kit,view,pose,tick,w,h,rows,fit,mirror,...}]}`. Rows are
**final native game pixels**, including facing/mirroring. Resolved `fit` is in
pre-mirror native canvas coordinates, with any headroom already accounted for.
No pad/crop/standing-point fields beyond this minimum are required; all extra
exporter metadata is preserved. Static worn output records its additional
`worn_padding` and translates the known `fit` coordinates. It does not reinterpret
unknown exporter-specific metadata. An undressed export preserves it unchanged.

The tool renders fallback inks only, deliberately not an HD painting compositor.
Authored Sprite JSON has `frames`, `at`, `pixel:true`, no invented paint hash.
Existing painting metadata can travel in Sprite JSON, but preview shows its
fallback. Nonempty `moves` is rejected with a visible error; exact movement/HD
compositing must be exported from the game, not approximated by creative motion.

## Validation at handoff

- Baseline supplied by operator: **29 passed, 2 skipped**, BEFORE image at
  `/tmp/worn-fit/shots/pr-before.png` (old style montage, not a parity test).
- Implemented suite: **70 passed, 2 skipped** (`python -m pytest tests -q`).
  Skips are optional Mixamo input and the not-yet-produced native Rust dump.
- Legacy unittest discovery also passed: **30 tests, 1 skipped**. Existing creative
  code emits ResourceWarnings for old unclosed files; no native-path warning found.
- Independent development probe: **32,768** Python sizing vectors exactly matched
  standalone Rust f32 arithmetic/rounding for native part budgets. Probe is ignored
  under `out/fit-vectors`, not a second committed table or a claim of game parity.
- New tests deliberately corrupt actual PNG pixels and exercise missing files,
  missing frames, extra frames, unknown kit, version/profile failure, and native
  preservation. Canonical size changes affect authored output; resolved native
  anchor changes affect preview placement.
- `git diff --check` passed.
- BEFORE, asymmetric sample, and synthetic placement preview inspected via `chafa`
  terminal image rendering; no dedicated vision/image-view tool was exposed.
  The synthetic preview is explicitly labelled SYNTHETIC / not_game, not an AFTER
  parity screenshot.

**Upstream blocker, not a pass:** at handoff the shared checkout
`<worn-fit worktree>` still has no `crates/delve/assets/knight-fit.json`,
and `/tmp/worn-fit/native.json` is absent. The requested real comparison was
invoked and correctly failed on missing canonical input. No fabricated table,
source-parser substitute, or fake native dump was used to make the gate green.

## Run once Part 1 exports the authoritative frames

From this branch/worktree's `tools/sprite-rig`:

```sh
PY=/tmp/worn-fit/pyenv/bin/python
export ANGELX_ROOT=<worn-fit worktree>
export SPRITERIG_NATIVE=/tmp/worn-fit/native.json
$PY -m pytest tests -q -rs
$PY -m spriterig.native --native "$SPRITERIG_NATIVE" --kit knight --out out/authoritative
$PY tools/compare_game.py --native "$SPRITERIG_NATIVE" --actual out/authoritative \
  --out out/evidence/native-parity.png --zoom 8
$PY -m spriterig.native --native "$SPRITERIG_NATIVE" \
  --worn samples/native/odd_helm.png --slot helm --out out/odd-helm
```

The integration gate reads every exported native body/pose, exports all untouched,
compares every knight, and exercises every static worn slot on every real frame.
The comparison JSON records how many frames passed and the common zoom. An
undressed comparison must be used unless the Rust reference has the same gear.
