# Attributions

All code in this directory was written from scratch. No third-party code was copied.

## Ideas / inspiration (no code taken)
- **See-through** (shitagaki-lab, https://github.com/shitagaki-lab/see-through) — Apache-2.0.
  Idea of splitting a single character image into layered body parts.
- **PSD2Live** (tsunehimatoi, https://github.com/tsunehimatoi/psd2live) — GPL-3.0.
  Inspiration only (auto-rigging a layered character with idle motions). No code read into or copied.
- **rig-c** (https://rig-c.org) — code MIT (per rig-c.org/docs/licensing); its sample assets are
  CC BY-NC 4.0 and none are used here. Idea of a CPU-only painting-to-rig pipeline.
- **Meta Animated Drawings** (https://github.com/facebookresearch/AnimatedDrawings) — MIT.
  Idea of segmenting a figure, attaching a skeleton, and mesh-deforming it with procedural motion.
- Linear blend skinning and barycentric triangle rasterization are standard textbook techniques.

## Libraries used at runtime
- **NumPy** — BSD-3-Clause.
- **Pillow** — MIT-CMU (HPND) license.
Python standard library (argparse, json, time, os) — PSF License.

## Motion data
All 15 motions are procedural or hand-keyed in `spriterig/anim.py`. No motion-capture data,
no downloaded animation files, no models.

## Sample sprites (Frosty's own art, from newjordan/angelX-private)
- `samples/frosty/red_knight.png`: `cockpit/assets/knights/cast-v1.png`, crop x1060-1190 y355-612
- `samples/frosty/blue_knight.png`: `cockpit/assets/knights/cast-v1.png`, crop x1070-1195 y955-1196
- `samples/frosty/kimi_robe.png`: `cockpit/assets/realm/doppelgangers/sprites/kimi.png`
- `samples/frosty/loom_warden.png`: `cockpit/assets/realm/avatars/pools/smithy/sprite/nemotron-loom-warden.png`

## Delve mode
- Character art and palette: angelX `crates/delve` (Frosty's own project). Parsed at run time and not redistributed, apart from the rendered showcase.
- The braille 2x4 cell layout follows the Unicode braille block in the style of Dotmax. No code was copied.
- The majority-vote resampling is inspired by the RotSprite idea of keeping palette-pure pixels. This is an independent implementation in numpy.
- Mixamo FBX clips/characters are user-supplied (Frosty's Mixamo account), not redistributed. FBX parsing follows the public binary layout notes (Blender io_scene_fbx).
