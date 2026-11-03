> For authoritative Delve dolls, native pixels, and game-ready worn gear, use [README.md](README.md).
> Every workflow on this page is **creative animation, not game-pixel parity**. Its metre-based sockets,
> source parsing, cleanup, and procedural poses are not the worn-fit contract.

# Creative animation (non-game parity)

Rapid CPU 2D sprite skinner/animator. numpy + Pillow only, no models, no GPU, no mocap data.

Pipeline: PNG (alpha, or flat background keyed from the corner pixel) -> humanoid segmentation
(heuristic, or hand-painted polygons via `--override`) -> 11-bone hierarchy -> grid-triangulated
mesh per part -> linear blend skinning (smoothstep weights at each joint) -> procedural /
keyframed motion -> batched rasterizer -> sprite sheets, PNG frames, GIFs and a montage GIF.

## Usage
    python3 -m spriterig creative input.png --out out/ [--anims all|idle,walk,...] [--override rig.json]
        [--mode auto|aa|nearest] [--ss 2] [--scale 2] [--cell 6] [--name hero]
        [--no-frames] [--no-gif] [--no-montage]
    python3 -m unittest discover -s tests

Motions (15): idle, walk, run, jump, attack_right, attack_left (`attack` = attack_right), cast,
hurt, death, celebrate, dance, crouch, block, wave, kick. Loops use Catmull-Rom keys; one-shots
use smoothstep and hold the final key. Motions are authored facing right and mirrored for
left-facing sprites.

Outputs per run: `<anim>_sheet.png/.json`, `<anim>.gif`, `<name>_montage.gif` (every motion,
labelled, on a ground plane), `<anim>/NNN.png` (unless `--no-frames`), `parts.png`
(segmentation debug), `rig.json`, `timing.json`.

### Render modes
- `aa`: 2×2 supersampling (`--ss`), bilinear texture sampling on premultiplied alpha, box
  downfilter. Smooth edges, no fringes or joint seams (part masks overlap 2 px).
- `nearest`: point sampling, hard alpha. Keeps pixel art crisp.
- `auto` (default): `nearest` for hard-alpha sprites ≤96 px tall, else `aa`.

Triangles are rasterized in size-sorted batches of 384 with numpy (no per-triangle Python loop).

### Override JSON (all keys optional)
    {"side_f": "l",            // which image side holds the near ("front") limbs
     "facing": "right",        // sprite's facing; motions mirror for "left"
     "mode": "nearest", "cell": 6,
     "amp": {"upper_leg_f": 0.35},   // per-bone motion multipliers (stiff robes, etc.)
     "backfill": [30, 60, 100, 200], // true/false, or a box limiting torso paint-in behind the front arm
     "joints": {"root": [70,178], "neck": [68,62], "shoulder_f": [52,80], "elbow_f": [44,108], ...},
     "polys": [["torso", [[x,y],...]], ["head", ...], ...]}   // painted in order, last wins
Part names: head, torso, upper_arm_f/b, lower_arm_f/b, upper_leg_f/b, lower_leg_f/b.
Legacy keys `fractions`, `torso_half_width`, `parts` still work. See `samples/frosty/*.json`.

## Samples
`samples/frosty/` holds four of Frosty's sprites (sources in each JSON's `_source` and in
ATTRIBUTIONS.md) with hand-tuned overrides. `samples/knight.png` is a synthetic test figure.
Showcase GIFs: `showcase/<character>/`.

## Benchmark
All 15 motions (176 frames), scale 1, box CPU (Xeon), single thread. Details in
`showcase/BENCHMARK.md`.

| character | sprite px | mode | tris | rig ms | ms/frame | frames | render s | gif s | total s |
|---|---|---|---|---|---|---|---|---|---|
| red_knight | 116×245 | aa | 1670 | 57 | 46.6 | 176 | 9.58 | 1.07 | 10.66 |
| blue_knight | 116×242 | aa | 1612 | 55 | 47.5 | 176 | 9.93 | 1.31 | 11.25 |
| kimi_robe | 104×200 | nearest | 1016 | 31 | 8.4 | 176 | 2.15 | 0.83 | 2.99 |
| loom_warden | 132×200 | nearest | 974 | 30 | 7.9 | 176 | 2.01 | 0.69 | 2.71 |
| synthetic_knight | 96×144 | aa | 442 | 41 | 13.7 | 176 | 2.81 | 0.43 | 3.24 |

## Limitations
- Single-image rigging: anything hidden behind a limb is invented (a blurred, darkened
  paint-in of the torso colour), visible on big arm swings.
- Front-facing robes have no real legs; the legs are robe halves with damped amplitude.
- Props (shields, swords) follow whichever part they are painted into.

## Legacy Delve cut-outs (non-game parity)

`python -m spriterig.delve [--angelx /path/to/angelX] [--out showcase/delve] [--only hero_red,goblin] [--anims walk,run] [--mode pixel|rotate] [--bench]`

The characters are read straight out of the delve Rust source (`crates/delve/src/render/overworld`). Each one is a set of ink rows: one char per palette ink, and `.` means transparent. The source covers the hero kit (helm, harness, steel legs in 3 stride frames, shield, sword strike layer), the four seat recolours, the Brannoc, guard, miner and smith folk, and the goblin.

Each sprite is cut into rigid bone layers: root, torso, head, arm_b, arm_f and legs. The 15 motions from `anim.py` drive these layers.

- `pixel` mode (the default) moves parts in whole pixels and snaps their turns (45° for arms, 90° for the rest). Inks are chosen by a 4x4 supersampled majority vote, so every output pixel is a delve ink. This creative render is not a native-frame parity reference; it can clean up or resample pixels.
- Legs use the authored stride frames.
- Where the source has an authored alternate layer (the hero sword strike), it replaces the rotated layer.
- Nothing sinks below the floor.

Output for each character:
- right/left PNG sheets at 1x and 4x
- 6x GIFs over a dungeon backdrop
- Dotmax-style braille `.txt` and `.ans` files
- `<name>.json` with inks, frames, fps, loop and feet anchor
- `<name>_frames.rs`, which holds `pub const ANIM: [[&str; H]; N]` ink rows ready for `Img::from_rows`

`side_scroll_demo.gif` shows the hero walking, turning and striking through a scrolling hall in front of the cast.

## Shared creative skeleton and 2D billboards (`spriterig.skel`)
The unused mesh-rendering track has been removed. Clip import and 2D billboards remain.

Dances: `MIXAMO_DIR=... python3 -m spriterig.skel.dances` → `showcase/skel/dances/`. Dance import strips root X/Z travel.
Paths: delve art is read from `ANGELX_ROOT` or the enclosing repo (`tools/sprite-rig/../..`); FBX tests run only when `MIXAMO_DIR` is set.
Tests: `python3 -m unittest discover -s tests` or `pytest tests`. Generated output under `showcase/` is git-ignored except `showcase/curated/`.

## 2D doll animator (v5, `spriterig.doll`) – armor, weapon attacks, emotes
```
python3 -m spriterig.doll list                                   # bodies, items, loadouts, clips
python3 -m spriterig.doll export -l knight,mage -c slash,dab --views side,3q --gif --out out/doll/exp
python3 -m spriterig.doll export -l all -c all --views 8dir      # everything: PNG strip + JSON (events, origin) + index.json
python3 -m spriterig.doll export -l knight --mixamo /path/to/mixamo --in-place -c mx_kicking   # Mixamo clips still work
python3 -m spriterig.doll check -l all -c all [--strict]         # off-canvas / stray pixels / pops -> out/doll/check
python3 -m spriterig.doll bench
python3 -m spriterig.doll.reels                                  # rebuild showcase/curated/doll_*.gif
```
Library: `spriterig/doll/library/` – `attacks.clip` (guard, slash, overhead, thrust, spin, combo, shield_bash, bow_shot,
staff_cast, dagger_flurry), `emotes.clip` (idle, walk, dab, floss, chicken_dance, facepalm, flex, trip, tpose_glitch,
take_a_bow, laugh, shrug, sit, sleep, jump, victory), `loadouts/*.json` (11), `equipment.json`, `bodies.json`.

### Add creative doll armor (not game worn gear)
Add an item straight into `equipment.json` (validate with `python3 -m spriterig.doll.make_library`), e.g.
`"spiked_helm": {"kind": "helm", "slot": "head", "material": "metal", "rows": [...], "pivot": [x, y], "size_m": 0.3}`
or a recolour-only piece `{"kind": "limb", "segments": ["LeftForeArm", "RightForeArm"], "material": "gold", "grow_m": 0.01}`.
Use it in a loadout's `equip`; recolour per loadout with `"colors": {"metal": ["G", "g"]}`. Run `check -l <loadout>`.
### Add a creative doll weapon (not game worn gear)
`{"kind": "weapon", "type": "sword", "slot": "main", "socket": "RightHand", "rows": [...], "pivot": [col, row of the grip],
"length_m": 0.9}` – rows are pixel art, tip pointing up; codes M/m metal, L light, G/g gold, W/w wood, P/p S/s
primary/secondary, T/t bone, k outline, `.` clear, any other char = literal delve ink. Off hand: `socket: LeftHand, slot: off`.
### Add an attack or emote
Append to a `.clip` file (or drop a new `*.clip` in the library):
```
clip uppercut
fps 12
loop no
tags attack
0  guard=0                        # or 'rest'
3  ease=in   rarm=-30,0,20 rfore=110 chest=0,20,0 bob=-0.03     # anticipation
5  ease=snap hit smear rarm=150,0,10 rfore=10 chest=0,-25,0     # strike: hit marker + smear frame
9  ease=back rarm=120 rfore=30                                     # follow-through/overshoot
14 ease=out  rest
```
Bones: larm/rarm, lfore/rfore, lhand/rhand, lsh/rsh, lleg/rleg, lknee/rknee, lfoot/rfoot, spine, mid, chest, neck, head,
root; values are degrees `x,y,z`. Then `export -c uppercut --gif` and look at it.

## Historical v6 silhouette styling (non-game parity)

The creative 2D dolls imitate a stout knight style rather than realistic stick proportions.
They do not reproduce game pixels or game fitting:

* **One proportion set for every body** (`DELVE` in `spriterig/doll/render.py`, per-body
  `"proportions"` overrides in `bodies.json`): legs 0.62x, spine 0.82x, forearms 0.74x,
  shoulders 1.35x wider, hips 1.45x wider, big helm-sized head. Bone lengths change, rotations
  don't, so every `.clip` and Mixamo clip plays unchanged on the stout body.
* **`helm_great`** (`"shape": "bucket"`): flat-topped delve great helm with a dark visor slit
  that tracks the facing, brow trim and breaths. Knights, blue knight and paladin wear it.
* **`tabard`** item (`kind: tabard`): surcoat in the seat colour (`primary` = delve ink 7/8,
  gold trim = 5/a) from shoulders to mid-thigh.
* Brown leather gauntlets + boots under silver greaves, bigger pauldrons and plume, props
  scaled with the body (`weapon_scale`, default 0.72).
* Idle is a guard stance (sword up and forward, arms out) rather than arms dangling.
* The old separately scaled style montage has been removed. `tools/compare_game.py` now
  asserts exact native game pixels; procedural dolls are not a parity reference.
* `library/*.json` describes creative dolls only; `python3 -m spriterig.doll.make_library` validates it.
