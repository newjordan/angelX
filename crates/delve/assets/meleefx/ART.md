# Melee strips

Hand-drawn weapon swings, baked from [pixelfx](https://github.com/patch-gg/pixelfx)
(`melee/sword-slash`, `heavy-cleave`, `thrust`, `hammer-smash`, `uppercut`, and
`impacts/hit-spark`) and snapped to the Delve's inks by `scripts/delve/meleefx.mjs`.

pixelfx is MIT, Copyright (c) 2026 patch-gg. The sword-slash crescent recipe in
pixelfx is adapted from Sprite Fusion's open-source slash
(<https://www.spritefusion.com/pixel-art-vfx/slash>), MIT, Copyright (c) 2026
Hugo Duprez.

The strips layer over the code-drawn swing. They are not scaled or rotated at
draw time: eight aims are baked, and a stamp never draws past the swing's reach.
Unwished weapons keep the code drawing alone.
