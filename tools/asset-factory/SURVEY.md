# Existing paths surveyed before design

## Rig and anchors

Read `tools/sprite-rig/REPORT.md` first. `tools/sprite-rig/spriterig/quick.py`
is the production source→mask/joint rig→native export, with the 13 clips in
`quick_motion.py`. `crates/delve/assets/knight-fit.json` is the canonical part
size/contact table. Python uses the real ink table through the existing rig
palette reader; no second palette or animation system was introduced.

`arena/quick_rig.rs` parses full frames, variable ticks, pivots and integer
contacts. `knights.rs` resolves one timed sample for the body, wear and weapon
orientation; `figures.rs` draws it. Ordinary portable checked Sprite rows stay
in cards/saves; the compiled catalog associates their exact identity with a
full rig on capable clients. The catalog adds no save or wire fields.

## Wish painter

Read `cockpit/src/drive/together_painter.rs`,
`~/.angelX/wish-paint/painter.py`, actual files in
`~/.angel/dungeon/{paint-jobs,sheets}`, and
the play-test box's `{bridge.py,box-wish-painter}`.

Cockpit chooses `ANGEL_WISH_PAINTER`, else `~/.angelX/wish-painter`, writes
`job.json` and stderr log under `.angel/dungeon/paint-jobs`, passes a JSON job
on stdin, and reads NDJSON `slot,level,file,frames,move,pixel,rows`. It checks
plain filenames, output bounds, PNGs and rows before content-addressed
publication to sheets/phrasebook. Worn, shot, orbit, summon/foe and aura have
separate native size caps. This factory does not bypass that path or alter
existing wishes. Its explicit compiled catalog is for shipped assets.

The remote box wrapper queues jobs and polls lines; the home bridge watches
via SSH, uses stable hashed local job dirs, calls the home painter, and copies
PNGs back. Neither bridge nor a game host was started for this task.

The current rough pass is **Z-Image-Turbo**, via local ComfyUI on the GPU host;
HD and finishing code uses the configured Codex image/pxl path. Rough→2x→4x
plus top-tier animation is a genuine saved ladder: for example the saved
flaming-bullets shot is 16x12, 32x24 and 64x48. It is not evidence for rig
animation and was not relabelled as such.

For this lane's knight direction, use **local Qwen-Image 2.1**, already
installed in the SAME ComfyUI service, not the painter's default Turbo or
Grok. UNET/CLIP/VAE node definitions and actual weights were checked before
submitting. `backend.py` uses those nodes with logged workflow, prompt,
seed, prompt id, PNG hash, status and elapsed time. Local generation has no
paid API charge. Thirty reserved calls is a hard cap including failures.

## Art slots and props

Surveyed `crates/delve/assets/art-slots/*/{slot.json,variants}` and
`crates/delve/build.rs::slot_art`. Slot maps name a stable slot, per-tier PNG
variants, scale, registration/pivot/collision constraints; build.rs compiles
supported map-backed slot art. `brasswake.cask` is an existing reference
(not overwritten). Different adventure slots are not a global bag of loose
PNGs. A guessed new slot would not make the arena consume it. The new catalog
therefore has a checked public API plus explicit arena integration.

`scripts/realm/hy3d_prop.py` imports a Hunyuan GLB through node transforms,
texture/material sampling, weld/decimation and world Z-up conversion to an
OBJ supported by the world3d prop loader. It needs pygltflib,
fast-simplification and scipy in addition to numpy/Pillow. This 16px tavern
cask needs no 3D mesh; the factory exports native prop/tile rows rather than
pretending that a PNG is a finished Hunyuan model.

## Direction and isolation

The source ink palette's `K X g j G J h` tonal ramp plus small `o` ochre
accent avoids signal-bank white/cyan and hypercontrast. The static review
uses double pixel/scroll borders, not blue panels. Existing arena cyan aim
and selection markers are inherited UI, not newly painted asset colours.
No edits were made to the parallel melee-VFX or bathroom-mirror modules,
locked cockpit files, or their tests. The figures edit is only the three-line
explicit factory enemy-art hook; no swing implementation was modified.
