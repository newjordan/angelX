# Mossbell art

Mossbell is an original bronze seed-pod keeper, with leaf antlers, root arms and
one curved visor plus an amber seed light. Its table sits in the bathhouse's
conservatory bay. The copper, teal and leaf forms have no borrowed characters,
table layouts, names or music.

[`scripts/mossbell-art.py`](../../../../scripts/mossbell-art.py) converts a three by
three keeper sheet and a four by three effects sheet using a shared nearest
scale, <=16 inks, isolated-ink cleanup and a coloured contour. Curved visor and
seed lights are restored after cleanup. There is no smoothing filter. The room,
three bumper shapes, ball, chain pops, damage overlays and knight arm poses bake
into OnceLock caches. The terminal uses the same solid impacts and table;
only steam goes through the braille layer. Broad dark seams with warm cores
are baked for cell sampling; signature lights are restored afterwards. The master ink palette is unchanged.

All nine poses are 48 x 48 pixels: idle, root tell/attack, crown tell/attack,
flinch, two broad shell damage stages and collapse. Each effect has four 32px
frames: bumper ring, shell petals and jackpot bloom. Active poses retain damage
seams through cached masks. Physical anchors are shared with integer collision
geometry, including the exact root band and crown gate.

Root tells are four 184 x 11 native sprites. Connected tendrils extend to
46, 92, 138 and 184 pixels, carrying their amber bud with them; both directions
bake once. Crown tells are four 51 x 24 sprites, with broad hooked jaws reaching
6, 12, 18 and 24 rows into the original gate. Neither uses a progress rail or
base ring. `native.json` retains their cleaned four-ink grids. Both use the
existing 36-tick normal / 30-tick rage progression, without simulation changes.
The converter accepts `--root-source` and `--crown-source`; the review script
requires increasing authored area and actual cell changes beyond each previous
front. Hood flecks are removed, and damage-1 gains a second shorter fracture.

The first crown source was rejected at native room size: thin branches sank
into the Keeper. The broader replacement preserves an open chest while adding
large amber hooks and teal jaw planes. Retained sources and full prompts live
in `/work/artifacts/boss-art-20261015/game-3c/source/`: `root.png`,
`crown-broad.png`, `root-prompt.txt` and `crown-broad-prompt.txt`.

Rejected the first palette conversion's tiny bronze flecks, then a second cleanup
that flattened the face. The retained pass preserves copper facets, restores a
curved visor and shaped chest seed, and uses broad dark shell splits. Rejected
the initial launch path into the return rail and instantaneous paddle rotation;
the lane clears its bank and the paddle now collides across its swept wedge.

Source prompts (retained sheets in the task review directory's source/):

Keeper: one 3 by 3 contact sheet on flat neutral grey, no text/gridlines. Same
original creature, same scale and palette: squat bronze seed-pod keeper, rounded
copper shell, two teal leaf antlers, short root arms, no legs, split acorn mouth,
one curved amber chest light. Front three-quarter, finished 48px pixel sprite,
chunky clusters, 4-tone hue-shifted ramps, 1px coloured contour, <=16 inks, cool
teal shadows, warm ochre light, no black outline, dithering, speckles or white.
Row 1 idle, raised-root windup, root extended sideways; row 2 crown spread open,
crown closed down, leaning hit flinch; row 3 broad dark shell split, broken crown
and two splits, collapsed folded-leaf seed. Equal cells/margins, distinct tiny
silhouettes, no scenery/shadows/props, commercial likenesses, logos or letters.

Effects: one 4 by 3 sheet on flat neutral grey, no text. Centered finished 32px
pixel effects with equal margins, four-tone copper/ochre/amber/teal ramps,
<=16 inks, coloured outline, no white/dithering/speckles/dots. Row 1 rounded
bumper ring from compressed oval to donut to fading thick arc; row 2 connected
four-petal leaf impact grows then folds; row 3 six-petal seed jackpot expands
into curling wreath. Clean negative centers, chunky solid silhouettes, 24–32px,
no rays, sparkles, characters or scenery.
