# sprite-rig: native authoring and separate creative animation

## Native game contract (spriterig/native.py)

The default CLI reads Rust-exported native frames and the canonical version-1
`knight-fit.json`. Real per-frame body art and resolved fit are authoritative;
no approximate body parser or shared creative skeleton participates. Native
ink rows pass through unmodified; static worn assets use exact Rust fitting
primitives. RGBA and manifest parity is asserted by `tools/compare_game.py`.
See README.md for the full coordinate, sampling, sizing, and painting limitations.

Everything below is a **creative, non-game-parity** animation track. It can generate
new motion, resample parts, and clean up procedural pixels; none of those steps
may be applied to the authoritative native workflow. The unused experimental
mesh renderer was removed, including its CLI flags, tests, and curated output.

## 1. Shared creative skeleton (spriterig/skel/skeleton.py)
* Mixamo bone names/hierarchy (prefix `mixamorig:` stripped): Hips, Spine, Spine1, Spine2, Neck, Head, HeadTop_End,
  Left/Right {Shoulder, Arm, ForeArm, Hand, HandIndex1, UpLeg, Leg, Foot, ToeBase, Toe_End} = 27 bones.
* Y-up, facing +Z, character-left = +X, metres; rest = I-pose (arms down), no bone roll. Height 1.71 m.
* A pose = local rotations `[27,3,3]` + hips translation. `fk()` gives globals. Every clip, importer and renderer speaks this.
* Retarget: `G_ours = G_src · Rrest_srcᵀ · arc(our rest dir → src rest dir)` – copies global bone directions,
  so T-pose (Mixamo) vs I-pose sources both work; extra bones (fingers, eyes) are dropped/folded.

## 2. Clips (spriterig/skel/clips.py, fbx.py)
* `Clip(rot[T,27,3,3], hips[T,3], fps, loop)`, `.sample(t)`, npz save/load.
* Importer folder `load_folder(dir)`: **.fbx** (pure-python binary FBX parser: zlib arrays, Pre/Post rotation,
  euler order, AnimationCurve keys, UnitScaleFactor), **.bvh** (read/write, round-trip exact), **.glb** (glTF animations), .npz.
* The 15 procedural delve motions are always present as fallback clips (`procedural_set()`), lifted from the 2D sagittal rig.

## 3. 2D fast track (spriterig/skel/billboard.py) – camera-facing sprites
* A `Billboard` = N authored directional views (hero: TOWARD / ACROSS / AWAY; folk: one side view), each a set of
  ink-sprite parts pinned to bones.
* Per frame: camera (yaw, pitch) + character facing → pick nearest view (mirror for the opposite side) → project the
  3D bone pose into that view's screen plane → rotate/stamp each part (2×2 supersampled, palette-only output, snap to pixel grid).
* Any camera angle: yaw picks the view, pitch foreshortens vertical bone extents. Cost: **~1–1.4 ms/frame** per character in pure numpy.

## 5. Mixamo export settings
Clips: **FBX Binary, Without Skin, 30 fps, Keyframe Reduction none, In Place** (BVH also fine).
Drop animation clips in one folder → `python -m spriterig.skel --clips DIR`. No character mesh is required.

## 6. Creative 2D doll animator (spriterig/doll; non-game parity)
Built for the requests that arrive constantly: *new armor, new weapon attack, new joke emote*.
* **Body** = capsules per bone (thigh/shin/arm/fore…) + torso hull + sphere head (hair, eyes, beard, ears) drawn
  from the shared 27-bone skeleton at any yaw/pitch/facing; painter-sorted by camera depth, outline pass then fill
  pass, so joints never seam. Light-side shading per capsule.
* **Equipment** (`library/equipment.json`, hand-edited JSON, validated by `make_library.py`): items have a `kind` – `limb` (recolour/grow
  segments: gauntlets, greaves), `chest`, `robe` (flared hull, drawn over legs), `helm`/`hat` (sprite or shape on the
  head), `plume`/`cape` (Verlet springs, pushed behind the body), `weapon`/`shield` (pixel sprites on a hand socket,
  auto-flipped, bow string pulled to the other hand). Each item names a `material`; materials map to delve inks, so
  every item is palette-swappable per loadout (`colors`). Later items in the same `slot` replace earlier ones.
* **Loadout** = JSON `{name, body, equip:[ids or inline items], colors:{material:[fill,shade]}}`. Bad ids/keys raise
  a message listing what exists.
* **Clips** = text DSL (`library/*.clip`, see `dsl.py` header): keyframes of per-bone degrees with eases
  (linear/in/out/inout/snap/back), `hit` / `smear` markers, `hold`, `=K` (repeat key K), `hips=`/`bob=`, headers
  `loop`, `tags`, `weapon`, `squash`, `fx zzz`. Limb rotations are swing (exp-map of x,z) + twist (y), so x and z
  combine intuitively. `dsl.blend` (quaternion nlerp) and `dsl.sequence` (cross-faded chains) build combos.
  Mixamo FBX/BVH clips load unchanged (`--mixamo DIR`, resampled to 12 fps, named `mx_<name>`).
* **Juice**: anticipation/follow-through are just keys + eases; smear frames draw a trail quad from the previous
  weapon grip/tip; squash/stretch from hips vertical velocity (about the floor); springs prewarmed so capes settle.
* **Robustness**: final `cleanup` fills 1-px seams (4-neighbour pinholes) and drops orphan pixels (also applied to
  billboard and delve cut-out renders); canvases auto-grow per clip so jumps/spins never clip;
  `python -m spriterig.doll check` flags off-canvas, empty, stray-pixel and popping frames and writes magenta previews.
* **Cost**: 1.3–3.2 ms/frame at 48 px (see showcase/BENCHMARK.md).

## Known gaps
* Folk have only a side view (front cameras reuse it). More authored views = better billboards.
* "Hit Reaction" FBX imports crouched/odd – check the clip or its hips track.
