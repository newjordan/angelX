# Crock and the slag drop

80×80 rival, 24×24 wind-up, 16×24 slag, 32×18 impact. Transparent,
nearest sampled and snapped to sixteen existing realm colours. No new inks.
`python3 scripts/cinder-art.py <crock-source> <slag-strip-source> --review <sheet.png>`
rebuilds the sprites; no smoothing, dithering or per-frame recolouring.
Source paintings and review sheets live in `/work/artifacts/boss-art-20261015/game-1/`.

The first converter crop was rejected: a neighbouring shard in the projectile
third made its bounding box too wide and collapsed the slag silhouette.
Tighter subject crops preserve the original eyebrows, mitten hands and flame.
The final sprites were inspected at three times their game resolution.

Source prompt — rival:

Use case: stylized-concept. Asset type: original pixel game boss sprite. A single squat friendly-menacing kiln spirit named Crock, a round ceramic furnace creature, frontal three-quarter view, entire body centered, chunky readable rounded shapes, two small brick feet, stubby mitten arms, arched furnace mouth with a warm amber core, hood-like teal glazed chimney head, mischievous heavy eyebrows and two amber eyes. Cool blue-green shadows shifting to warm terracotta and brass highlights. Hand-clustered pixel art silhouette, selective coloured contours, limited palette around 16 colours, crisp hard pixel edges, no smoothing, no black outline. Single subject on a perfectly flat neutral mid-grey background #808080, no floor shadow, no scene, no labels, no text, no white highlights, no bloom, no striped detail, no other subjects. Designed to read when reduced to 64 pixels tall.

Source prompt — attack lifecycle:

Use case: stylized-concept. Asset type: original game pixel-art attack VFX strip for a ceramic kiln spirit. Three evenly spaced isolated stages of ONE amber molten clay slag attack arranged horizontally on an absolutely flat neutral mid-grey #808080 background. Left third: wind-up, a small suspended cracked clay pebble wrapped in a tight curling teal smoke crescent, tiny amber cracks. Centre third: falling projectile, a chunky dark terracotta slag lump with one warm amber molten seam and a SHORT two-lobed copper flame rising above it. Right third: impact, a low wide chunky amber and terracotta ceramic splash with five thick rounded shards bursting out to either side, no tall flame. Each stage fits cleanly in its own third with ample grey margin and no overlap. Hand-clustered pixel art, crisp pixel edges, no smooth blending. Limited palette of cool blue-green shadows, copper, terracotta, muted golden highlights, no white. Coloured selective contours. Flat front view, no ground, no shadows, no lettering, no numbers, no border, no scenery, no bloom, no small noisy sparks. Readable at 24 by 24 pixels per stage.

## Send-back states and effects

The original `crock.png` is unchanged. Damage stages begin at 12, 24 and 36
cracks. `flinch-1/2` and `windup-1/2` retain the approved body below the head;
the wind-up lifts the lid and opens its amber vent. Damage is composed over
both action poses once in the renderer's cache. The lab copy is 28×28.
Tell is 56×36, slag 18×24; impact is four 40×24 frames, three ticks each.
Terminal copies open teal shadows once and restrain the amber highlights;
the cell drop uses a broad ceramic shard with one seam instead of braille noise.
Broad eyes and a dark furnace cavity preserve the face through cell averaging.
All colours remain in the existing master palette.

Rebuild from the retained source sheets:

```
python3 scripts/cinder-art.py \
  --states /work/artifacts/boss-art-20261015/game-1b/source \
  --review /work/artifacts/boss-art-20261015/game-1b/states-3x.png
```

`--draft` shows the full variations before compositing. Rejected those full
body replacements: normalising their bounds moved the shoulders and feet.
The retained pass copies only damage seams and action heads, preserving the
original body. Nearest sampling and palette snapping preserve the clusters;
`clean` removes isolated single pixels. Review sheets show at most 12 sprites.

Source edit prompt — damage strip (reference: crock.png):

Use case: precise-object-edit. Asset type: three-stage pixel sprite damage strip. Edit target: the supplied Crock sprite. Make a horizontal strip of exactly THREE equally spaced copies of this SAME figure, each isolated on a perfectly flat neutral mid-gray #808080 background, front three-quarter view at the same scale and baseline. Preserve the rounded body, teal chimney/lid, eyebrows, eyes, arched furnace mouth, mitten arms, brick feet, original teal/amber hue-shifted palette and all proportions. Change ONLY damage: left copy has one thick connected amber crack in its left ceramic shoulder (12 cracks); middle has that crack plus a branching connected crack across its belly and right shoulder (24 cracks); right has the same cracks widened with small separated ceramic plates still attached to the body, exhausted eyebrows and lifted crooked lid (36 cracks). Each full figure within its own third, generous margins, no overlap. Crisp chunky pixel clusters, no isolated pixels, coloured 1-pixel contour, no black outline, no spark speckles, no text, no numerals, no smooth shading, no bloom. Identity and original sprite's character are essential; no redesign.

Source edit prompt — action sheet (reference: crock.png):

Use case: precise-object-edit. Asset type: four-frame pixel sprite action sheet, 2 columns and 2 rows. Edit target is the supplied Crock figure. Make exactly FOUR full-body copies of this SAME figure, same 80x80 sprite proportions and palette and same feet baseline per panel, on flat neutral mid-gray #808080 background. Preserve body shape, arched amber furnace mouth, teal glaze chimney, mitten hands and brick feet and original character exactly; minimal action differences only. Upper left hit flinch frame 1: eyes squeezed shut, shoulders jerk up slightly, head tilted back two pixels, lid jolts. Upper right hit flinch frame 2: shoulders settle, head tilted forward, squint with irritated eyebrows. Lower left slag wind-up frame 1: lid lifts a little off chimney, broad amber vent light in opening, hands braced by belly. Lower right slag wind-up frame 2: lid lifts higher tilted above chimney, opening glowing amber, cheeks puffed and hands braced. Original body and hands should stay almost unchanged; no damage cracks needed. Clean hand clustered pixel art with coloured one-pixel contours, cool teal shadows warm amber highlights, chunky readable shapes. Each copy isolated within own quarter with grey margins, no panels or labels, no text, no floor, no bloom, no isolated sparks, no smoothing, no white. Keep the recognizable identity and size.

Source effects prompt (references: crock.png and previous slag.png):

Use case: stylized-concept. Asset type: pixel game attack lifecycle sheet, two rows of three equal panels on perfectly flat neutral midgray #808080. References are Crock the teal ceramic kiln and its brown slag. Exactly six isolated effects, no character. Top left: LARGE broad teal kiln steam tell, three connected rounded lobes curling up around a molten amber ceramic lump, designed for 56x36 pixels. Top middle: falling slag projectile, one irregular dark terracotta ceramic teardrop with a thick amber diagonal seam and a short two-lobed copper tail, designed for 18x24 pixels. Top right: impact animation frame 1, a compact low molten amber splash, designed for 40x24 pixels. Bottom left: impact frame 2, broad bursting ceramic splash with three large attached rounded outward lobes. Bottom middle: impact frame 3, lower wider spread and two LARGE rounded fragments immediately beside splash, no tiny sparks. Bottom right: impact frame 4, low cooled terracotta puddle with short receding teal smoke lobes. Four impact frames at consistent scale and ground baseline. Thick clean connected shapes, coloured contours, cool teal shadows and warm terracotta/amber ramp, limited 16 colours matching reference. Crisp pixel clusters, absolutely no isolated single pixels, no speckle, no glitter, no thin filigree, no checker dither, no white, no bloom, no black outline. Generous flat grey margin per panel, no overlap, no text or panel borders.

## Second send-back: authored seams and solid cell effects

`python3 scripts/cinder-art.py --refine --review <states-3x.png>` rebuilds this
pass deterministically from `crock.png`. The base, windup-1/2, tell, slag and
impact-1/2/4 files stay byte-identical. At 12 cracks one 3px dark-teal seam
cuts the amber rib; at 24 there are three; at 36 they open to warm cores and
the original lid separates into two plates. No replacement body texture or
dark single-pixel speckle is introduced. Both flinches translate the same
base head by 2px, squint within its eye sockets and jolt the intact lid.
The first upward lid jolt was rejected because it clipped the top rim.

The lid reference is retained at
`/work/artifacts/boss-art-20261015/game-1c/source/lid-reference.png`.
Its broad ceramic split informed the seam; its wider cap was rejected as a
replacement for Crock's narrow lid. Reference prompt:

Single isolated teal glazed ceramic chimney lid, front three-quarter view,
on perfectly flat neutral mid-gray #808080. Squat round oval lid, two broad
attached ceramic plates split by one jagged crack, restrained thick amber
glow visible inside the seam. Chunky readable rounded shapes, selective
dark teal contours, cool shadow and warm light; limited dark blue-green,
muted teal, terracotta, ochre and amber. Crisp clustered pixel art, designed
to reduce to 25 by 12 pixels. No creature, body, debris dots, speckle, dither,
black outlines, white highlights, bloom, text or extra subjects.

Impact-3 is a low splash with angular ceramic plates. Three 24px filled
radial clear flashes replace square outlines. Effects and flying pots use
the solid half-block layer; only atmosphere belongs in braille. Eight
integer path samples arch each cleared pot into Crock, with column-dependent
lift. The first shallow arc was rejected as a slanted three-pot strip. CHAIN is cached at 2x scale and appears at the cleared board row.
Tell retains its silhouette behind Crock; its cell variant uses two open
teal curls with broad negative space instead of cream. Terminal caches
retain dedicated connected 4px seam masks; the first 3px-only cell attempt
was rejected because averaging hid stages 12 and 24. Live terminal seats use
these layers directly, covered by a live-route regression check. The 28px
lab figure has broader eyes/cavity and sits 14px farther left.

Review: `scripts/cinder-review.py <output-dir> before|after` makes 1x room
sheets and HTML from the actual ANSI half-block/braille buffer. Each sheet
contains at most 12 views and remains below 1800px wide. Native room shots
and terminal buffers come from the two ignored Cinder review writers.

## Carried polish

RGB seams at stages 12 and 24 retain their 3px deepest-teal width. The
terminal stage-36 mask now has a 1px amber core inside a 5px deepest-teal
stroke (2px dark on either side). Rival plots have 2px gutters and seat
baselines. The lab copy moves another 10px left and 9px down; two hand
placed warm eye pixels and a dark mouth preserve its face. Clear-2 fills
the 24px canvas. The health fill moves inside its existing bar below the
Crock label, and the page volume control moves to the bottom corner.
