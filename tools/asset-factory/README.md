# Delve micro asset factory

Quality before throughput. The factory produces native ink assets, keeps source and stage evidence, and publishes only assets accepted by the gate. It reuses the quick rig, the game's anchor table, ink palette and existing local image-generation services. No image calls are made to Grok.

## Compiled catalog contract

`crates/delve/assets/factory/catalog.json` is version 1 with an `assets` array. Each entry has a unique `id`, `name`, `brief`, `kind` (`body`, `weapon`, `worn`, `vfx`, `prop`, `tile`), `frames` (arrays of ink rows), `pivot` (integer pixel pair), `anchors` (named integer pairs), and `part` (existing worn part, default `body`). Bodies also contain `rig`, the complete version-1 quick-rig game export. Weapons contain `attack` with `clip`, `sound` (an existing game sound pick) and `vfx` (arrays of ink rows). All output ink rows use the game's palette and binary alpha. Catalog entries contain no local paths and add no save or wire fields.

The authoring evidence is separate from the compiled catalog. Failed assets must not be installed. Replacing a catalog entry is explicit and keyed by id. Runtime consumers use the checked catalog, ordinary Sprites, and quick-rig samples; the factory is not another compositor.

## Run

From the repository root (add `bin` to PATH for literal `factory`):

```sh
export PATH="$PWD/bin:$PATH"
factory make body "A ragged ash knight with two free arms and planted boots" --id ash-knight
factory make weapon "An iron spear with an ochre binding" --id field-spear --clip thrust --sound swing
factory make worn "Empty broad riveted breastplate, no head or limbs" --part body --id field-mail
factory make vfx "Unequal iron impact shards, a small ochre centre" --id iron-impact
factory make prop "A small tavern cask with a side tap" --id spare-cask
factory make tile "Uneven muted grey flagstones" --id grey-flags
factory make body "My monster" --source /absolute/source.png --id my-monster
factory batch tools/asset-factory/batch.json
factory review
```

`bin/factory` uses `uv` with the quick rig's existing numpy/Pillow dependency
set. `FACTORY_PYTHON=/path/to/python` selects an already provisioned interpreter.
`factory --out /absolute/evidence make ...` changes evidence location (default
`$TMPDIR/asset-factory`, `/tmp` when unset). Generation uses `FACTORY_COMFY_URL` (default
`http://127.0.0.1:18189`), a local-only tunnel to the already installed Spark
ComfyUI. No model downloads, backend installs, host, tunnel or GPU services are
silently launched by the command. If the image service is unavailable the
command fails visibly before dependent quality measurements. Source import
works without a service. `--key RRGGBB` is explicit: opaque corners are never
guessed away. Generated sources explicitly request a flat magenta ground.

`body` has the alias meaning character body, not armour. Worn `--part` accepts
the canonical anchor names: `head`, `body` (torso), `back`, `feet`, and the
other names in knight-fit.json. The short/wide torso gate deliberately rejects
whole figures. All source resizing is uniform; no squashing a body into armour.

## Stages and decisions

1. **Generate/import.** Preserve the source master and hash. Reserve a generation
   call before sending it; log workflow/prompt/seed/status/output hash/duration.
   The lane-wide evidence log allows at most 30 calls, including failures.
2. **Clean.** Explicit alpha/key, a single uniform scale to the canonical native
   part size, binary alpha, exact muted game inks; no signal-bank white/glow.
3. **Fit.** Bodies use quick.fit and editable masks/joints. The saved source rig
   has all 13 shared clips. Actual raster floor compensation is saved in each
   gait pose's offset and rebuilt through quick.build, never edited into final
   rows. Weapons export painted grip/tip, full-frame pivot, chosen clip, VFX and
   an existing game sound pick. Runtime validates sound names against the actual
   bundled MP3 index. Worn sizes come from the same canonical fit table.
4. **Animate.** Full quick clips for bodies; four native directional attack VFX
   frames for weapons; fixed-pivot dissipating frames for hit sparks. Static
   props, tiles and armour stay static rather than fake animation labels.
5. **Gate.** Reject malformed/empty prerequisites; check alpha, style inks,
   warm-accent proportion, silhouette mass/connectivity, native size, pivot,
   frame registration, full rig schema/clip coverage, and painted gait floors.
   True one-pixel flecks/excess ochre get a targeted local repair and measured
   re-gate; missing anatomy or a tall armour figure requires a source/rig edit.
   A failure is recorded beside the asset and does not publish a replacement.
   No blind image rerolls. `--fixup /path.json` applies explicit reviewed native
   rows and a mandatory explanation, retaining before/after stages.

Native readability and style still require looking: the page says **visual
review pending** until `factory approve ID "specific inspection notes"` is
recorded. Automated green is not represented as proof of artistic quality.
The supplied batch was inspected at native scale and enlarged, corrected, then
seen in real arena screenshots. Review histories retain previous records;
`fixups/*.json` and `refine_samples.py` are reproducible authoring edits, not
replacement tests or unlogged generated art. Timings include actual original
Qwen time even when rebuilding a source master from disk. Original source hashes,
prompts, seeds and call ids are committed in `sources/generation-receipts.json`;
the full workflow log is preserved with the review.

## Outputs and game use

- `crates/delve/assets/factory/catalog.json`: compiled game-ready ink/rig catalog.
  Rust includes it at build time; no workstation paths are saved or sent.
- `$TMPDIR/asset-factory/jobs/ID/`: source, cleaned/fitted/animated PNGs,
  authoring rig, complete game export, gate reasons, timings and history.
- `$TMPDIR/asset-factory/review/index.html`: standalone scroll-border review.
- `$TMPDIR/asset-factory/shots/ID-{1,4}x.png`: production arena PNG evidence.

Rebuild the game after changing the compiled catalog. There is no claim of
live disk hot-reload. The public production API is
`render::overworld::arena::factory::{catalog, Catalog, Asset, Placement}`.
`catalog()?.install(&mut run.book, Replace::No)` installs checked cards;
`catalog.get(ID)?.apply(&mut run, hero_id, Replace::No)` equips normal Book
weapons/armour/bodies through existing rules. Exact ordinary body Sprites pick
up their complete rig automatically. A replaced/edited card cannot inherit
factory metadata by id alone. `weapon.attack()` exposes its clip, four-frame
VFX and `sound()`; the arena consumes the art, presentation clients retain
sound timing. This lane does not add another audio mixer.

`with_placements` integrates prop/tile/VFX layers around the normal arena
frame. `with_enemy_skins(&[(enemy_id, "marsh-watchman")], || frame_for(...))`
selects either new body for a real enemy without changing its saved kind,
hitbox or deterministic AI. Default bestiary pixels remain protected. The
cask is also drawn in the tavern's ordinary scenery. The sample shot hook
shows bodies both equipped and on real enemy instances. No prototype host
needs to be kept running.

## Verify

```sh
PYTHONPATH=tools/sprite-rig uv run --no-project --with numpy --with pillow --with pytest \
  python -m pytest -q tools/asset-factory/tests \
  tools/sprite-rig/tests/test_quick.py tools/sprite-rig/tests/test_quick_quality.py
CARGO_BUILD_JOBS=4 \
  cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast
ANGEL_ARENA_SHOTS=$TMPDIR/asset-factory/shots \
  CARGO_BUILD_JOBS=4 \
  cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast \
  write_factory_shots -- --ignored
factory review
```

Survey and backend distinctions: [SURVEY.md](SURVEY.md).
