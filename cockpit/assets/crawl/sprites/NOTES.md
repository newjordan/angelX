# Crawl hands and props

- `hand_sword_idle` (40x48): Right steel gauntlet gripping a gray longsword tilted up-left, with broad finger plates and a dark mail cuff; blade edge and crossguard remain readable in the dark corridor.
- `hand_sword_raise` (40x48): Same right gauntlet drawn higher and back, sword tilted up-right; the lower cuff is fixed while the fist moves upward.
- `hand_sword_swing` (40x48): Blade cuts down-left with three separated pale-gray trails; the blade point was restored after cuff cleanup, with no white or signal inks.
- `hand_torch_a` (24x40): Left gauntlet holds a wooden shaft and wrapped head, with a tall left-leaning red-and-gold flame; the mail cuff and shaft leave the bottom edge.
- `hand_torch_b` (24x40): Right-leaning flame and a changed gold core; hand, shaft, head, and lower cuff pixels are identical to A.
- `campfire_a` (24x20): Broad pointed flames above crossed brown logs inside a low oval stone ring, seen slightly from above; log diagonals were strengthened after reduction.
- `campfire_b` (24x20): Alternate flame silhouette and hot core over the identical logs and stone ring; only flame pixels change.
- `boulder` (24x18): Asymmetric chipped gray rock with broad upper-lit planes, two dark fractures, and a charcoal underside; no fine grain.
- `lectern` (24x30): A large open parchment tome, dark center gutter and schematic page marks, sloping brown desk, pedestal foot, and a small candle on the right; page marks and candle were repainted to survive reduction.
- `chest` (20x16): Closed brown chest with a curved lid, two broad iron straps, dark lid seam, and square center latch; straps and latch were strengthened after reduction.

All ten source images were generated separately using built-in imagegen and visually inspected. All final PNGs, the complete 6x sheet, enlarged sheet crops, and the hands in all three corridor poses were inspected. The 288x160 corridor context places the torch at `(0,120)` and sword at `(248,112)`. Props are included as additional silhouette checks. The final PNGs use binary alpha: `.` is transparent and every other pixel exactly matches the brief palette.

Sword cuffs share the same four mail rows at `x=25..39`, `y=44..47`; their bottom attachment is identical. The swing blade remains separate from that fixed anchor. Torch and campfire pairs preserve their non-flame substrates. Flames use only `4 @ 5 6 7 a 8`; candle light is restricted to its tiny flame. No non-light sprites use signal inks.

The checked-in tools preserve the originals, exact prompts, placement choices, deterministic quantization, and literal ink retouches. `../tools/SPRITES-README.md` explains rebuilding. `../tools/sprite-validation.json` records passing pixel checks.

The 2x4-dot preview in `../tools/sprite-qa/dots-2x4.png` is a readability approximation, not a render from the crawler's Rust engine. Fine mail links, book marks, and rivets merge at terminal scale; the silhouettes, blade directions, fire, book spread, and chest bands are the intended primary cues.

Rust sources and earlier textures were not changed.

## Brief 3: open chest and wall fixtures

- `chest_open` (20x16): The original chest's lid is raised into a squared, dark inner wooden face above a heap of gold with pale glints and one teal gem. The lower body, center latch, and bottom silhouette at rows 8–15 match `chest` exactly; iron-band columns 4, 5, 14, 15 at rows 1–14 are also identical. Gold uses `4 @ 5 6 $`; the gem uses only `2`.
- `sconce_a` (12x20): Front-on wall fixture with a bottom iron mounting plate, bent projecting bracket and socket, a slightly up-right slanted brown torch, soot-gray head, and a right-leaning flame. Broad gray bracket highlights were restored so the ironwork remains visible against stone.
- `sconce_b` (12x20): Alternate left-leaning tongue and shifted gold core. Exactly 17 flame pixels differ from A; every torch-head, shaft, bracket, socket, and mounting-plate pixel at rows 8–19 is identical. Fire inks occur only in the flame at rows 1–7.
- `banner` (12x24): Hanging iron rod, faded rust-red cloth with broad folds, a pale upward-pointing sword sigil, and three uneven torn lower points. The blade and crossguard were strengthened after reduction. The cloth uses the palette's matte red-brown inks, rather than emissive fire red; no banner pixel uses a signal ink.

All four new imagegen originals were viewed individually. Both versions of the reduced PNGs, their plain-background strips, the complete 6x `sheet-3.png`, and the 2x4-dot approximations were also viewed. The first reduction flattened the gold values, thinned the sword sigil, and lost the sconce bracket against masonry; retained ink retouches fix those issues. The final sheet places `chest` and `chest_open` side by side and shows both sconces and the banner against the unchanged `wall_stone`, including a shared wall vignette.

The final sprites read best through their silhouettes and broad value clusters. Small cloth folds and individual coin glints merge in the dot approximation; that preview is not the crawler's actual Rust render. The banner is muted rust-red within the supplied nonemissive palette. No extra glow or white was introduced.

`../tools/SPRITES-3-README.md` documents rebuilding. The new tools retain source art, exact prompts, crops, literal ink cleanup, source hashes, and passing checks in `sprite-3-validation.json`. All earlier asset files remain byte-identical.
