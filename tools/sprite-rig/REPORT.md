# Quick rig: audit, implementation, and factory handoff

Branch: `delve/quick-rig`. Base: `d59b1096a`, also the merge base with
`play/latest-20261025`. No push. External report and evidence:
`/tmp/quick-rig/REPORT.md` and `/tmp/quick-rig/shots/` (outside the checkout).

## Outcome

The main animation authoring path is now `spriterig quick`: a native-pixel,
15-part 2D cutout rig with editable source-space joints/masks, 13 common clips,
per-frame contacts, and exactly the ink-frame JSON the game consumes. The real
arena uses it for the generic, fully equipped realm knight, including ordinary
worn equipment. Named authored kits, lost-gear stages, avatars, shapes and old
whole-body sheets retain their previous paths. No save/wire fields, Rust
libraries or heavy Python dependencies were added.

**Not claimed:** automatic understanding of hidden anatomy; arbitrary-view
reconstruction; a new wish-card directive; a disk-scanning asset registry;
replacement of the game's death/bones mechanic. Those are separate from the
rig foundation. The per-sprite in-process registration API is available to the
next asset factory, with checked old sheet rows as the portable fallback.

## 1. Audit of PR #28 and the starting branch

Read `DESIGN.md`, `PART2.md`, `README.md`, `CREATIVE.md`, the implementation,
tests, and the existing curated showcase. Compared this branch's tool against
the PR ref, not just its PR description. The branch was already materially
better than the PR: `native.py`, real-frame tests, canonical `knight-fit.json`
integration and strict `compare_game.py` parity were branch-only additions.
The PR ref still had the experimental mesh renderer and two experimental 3D
GIFs; this starting branch had already removed them. **This work did not delete
or restore those files.**

### Solid, retained pieces

- Native export uses actual Rust pixels and resolved `Body` fits. It does not
  reconstruct body art from source-text guesses. The asymmetric helm example
  and exact RGBA/fit comparison are useful authoring tools.
- The native tests exercise real errors and geometry: profile validation,
  Rust-compatible rounding, mirroring, alpha and static worn placement.
- The 27-bone hierarchy, FK, BVH roundtrip, imports and palette-pure billboard
  work are real reusable creative research. They are not the tiny skeleton a
  16px skinner needs, but are not fake solely because they are non-game paths.
- The doll clip DSL, separate gear/loadouts and anticipation keys are useful
  creative references. Kept them working rather than replacing all tracks.
- Existing Python dependencies are sufficient: numpy and Pillow. Pytest is
  only for verification.

### Broken promise / disconnected paths

- At the start, no creative output was loaded as a new animated knight body by
  the arena. Native export was intentionally a parity/static-worn workflow;
  it explicitly said new body motion did not hot-load into the game. A dance
  reel or side-scroll GIF was not proof that any in-game knight could use it.
- Multiple rigs, axes, FPS values and export formats existed. The 27-bone
  Mixamo hierarchy and 48px doll path were not a shared game anchor contract.
- `delve.source` parses Rust artwork for creative cutouts. That is not a
  reliable replacement for authoritative native export. The new source knight
  is exported by Rust instead; only the palette is read from the existing ink
  table in Python.
- Old creative tests were not all empty: rest-skin identity, weight sums,
  import roundtrips and palette checks are valuable. Tests that merely assert
  a GIF/file exists, a frame is nonempty, or two arrays differ cannot establish
  contact stability, readable poses, timing or game integration. Historical
  benchmark tables measure that path's render cost, not game-readiness.
- Existing showcase art contains useful helmet/plume and shield silhouettes,
  but some sword contacts read as detached sticks, raised blades merge into
  the helmet, and attack effects can dominate the body. The larger doll reel
  is reference art, not evidence its details survive at a native 16px body.
  GIFs were sampled as images; they were not falsely classified as static just
  because a sampled frame was static.

### What the game actually needed

`arena/knights.rs` draws kit art, selects held items/poses, and resolves worn
fits. `figures.rs` then dresses, mirrors, rolls, petrifies, tosses and places
those figures. `rules/looks.rs` supplies legacy `Sprite`, `Body` and `Part`;
`wear.rs` fits ink and painting layers. The native authoring table is
`crates/delve/assets/knight-fit.json`.

Previously, the body fit, hard-coded hand sockets, held-weapon pose and arena
floor handling were independent. A padded animation could use the right
picture but the wrong fit, acquire a different floor when stone/tossed, or be
rotated a second time during a roll. A modern rig needed one timed sample
carrying image, sockets and placement together, while keeping old saves and
native untimed APIs stable. Art-slot banks and wished sheets already have their
own loading/identity policy; replacing them indiscriminately was not needed.

### Audit images (external report paths)

![Prior creative sheet at native scale beside new 16px source](shots/audit/scale-review-1x.png)

![Legacy game knights, actual 1x](shots/legacy-contact-1x.png)

![Exact native parity, common integer zoom](shots/native-parity.png)

## 2. Quick-rig tool

Files: `spriterig/quick.py`, `quick_motion.py`, CLI dispatch in `__main__.py`,
`samples/quick/knight/{source.png,rig.json}`, and the small squire/sheet example.
The README has **Rig a new character in 3 commands**.

- Fit a transparent sprite, explicit-keyed generated image, or a selected sheet
  cell without silently resizing source art. Facing normalization is saved.
- Small hierarchy: head, torso, back/cape, upper/lower arms and legs, separate
  hands and feet. Disjoint source masks and joints are JSON-editable. A cape
  can legitimately have an empty mask when the source has no cape.
- Rigid inverse-nearest raster sampling avoids forward-splat holes. Joint
  collars borrow only existing neighboring pixels; they do not invent hidden
  anatomy or blur the rest sprite. No rest-pose silhouette is redrawn.
- Contacts are chosen from the owning part's actually rasterized pixels.
  Rounding a continuous wrist independently had left transparent contacts;
  that defect was found and fixed. `tip` is deliberately a directional endpoint,
  not required to be painted body ink.
- Shared clips: idle, walk, run, roll, hurt, die, cast, slash, overhead, thrust,
  flail, bow, gun. Swing action/smear frames are shorter than anticipation and
  recovery; smears use the tip trajectory and source highlight inks.
- Fixed canvas/pivot. The real knight's editable fixup corrects the raster floor
  for each locomotion pose; the gate caught a one-pixel-high run before that
  correction. It does not hide bad grounding by weakening the check.
- Export `game.json`, lossless atlas `sheet.png`/`sheet.json`, reloadable rig and
  a GIF per clip. GIF clock is **30Hz**, matching Delve, not the initially wrong
  60Hz assumption. Regression: 6 game ticks previews as 200ms.
- Bounded input/JSON allocation and runtime-parity validation: 128px runtime
  canvas, 128 frames/clip, 1024 total frames, 1..3600 ticks/frame, idle required,
  standard clip names and 8MiB JSON. Source images have separate bounded limits.

## 3. Shared runtime contract and real game integration

Files: new `arena/quick_rig.rs`, `knights.rs`, `figures.rs`, `kingdom.rs`, arena
module registration, and the compiled `assets/quick-rig/knight.json`.

Version 1 is `{version, canvas:[w,h], pivot:[x,y], clips:{name:[frame...]}}`.
Each frame is `{ticks, rows, anchors}`. Rows are final facing-right Delve inks,
with `.` transparent. Anchors are `grip`, `off_hand`, `head`, `back`, `feet`,
`tip`. Tip is also the muzzle/direction endpoint; consumers should not invent a
second weapon pivot. Mirroring is `x = width-1-x` for rows, contacts and pivot.

`Rig::parse` fails atomically on invalid fields, palette, dimensions, duplicate
clips, empty idle, timing or sockets. Sampling uses integer game time/distance,
loops locomotion, and clamps one-shots. Missing optional clips fall back through
walk/slash/idle. A malformed compiled replacement falls back to legacy art;
the tracked asset itself is required for compilation.

The live arena's `dressed_frame` selects once, resolves the image and fit from
that sample, composites ordinary gear, and carries a pivot/roll capability.
Walking, running, rolling, hurt, casting and attacks reach actual `arena::frame`.
Cast age comes from the event rather than arbitrary global phase. Forged/equipped
weapon labels choose slash/overhead/thrust/flail with geometry fallback. Native
bow/gun visuals remain connected to the grip. Raised/resting shields retain
aim, throw and hit feedback.

Worn right-hand hard inks now orient from the existing contact toward exported
`grip -> tip`; painting rectangles rotate about the same pixel-center contact.
The original center/bottom `Part` contact is preserved, including mirroring,
source motion and transformed bounds. Left/head/back gear does not accidentally
inherit weapon rotation. The source-up convention is documented by the runtime
helper. Built-in held art is suppressed when a right worn weapon replaces it.

Old `figure`/`body_of` calls retain native untimed geometry. Live generic fully
armored knights use the rig; named authored identities and missing-gear stages
keep their own silhouettes. Old whole-body wishes are not silently promoted to
a rig or allowed to borrow its roll/pivot. `with_sprite_rig` is a scoped client
association for a parsed rig and exact checked `Sprite` rows, including unwind
restoration. No filesystem path or pointer enters a save or network message.

There is no new automatic loader for a run's `.angel/dungeon/sheets`; the factory
seat can register those assets explicitly. The compiled knight is the shipped
production loading path. The game's death/bones presentation remains unchanged;
the die clip is available to asset consumers. The main body is a right-side
view plus mirroring, not newly authored front/back art.

## 4. Quality gate, tests, and evidence

One command, from `tools/sprite-rig`:

```sh
python tools/review_quick.py --input samples/quick/knight/source.png \
  --rig samples/quick/knight/rig.json \
  --game ../../crates/delve/assets/quick-rig/knight.json \
  --out /tmp/quick-rig/shots/gate-final
```

It first runs the pre-existing Rust export with explicit legacy mode, then
measures that usable input. It fails on prerequisites rather than treating a
failed export as zero quality. An explicit `--native` supports offline review.
It writes clean and socket-overlay contact sheets at 1x/6x, all clip GIFs and
`checks.json`. Contacts share a floor and pixel scale with actual legacy
knights; transparent padding is cropped only for layout, never body resizing.

Checks include schema/palette/coverage, fixed idle/walk/run floors, silhouette
mass/connectivity heuristics, painted contact proximity, exact source rebuild
of every pixel/tick/socket, source contact ownership and actual smear-added ink.
Mutations test broken floors, detached/incorrect contacts, invalid baselines,
unchanged clips, missing smears and runtime-unloadable exports. Readability
heuristics are explicitly not a substitute for visual inspection.

### Verified results on the final implementation

- Required command: `CARGO_TARGET_DIR=crates/target/quick-rig
  CARGO_BUILD_JOBS=4 cargo test --manifest-path crates/Cargo.toml -p angel-delve
  --profile fast`: **2884 passed, 0 failed, 304 ignored**; doc tests green.
- Whole pre-existing plus new Python tool suite: **220 passed, 3 skipped**.
  The skips are optional source/motion-data tests, not hidden quick-rig failures.
- Fresh-reference one-command quality gate: **PASS**, every reported check pass.
- Native workflow retained: **24/24 realm frames exact RGBA + fit parity**.
- Ignored production shot hook: **1 passed**, 48 real arena frames plus crops,
  strips and exact-nearest 1x/4x PNGs under `shots/arena-final`.
- `git diff --check`: clean. Three pre-existing Rust test unused-variable warnings
  remain outside this work. No cockpit struct fields were added, so the
  conditional cockpit check was not required; its test suite was not run.

Rust tests cover parsing/limits, action timing and fallback, mirroring, anchor
geometry, actual wished and ordinary-geared arena walk/roll/swing composition,
cast origin, shield feedback, stone/toss pivots, newer legacy sheet precedence,
painting transforms and right-weapon orientation. Existing native/posse/painting
assertions were retained. Two historical pixel snapshots explicitly scope the
legacy renderer rather than pin the intentionally changed new art. New tests
exercise the default rig independently. The original snapshot hashes were not
replaced with freshly blessed new values.

### Visual review and remaining gate

The initial images found an opaque-background source bug, a one-pixel run floor
error, static-looking idle, weak ranged signals and an axis-aligned worn sword.
These were fixed before final tests. The final all-GIF frame sheet was visually
reviewed: all 13 clips coherent, no empty/cropped bodies or detached limbs.
All final idle/walk/run/roll/hurt/cast arena samples were visually reviewed at
1x; earlier all geared clip crops were also inspected and drove weapon rotation.

**One requested gate remains incomplete:** the image sidecar hit its usage
quota before re-inspecting the last *final, newly oriented* melee/bow/gun arena
PNG pages. A second read-only review route had no usable image viewer. It
verified all 216 PNGs decode, all 108 scale pairs are exact nearest-neighbor,
all 48 crops match their arena frame, and both overview pages contain exact
full-frame tiles. Those are artifact integrity checks, **not visual signoff**.
The remaining review targets are `shots/final-arena-melee.png`, bow/gun in
`shots/final-arena-body.png`, and `shots/final-arena-overview-1-1x.png`.
This limitation is not concealed by the green unit tests.

![Final clip frames, true scale](shots/all-gif-frames-1x.png)

![Final body poses, enlarged](shots/final-body-3x.png)

![Final production arena melee gear](shots/final-arena-melee.png)

![Final production arena body/ranged gear](shots/final-arena-body.png)

## 5. Factory handoff and open questions

- Start from the README three commands. Edit masks/joints/contact offsets when
  fit is wrong, rebuild, run the gate, inspect it. The source/fixup/compiled
  knight trio is reproducible and tested; generated output is not the authoring
  source of truth.
- Import a new game-ready asset by replacing the compiled knight JSON and
  rebuilding, or use `Rig::parse` plus `with_sprite_rig` in the client's asset
  registry. Do not put local paths into saves.
- Future work: named-kit/front-back directional authoring, explicit factory
  disk/manifest registration, and optional weapon source-axis metadata. These
  are not faked by repeating one image under different labels.
- Die is exported but does not override the game's bones presentation. A single
  input view cannot reconstruct occluded anatomy. Bigger/generated art needs
  proper alpha, native-scale art direction and manual masks, not blind downscale.
- No working creative code was deleted. The prior removal of 3D remains as it
  was; creative import/doll/skel systems stay out of the production quick path.
- Locked cockpit/harness files were not touched, no seal script was run, no
  long-lived probe/server was started, no new Rust dependency or save field was
  introduced. Parallel-seat gameplay/UI files were not edited.
