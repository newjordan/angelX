# Creative-track benchmarks (not game-pixel parity)

These historical timings do not measure native game parity. See ../README.md for native export and comparison.

All 15 motions (176 frames) per character, scale 1, `--no-frames`, single thread,
NumPy + Pillow only. Machine: Intel(R) Xeon(R) Processor, Python 3.13.5.
`aa` = 2×2 supersampled bilinear, premultiplied alpha; `nearest` = point sampling (pixel art).

| character | sprite px | mode | tris | rig ms | ms/frame | frames | render s | gif s | total s |
|---|---|---|---|---|---|---|---|---|---|
| red_knight | 116×245 | aa | 1670 | 57 | 46.6 | 176 | 9.58 | 1.07 | 10.66 |
| blue_knight | 116×242 | aa | 1612 | 55 | 47.5 | 176 | 9.93 | 1.31 | 11.25 |
| kimi_robe | 104×200 | nearest | 1016 | 31 | 8.4 | 176 | 2.15 | 0.83 | 2.99 |
| loom_warden | 132×200 | nearest | 974 | 30 | 7.9 | 176 | 2.01 | 0.69 | 2.71 |
| synthetic_knight | 96×144 | aa | 442 | 41 | 13.7 | 176 | 2.81 | 0.43 | 3.24 |

`rig ms` = segmentation + mesh + skin weights. `render s` = posing + rasterizing + cropping all
frames. `gif s` = palette + writing 15 per-motion GIFs and the montage.
Same sprite in `aa` for comparison: loom_warden 28.1 ms/frame (vs 7.9 nearest).

## Delve mode (v3), box CPU, numpy+Pillow
```
hero_red     16x16  rig 1.5 ms  1.89 ms/frame  176 frames  render 0.33 s  with export 1.30 s
hero_blue    16x16  rig 0.9 ms  1.56 ms/frame  176 frames  render 0.28 s  with export 1.20 s
hero_green   16x16  rig 1.0 ms  1.83 ms/frame  176 frames  render 0.32 s  with export 1.26 s
hero_gold    16x16  rig 1.0 ms  1.58 ms/frame  176 frames  render 0.28 s  with export 1.24 s
brannoc      22x20  rig 1.1 ms  2.54 ms/frame  176 frames  render 0.45 s  with export 1.84 s
guard        12x13  rig 0.6 ms  0.91 ms/frame  176 frames  render 0.16 s  with export 0.75 s
miner        12x12  rig 0.6 ms  1.05 ms/frame  176 frames  render 0.18 s  with export 0.74 s
smith        12x12  rig 0.6 ms  0.86 ms/frame  176 frames  render 0.15 s  with export 0.65 s
goblin       14x12  rig 0.8 ms  1.02 ms/frame  176 frames  render 0.18 s  with export 0.78 s
```

## skel (creative 2D billboards), box CPU, pure numpy
| track | asset | cost |
|---|---|---|
| 2D | hero (3 views, 16×16) | 1.2 ms/frame |
| 2D | brannoc / guard / goblin | 1.4 / 1.0 / 1.2 ms/frame |
| import | 9 Mixamo FBX clips | 1.0 s total |

## dances (21 Mixamo FBX clips)
Historical creative 2D timing: import 21 FBX 4.3 s; 1.34 ms per character-frame.
The unused mesh-rendering track and its outputs have been removed.

## 2D doll animator (v5), box CPU, numpy only, `python3 -m spriterig.doll bench`
383 frames per loadout (attacks + emotes, mixed side/front/3q/back), render only:
```
peasant           383 frames  1.29 ms/frame  (48px)
knight            383 frames  3.12 ms/frame  (48px)
mage              383 frames  2.08 ms/frame  (48px)
dwarf_berserker   383 frames  1.73 ms/frame  (40px)
```
Batch export incl. PNG/JSON/GIF IO: ~4.9 ms/frame (20 sheets, 928 frames incl. 3 Mixamo clips, 4.5 s).
