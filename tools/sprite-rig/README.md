# sprite-rig: native Delve dolls and worn-asset authoring

**The default CLI uses authoritative game frames, not approximate procedural dolls.**
CPU-only Python; runtime dependencies are **numpy + Pillow**. Run commands from
`tools/sprite-rig` (or put that directory on `PYTHONPATH`). Pytest is a test dependency only.

## Rig a new character in 3 commands

Install `numpy Pillow` in your Python environment first. From `tools/sprite-rig`:

```sh
python -m spriterig quick fit character.png --out out/hero/rig.json
python -m spriterig quick build character.png --rig out/hero/rig.json --out out/hero
python tools/review_quick.py --input character.png --rig out/hero/rig.json --game out/hero/game.json --out out/hero/review
```

Between commands 1 and 2, fix `rig.json` if needed: source-space `joints`, the
15 disjoint `#`/`.` part masks, contact `anchors`, and local-angle `clips` are
editable. Output never silently resizes your art. Start with a transparent,
right-facing, unarmed sprite, preferably 16–32 pixels tall. For a wish-painted
sheet add `--cell-size W H --cell INDEX` to **fit**; that selection is saved in the
fixup. For an opaque generated image, explicitly use `--key-color RRGGBB` (or
prepare its alpha first). This is a silhouette fit, not an anatomy model: hidden
limbs, big hats, asymmetric poses and non-humanoids need manual masks/joints.

**Command 3 is the quality gate.** It exports a fresh *legacy Rust renderer*
reference before measuring anything, then writes common-floor, unscaled 1x
contact sheets beside current knights, nearest-neighbour 6x sheets, socket
review overlays, a GIF of every clip, and machine-readable `checks.json`.
It exits nonzero on broken input, palette, floor jitter, detached contacts,
missing clips, source-rebuild mismatch or missing smear pixels. Silhouette
metrics are heuristics: inspect the PNGs and GIFs yourself. `--native FILE` is
an explicit offline option, never a fabricated fallback. Cargo needs the
checkout's Rust toolchain; the command uses `CARGO_TARGET_DIR` and
`CARGO_BUILD_JOBS` if set (defaults `crates/target/quick-rig`, `4`).

**Runtime format:** `game.json` is exactly `arena::quick_rig::Rig::parse`'s
version-1 contract. All clips use one canvas and pivot. Every frame has `ticks`
(**30 game ticks/second**), final-facing-right ink `rows`, and integer sockets
`grip`, `off_hand`, `head`, `back`, `feet`, `tip` (tip is also the muzzle/direction
endpoint). Runtime mirrors pixels and sockets together with `x = width-1-x`.
Grip/offhand/head/back/feet contacts come from the rendered owning part, not
independent hand-tuned game coordinates. `sheet.png` + `sheet.json` provide a
lossless atlas alternative; `game.json` is what the current Rust path loads.

The full set is **idle, walk, run, roll, hurt, die, cast, slash, overhead,
thrust, flail, bow, gun**. Attack ticks distinguish anticipation, contact/smear,
and recovery. One-shots clamp; locomotion loops. `--scale` only enlarges GIF
previews, never runtime rows. `--clips` exports a subset with idle retained.

**Put one in the game:** each seat's plain knight in the real arena loads its
own design at compile time, `crates/delve/assets/quick-rig/knight-<design>.json`
(the seat table is `quick_rig::Design::of_seat`; every design keeps the 46x46
canvas, pivot `[23,30]` and all 13 clips). `knight.json` is the old realm
knight, kept as every design's fallback; its reproducible source and fixup are
`samples/quick/knight/{source.png,rig.json}`. Replace a file with a reviewed
`game.json` and rebuild. The arena samples once for
pixels, equipment sockets and placement, including worn helm/back/weapon,
rolls, casts and attacks. Named authored kits, damaged/missing gear stages,
avatars, shapes and old whole-body sheets keep their existing art/save behavior.
Death remains the game's existing bones presentation; `die` is exported for
new consumers, not a replacement for that mechanic.

For a wish body's **client-side asset factory**, parse `game.json` with
`Rig::parse`, associate its checked whole-body `Sprite` using
`quick_rig::with_sprite_rig(&sprite, Arc::new(rig), || render(...))`, and keep
those legacy rows as the portable fallback. This is an in-process API, **not**
a new wish-card directive, automatic `.angel/dungeon/sheets` scanner, or save
field. No per-machine rig path is put on the wire. The next factory can own
that registry/loading policy without changing this animation format.

Run tool tests with `python -m pytest tests`. For actual arena PNGs at 1x/4x:

```sh
ANGEL_ARENA_SHOTS=/tmp/quick-rig/shots/arena CARGO_BUILD_JOBS=4 \
cargo test --manifest-path ../../crates/Cargo.toml -p angel-delve --profile fast write_quick_rig_shots -- --ignored
```

The native parity workflow below remains unchanged. `creative`, `doll`, `skel`
and user-supplied Mixamo imports are separate reference/experimental tracks;
none is needed to fit or build a quick rig. No 3D renderer was restored.

## 1. Export the real dolls from Rust

Part 1 owns `crates/delve/assets/knight-fit.json` and the native-frame Rust test.
Export it from this checkout (no server or GPU required):

```sh
WORN_NATIVE_OUT=/tmp/worn-fit/native.json \
  cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast export_worn_native
```
The frame dump contains **actual game sprites**, their resolved, per-pose fit,
and whether their final pixels were mirrored. Do not substitute source-parsed art,
arcade sprites, or the creative animation library for this input.

```sh
# Use the checkout with the current Rust/table changes, not a stale worktree.
export ANGELX_ROOT=/path/to/angelX
python -m spriterig --native /path/to/native.json --kit realm --out out/knight
# Equivalent explicit entry point:
python -m spriterig.native --native /path/to/native.json --out out/roster
```

A small asymmetric input is included at `samples/native/odd_helm.png`; see
[samples/native/README.md](samples/native/README.md) for its end-to-end command.

`--angelx ROOT` overrides `ANGELX_ROOT`; otherwise the enclosing checkout is used.
The tool reads the **exact canonical fit table** at
`ROOT/crates/delve/assets/knight-fit.json`. No tools-local table or fabricated fallback.
Absent files, invalid versions/profiles, unknown inks, and empty selections fail visibly.
The realm ink palette is read from that same checkout's `ink.rs`; body art is never parsed.
Use `--kit`, `--view`, `--pose`, and `--tick` to select frames by their exported identifiers.

Outputs:
- `0000.png`, …: native-size transparent RGBA frames, **uncropped**, no cleanup or resampling.
- `frames.json`: reloadable ink rows plus the complete frame metadata and PNG paths.
- `preview.png`: up to 32 frames at **one common integer zoom** (`--zoom 8`), not fitted to cells.
- `workflow.json`: source paths, authoring profile, and applied static worn sprites.
- Add `--animate --fps 5` for lossless native-pixel PNG strips plus `animations.json`
  (resolved fits/offsets per pose) and looping GIF previews. No rescale or floor drift.
  For a new body animation, edit the exported ink rows, poses and fits in `frames.json`,
  then re-export; the manifest is reloadable. The game's body drawings remain authored
  in Rust — this tool does not hot-load replacement body animations into a running server.

## 2. Author a worn item, on the real game doll

```sh
python -m spriterig.native --native /path/to/native.json \
  --worn /path/to/helm.png --slot head --profile knight --kit realm --out out/helm
# A horizontal strip of 2 equal-sized RGBA cells, each shown for six game ticks:
python -m spriterig.native --native /path/to/native.json \
  --worn /path/to/shield-strip.png --frames 2 --slot shield --out out/shield
# Existing fallback Sprite JSONs, in equip order (oldest first):
python -m spriterig.native --native /path/to/native.json \
  --sprite out/shield/worn.json --sprite out/helm/worn.json --out out/loadout
```

`worn.json` is a **Rust `rules::looks::Sprite` fallback**, for example:

```json
{"frames":[[".W.","WhW"]],"at":"head","pixel":true}
```

Put this object in a card's `looks.worn` field, preserving any other `looks` entries.
It has raw local ink rows, **not pre-positioned rows**, so the game fits it to the
resolved body at runtime. Do not copy `frames.json` (the complete dressed body) into
`looks.worn`, or copy the creative doll equipment format into the game.
No `paint` hash is invented: authored assets deliberately use the native fallback inks.

PNG import uses explicit alpha (alpha <128 is clear), no corner-keying, and nearest
RGB palette quantization. Use transparent RGBA and exact realm colours to avoid loss.
One union ink crop is shared across all 1–4 animation cells, retaining relative offsets.
Authoring repair **shrinks only** to `--profile`'s part size; already-fitting art is not
enlarged. At preview/game-render time, a small drawing can grow by whole integer factors
for a larger body. The per-frame native `fit`, **not** `--profile`, positions worn gear.
`--profile` only selects the canonical authoring budget; a missing profile is an error.

Slots: `head`, `face`, `shoulders`, `body`, `back`, `left`, `right`, `feet`, `aura`, `whole`.
Game aliases are accepted (`helm`, `mask`, `cape`, `chest`, `centre`, `center`, `shield`,
`hand`, `hands`, `weapon`, `legs`). `left` means **shield arm**, not screen-left.
`whole` derives from bounds; it is centered, not bottom-aligned or under the body.

### Exact static fitting contract

- Table version 1 profiles directly contain `bounds` and the nine parts, **not a `parts` wrapper**.
  Bounds are `[x,y,width,height]`; each part has integer `anchor`, `size`, boolean `bottom`, `under`.
- Native manifest: `{ "frames": [{ "kit", "view", "pose", "tick", "w", "h", "rows",
  "fit": {"bounds", "head", …}, "mirror": false }, …] }`.
  Rows are final game pixels. Fits use native, pre-mirror canvas coordinates, including
  any body headroom. Extra Rust metadata is preserved in the tool export.
- Rust `f32` aspect shrink, round-half-away; integer upscales only when source fits.
  Clothing parts marked `stretch` fill their body rectangle; helmets and weapons preserve aspect.
  Sampling uses `src_x = crop_x + dst_x * crop_w // dst_w` (likewise y), **not Pillow resize**.
- Origin is `(anchor_x-w//2, anchor_y-(h-1 if bottom else h//2))`.
- Sprite frame is `(tick//6)%n`. Under items, body, over items; equip order within each pass.
- Clamp worn pieces above the body's canvas floor. Grow symmetric side padding and top only;
  shift the fit by that padding, preserving the standing point. Mirror the entire composite last.
- No closing holes, orphan-pixel removal, edge smoothing, invented limbs, or morphological cleanup.

**Scope:** this authoring preview renders static fallback inks, not the game's HD painting
compositor. Existing `paint` metadata is not interpreted; fallback rows remain visible.
Sprites with `moves` are **rejected**, not silently approximated. Preview their motion or HD
painting in the Rust game/exporter. Native frames already containing those effects can be
exported unchanged. The generic tools below are not an alternative motion-parity implementation.

## 3. Assert actual game-pixel parity

```sh
python -m spriterig.native --native /path/to/native.json --kit realm --out out/knight
python tools/compare_game.py --native /path/to/native.json --actual out/knight \
  --out out/native-parity.png --zoom 8
```

Both inputs are mandatory. This checks **every native knight frame** (or `--kit KIT`),
exact RGBA pixels, dimensions, ink rows, resolved fit, and mirroring; missing frames,
missing PNGs, and any differing pixel fail. The PNG shows native original, tool output,
and magenta difference, uncropped at one common integer zoom. A JSON result is saved
alongside it. Only the first 16 pairs are pictured; all selected frames are asserted.
Compare undressed exports, or export an equivalently dressed reference from Rust;
comparing a worn preview against a bare body correctly fails.

The removed v6-vs-game image used unrelated arcade art and scaled each subject separately.
It was a style montage, **not parity evidence**. The operator's original BEFORE screenshot
is external evidence; the tool no longer ships that misleading comparison as a showcase.

### Preview a native animation or an edited pose manifest

```sh
python -m spriterig.native --native /path/to/native.json --kit realm --view right \
  --worn samples/native/odd_helm.png --slot head --animate --fps 5 --out out/walk
```

Only authored native poses are game-exact. The separate creative DSL can prototype
new motions, but does not claim arbitrary Mixamo motion is already a native game pose.

## Tests

```sh
python -m pytest tests -q
# Also exercise the actual current Rust export (requires Part 1):
ANGELX_ROOT=/path/to/angelX SPRITERIG_NATIVE=/path/to/native.json python -m pytest tests -q
```

Unit vectors cover versioning, aliases, crop/round/sample/origin rules, padding, layer order,
mirroring, alpha, animated union bounds, native preservation, failure cases, and deliberate
PNG corruption. The opt-in cross-language gate reads all real exported bodies/poses and
checks knight pixel equality. Missing integration input is skipped explicitly, never claimed
as a passing game-parity run.

## Creative tools (retained, non-game parity)

`python -m spriterig creative input.png --out out/creative` keeps generic cut-out and
procedural animation. The legacy positional PNG form still works with a non-parity notice.
`spriterig.delve`, `spriterig.skel`, and `spriterig.doll` are also **creative** tracks:
metre-based sockets and generated poses are not the game's attachment contract.
See [CREATIVE.md](CREATIVE.md), [DESIGN.md](DESIGN.md), and
[showcase/BENCHMARK.md](showcase/BENCHMARK.md). The unused experimental mesh-rendering
track, its switches/tests, and its curated GIFs were removed. No GPU/model dependencies.
