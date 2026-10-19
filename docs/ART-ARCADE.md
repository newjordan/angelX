# Arcade drawing contract

Applies to every mini-game, character, boss, prop and effect, including top-down
Delve and both art kits. Original 1990s hand-drawn arcade principles: painted
forms, strong ink, expressive cartoon weight. Never trace, rip or closely copy a
commercial frame, character, vehicle, name, logo, costume or UI. Reference the
craft, not a franchise design. The knight pilot is an open-faced bascinet knight
with a raised brow,
small swept plume, patched oxblood coat, gorget, plated boots, heater shield and
a broad sword. Its medieval equipment and pose construction are original.

## Drawing at the displayed grain

Use a 384×224 native stage; a standing side-view character is about 43–48 pixels
including its plume (40–60 is the family range). Do not enlarge a small drawing
to imply detail. The pilot uses a fixed 64×56 transparent frame and foot anchor
[30,51]; that is an asset-space registration point, not a new collision box.
The fixed standing stack is plume 6px + helmet/face 13px + torso/gorget 14px +
legs/boots 13px = 46px. The model sheet is a TURNAROUND of construction, not a pose library. Match its
volumes, even contour, ramps, face kit and seat scheme exactly; exaggerate poses
and timing. A tidy standing chibi repeated in every state is a send-back. The
standing body stays 46px; acting may crouch to 40px or stretch to 52px inside the
same canvas. Keep registration; never enlarge the sprite to fake acting.
Keep game footprints, danger geometry, attack timing and anchors during adoption.
Top-down art retains its actual renderer density and projection.

* Outer contour: every 4-neighbour exposed opaque sample is indigo #22303a or
  warm grey-brown #322b2b. Use a 1px line on ordinary curves, 2px at deliberate
  overlaps. Contour is ink, never a leg, rear arm or shadow fill. Contour plus
  near-black must cover <=25% of the knight's opaque mass; no solid 3×3 contour
  block. No untagged 1px silhouette spur (solid sample with <=1 solid neighbour)
  or notch (transparent sample with >=3 solid neighbours). Only named, exposed,
  connected 1–3px sword/plume tips, shield points or boot joints can be tagged;
  the pilot uses none. Do not tag a ragged edge away.
* Internal contours: separate brow, shoulder, hand/weapon, coat/belt and boots
  with 1–2px lines. The darkest interior pigment must be >=12 integer RGB luma
  above #22303a. Shadow steel starts at #2f4a52; never #192934 in this knight.
  Drop #0a0c12 everywhere except the tagged 2×3px visor/eye patch. Exposed contour
  is resolved after layering the authored cutouts; buried construction edges
  use the part's actual shadow material, rather than accumulating dark ink.
* Major materials: keep 5–7 actual tones, with at least three tones each covering
  >=10% of their visible material region and no tone covering >35%. Count the
  helmet and coat separately using authored part masks, never a palette-wide
  box combining chest, sleeve, skirt and shield. The two lightest cloth tones
  may be short glint/rim strokes: requiring four samples of every glint would
  conflict with their area limits. Other untagged pigment islands still need
  >=4 connected samples; the three broad bands remain mandatory. Tone presence
  is not drawing quality. Draw cool form shadows, warm bounce, curved cel bands
  and small steel glints on both helmet and blade; no dither or noisy speckle.
* Proportions: head/helmet about 28–38% of standing body height, broad shoulders,
  an 8×6px pauldron, 5×5px steel fist inside its contour, 7–8px boots with 2px
  soles. Squat trunk, large hands/feet, clear elbows/knees, open negative space
  and asymmetrical weight. Head leads hips by 3–4px in run and attack. This
  knight's signature is its dented, bandaged helmet and repaired brass-trimmed
  steel gauntlet. Brass on the fist is only a two-pixel cuff stroke.
* Face: a raised visor brim frames a 7×6px opening with an angled 1px brow,
  a fully dark 2×3px eye (six pupil samples, no white eye dot), a 4–5px dark
  moustache bar and a jaw contour. Neutral, gritted attack, open shout on hit,
  and victory grin must read as different expressions. The gritted/grinning jaw
  opens one row, the shouting jaw two rows; its dark brown mouth is separate
  from the eye. Preserve
  >=2px dark eye and moustache marks in the real cell reconstruction. No white
  eye dot or tan profile block. Gear must leave the face kit readable.
* Props: a sword stays in hand in every non-defeat frame: 14–18px blade, three
  tones plus a glint. The shield is >=10×12px with a rim and seat-coloured field.
  Draw a visible hand on the grip, gorget between head and torso, steel legs and
  boots. A sword touching an arm at mid-blade is a drawing error even if tagged.
* Grit: at least three structural cues (baldric/buckle, sewn patch, dent/rivet,
  plated cuff, scuffed boot). Draw marks as seams, overlaps and tone shapes.
  No noise or isolated 1–3px pigment/alpha islands. Named straight 2–3px
  same-pigment eye, seam or rivet strokes are the only tiny-cluster exceptions; tags never waive
  palette, contour or checker rules. A singleton never qualifies. Mixed-colour or occluded tags
  never exempt a cluster.

## Colour and darkness

Black terminal space stays black. Rooms use cool deep shadow and localized warm
light; retain the existing `grade_actor`, surface grade and half-block/braille
painter. No white cores: fail any ink with min RGB >=198 and max RGB >=220.
Avoid bright areas dominating a dark room; highlights describe a surface and
occupy a small fraction of it. Saturation belongs to a few identifying accents.

The oxblood coat is dominated by #61303b and #853d43; #ad4e49 is its top broad
band. #cc6854 covers <=10% of a material region as rim, #dc8768 <=3% as glints.
Do not use the dark #38212d as interior fill in this model. The largest connected
bright patch must be steel (#8fa3a8 / #b3c2c4), larger than the cloth rim/glint
patch (#cc6854 / #dc8768). Cloth must not become a flat salmon spotlight.

The shared 75-entry world palette is unchanged. Its first 60 material entries
already supply six-step steel/leather and blue/green/gold seat ramps. Its final
15 entries carry signals and may not be borrowed for decoration. It has no
five-step red material ramp: its lone saturated red is a signal. The pilot adds
exactly eight local pigments in `assets/arcade/palette.json`: six cloth reds and
two orange embers. Skin reuses timber inks; gauntlets use steel, with a two-pixel
brass cuff. The eye is dark pupil ink; steel glints stay on the helmet and blade.
The build validator pins that exact bank and checks every authored RGB against
shared material inks + this bank. It cannot silently expand. Palette swaps change only plume, tabard and
shield-field pigments; geometry, steel and leather are identical in four seats.
The seats are oxblood, deep navy, olive and jade. Navy's broad steps are
#12516a / #2f4a52 / #456070; olive uses #335022 / #35543f / #3f6428;
jade uses #12516a / #35543f / #3a6d5d. The last two steps remain short rims and
glints. The former blue melted into steel and orange merged with the warm hand.
Changing those mappings, then making the hand steel, fixes both without adding
pigments. Coat/steel delta-L must be >=15 before grade; L means the contract's
integer RGB luma (299R+587G+114B)/1000, not CIE L*. Measure visible pigments,
exclude contour, and test every frame/seat both for the swap fields together
and the actual authored coat mask. A plume/shield average must not hide a bad coat. Chromatic coat/gauntlet-trim hue
separation is >=25 degrees; neutral steel hands use the value separation.
The >=12-luma interior floor still applies after every mapping. The unused
lowest cloth index remains reserved. No new pigment or shared-palette expansion. The bandage uses the pinned
shared structure ramp #7a7c72 / #94968c / #b0b2a8 as a local material; the exact
eight-pigment extension stays unchanged.
Palette membership applies before lighting; the unchanged grade produces RGB.
Measured evidence: in the broad red hue sector (328–360° or 0–22°, S>=0.3,
V>=0.25), only three shared material inks qualify, all dark warm browns.
Nearest-colour mapping sends all six cloth targets to neutral/timber inks,
turning the red seat brown. `palette-evidence.png` shows that loss against the
local ramp; steel and the other three seats reuse existing material inks.

## Animation and weight

| State | Minimum drawings | Required motion |
| --- | ---: | --- |
| idle | 4 | breath, subtle weight shift, delayed plume/shield; distinct from brace |
| run | 6–8 | contact/down/pass/up, 10–15 degree lean, 2–3px bob, 12–14px contact spread, shield counter-swing, plume trails 2–3px with one-drawing lag |
| jump/fall | 3 total | lift then tuck 5px shorter; articulate falling legs |
| land | 2 | 4px wider boot stance and 4px shorter body, then recover |
| attack/shoot | 5–6 | crouch 3px, visible over-shoulder grip, 3-tone >=24px smear through 120–160 degrees, contact with front foot 6px ahead, blade low behind, recover |
| hit | 2 | body recoils 3–4px, head thrown back, open shout; no palette-flash substitute |
| guard | 2 | shield at face height, 2px crouch, brace behind it |
| defeat | 4+ | knock back 8–12px, flop plume, back landing with boots up, one-drawing bounce then settled hold (the pilot uses five drawings) |
| victory | 3 | sword straight overhead, tip >=10px above plume, grin and off-hand fist pump |

Adjacent drawings must change >=4 native samples and >=2% of opaque mass. Whole
sprite translation alone is insufficient: articulate limbs, head, plume and
held props. Preserve the foot anchor. No mechanically mirrored running legs or
flashing idle palettes. Idle, guard, hit-0, attack contact and victory-2 have
pairwise silhouette IoU <=0.70. Run helmet centre must lead the authored hip
by >=3px; inspect the actual lean as well as the number.

Helmet bounding-box area <=10% spread and run bob <=2px remain construction
checks on NON-ACTING frames. Explicit squash/stretch/lean tags exempt acting
frames from those two sameness checks; no other floor is exempted. Eye/jaw,
actual part masks, steel glints, sword and shield remain required. Named
2–3px jaw, lid, cheek, cuff and hem strokes cannot excuse mixed pigments or
singletons. Cloth uses broad cel folds, never complementary repeated 4×4
cloth diamonds/checkers. The four expressions are restored from native face
rows above overlapping gear so cutout contacts cannot smear the face away.
The pupil occupies three rows and the moustache sits on row 11 of the helmet:
keep them in separate four-pixel half-block bands as far as the pose phase allows.
An earlier four-pixel pupil and row-10 moustache passed RGB gates but merged
into a smudge in cells; native review alone missed that failure.

Pilot hero anticipation is 220ms; boss danger wind-ups stay >=750ms and must
mark the exact existing landing area. Never change pressure or bullet density
as an art fix. Non-looping state playback holds the final frame.

## VFX recipes

| Family | Minimum | Construction and timing |
| --- | ---: | --- |
| explosion | 10 (pilot 12) | compact hot flash, growing irregular lobes, peak, smoke takeover, rising billows, curled residue |
| fireball | 6 | 2 charge, 2 travelling lobe, 2 breaking; squash before expansion |
| smoke | 6 | overlapping outlined lobes; warm undersides cool as they rise |
| debris | 4 | chunky outlined slivers tumble in arcs, then settle; no pin dots |
| sparks / hit spark | 3 | directional short strokes, extend, break, disappear |
| muzzle flash | 3 | onset, extension, recession; direction matches weapon |
| impact star | 4 | compact strike, asymmetric expansion, cooling, soot curl |
| dust puff | 5 | compressed puff, wide lobes, breakup, separated billows, last curl |

Fireballs/smoke/debris are also construction layers of the explosion; standalone
fireball/debris libraries are future assets. The pilot ships explosion, muzzle,
impact, dust and hit spark. At the explosion peak, the connected
#e2cb8b / #d8a95e hot core is >=40px inside
five fire tones (>=4px each), under the unchanged near-white limit. Debris has
2–3px irregular warm ember faces inside an indigo outline, tumbling on explicit
arcs; no dark 2×2 squares or floating dots. A hot plane opens into its lobe,
never a concentric bright-core ring. Impact/spark rays have two warm-lit pixels
inside their contour. Cooling impact ends in overlapping outlined puff lobes,
never a tadpole squiggle. Do not substitute one-pixel hairlines. Smoke's warm underside cools into grey crowns
and the #22303a outline. Hot cores are timber honey, not ivory. Thick dark
edges and internal lobe overlaps survive native size. Put dangerous solid cores
in the base; peripheral soot/air belongs to the existing braille atmosphere.
A bright star in every frame is not an explosion. No full-screen wash or hazard
shape that implies a larger hit box. Transparent padding is not gameplay area.

## Props and environments

Paint layered depth: dark structural silhouette, mid-plane material bands,
limited warm fixtures, foreground occlusion. Give a crate corners, grain-shaped
planes and hardware; sandbags sag and overlap; rust follows joints and dents;
banners tear and fold. At least three meaningful material/form cues per large
prop, never random speckle. Keep low-contrast scenery behind actor silhouettes.
Draw at the real projection, including overhead shoulders, boots and held gear.

## Gates and review

`cargo build` for Delve runs `arcade_gate.rs`; failures are build failures.
The gate checks the exact local bank, binary alpha, all exposed contours,
pigment islands, repeated checker tiles, actual material tones, broad-band area,
interior luma, contour mass/width, silhouette notches/spurs, cloth highlight area,
steel glints, four-expression face and held props, scoped helmet consistency
and run bob, pose IoU, forward lean, ordered cloth dither, seat contrast/hue,
overhead victory payoff, FX peak core, frame counts/timing and adjacent differences. Real-cell hashes
must differ too. These gates are a floor, never approval: review the drawing by
eye at 1×, then at 3× and through real cells. A passing flat, stiff or faceless
knight is still a failed drawing.

Regression fixtures retain the rejected pilot's pixels and atlas. Its opaque
contour mass, dark interior, notches/spurs, bright cloth and missing semantic
parts must fail. The true rejected chest plane [26,26,39,35] has 44 of 87 cloth
samples in the lightest ink (51%); the old coarse material box hid that flat
plane among dark sleeves and skirt. Test it directly, without modifying pixels.
Independent mutations also break broad-band distribution, contour fill, luma,
notches, spurs, helmet size, run bob, either prop, victory tip and the hot core.
The second rejected stiff model is retained as native atlas/manifest/palette
fixtures: it fails pose diversity, lean, ordered cloth diamonds and seat hue/
contrast. Independent controls copy an idle silhouette into guard, remove lean,
insert cloth diamonds and merge the seat with steel/brass. Exemption controls
prove acting tags scope only the box/bob checks; the non-acting limits remain.
Seat tests compare every source pixel/frame against all four graded swaps.

The standalone exporter compiles the exact cockpit painter and review test in
a small isolated crate, avoiding the full application/browser build. The stage uses the game's actor grade and the cockpit's real
`paint_cells`; its exporter records all frames and all seats at 96×28 cells,
including foreground/background RGB and glyphs. The cell gate requires at least
one lit cell per effect, >=8 per knight, and >=55% of sampled silhouette sites
above the floor by 12 RGB luma units. Every knight view also needs >=2
reconstructed native samples of both eye and moustache at least 12 luma darker
than its graded skin. The pilot retains >=3 of each across all frames/seats.
Bright teeth can trigger the painter's core protection and erase the moustache;
use muted teeth, never a white face glint. The 64×80 review crop includes all
56 asset rows and the floor; no shield tip is clipped. Saved evidence is tied to source frame and
renderer fingerprints at build time; a changed source requires a new export.

Review checklist, in order:

1. Compare the fixed model, old pilot and original Latch knight at 1×:
   recognisable face, hand/foot weight, readable facing,
   strong overlaps, tactile gear and original design; reject flat block bodies.
2. At 3× inspect contour breaks, tone steps, clusters, seams, glints and artifacts.
3. Watch every looping GIF and the dark-stage harness. Check weight, anticipation,
   registration, cloth/plume follow-through and readable defeat; no frame crops.
4. Inspect actual half-block/braille cells, every frame of every seat and all FX.
   A native
   drawing that disappears or becomes a bright blob in the buffer fails.
5. Inspect on a dark room, in motion, beside existing effects; no hypercontrast.
6. Keep the review pack to twelve images, each under 1800px wide. Record rejected
   attempts and reasons, gate results, smoke cost and review paths in REPORT.md.

What the old Atari-like shorthand looks like here: 2–3-colour flat blocks, a
helmet with no face, narrow stick limbs, one-pixel decoration, no continuous ink,
symmetrical stock poses and a two-stamp gait. Avoid it through drawn volumes,
material bands, asymmetry, meaningful seams and actual action drawings. Some
existing art (Crock/Clasp, Rafterwake poses) already exceeds that shorthand; keep
its personality while adding richer ink and weight.

Adopt first in side-view mini-games (Rafterwake, Brasswake, Latch), then the
Mossbell overhead body, then shared top-down v1/v2 heroes. Redraw at each actual
projection/density; never nearest-enlarge this side-view sheet into top-down.
Next do bosses by family, carrying a body plus anticipation/projectile/area/
impact kit together. Preserve current hit boxes, anchors, silhouettes of hazards,
lighting and rage ramps. Validate both kits and the real cells after each family.

Knight authoring is exclusively native pixel rows in `scripts/arcade-knight.py` and fixed contact/fold rows in
`scripts/arcade_knight_contacts.py`,
plus explicit contact/crease drawings. No knight source PNG is opened, sampled,
traced, palette-fitted, smoothed or filtered. `source/knight.png` is retained as
rejected historical reference only. The existing effects school still uses its
retained reference, with native core/debris/smoke additions in
`scripts/arcade-fx-polish.py`. Neither path changes gameplay or renderer density.

Rebuild and review commands (always set TMPDIR=/work/tmp and
ANGEL_CARTRIDGES=/work/tmp/empty-cartridges):

```
python scripts/arcade-art.py build
CARGO_TARGET_DIR=/work/build-cache/bossart-cockpit ANGEL_ARCADE_SHOTS=<output> \
  cargo test --manifest-path scripts/arcade-review/Cargo.toml \
  write_arcade_review -- --ignored --nocapture
python scripts/arcade-art.py review --output <output>
python scripts/arcade-cell-review.py --output <output>
CARGO_TARGET_DIR=/work/build-cache/bossart-crates cargo build --manifest-path crates/Cargo.toml -p angel-delve
CARGO_TARGET_DIR=/work/build-cache/bossart-crates cargo run --release \
  --manifest-path crates/Cargo.toml -p angel-delve-smoke --bin arcade-review
```

`arcade-review` is an explicit evidence-regeneration feature: native art gates
still run, while stale evidence is permitted so the cockpit exporter can write
fresh buffers. Default builds require that new proof and all source fingerprints
to pass. Do not use the regeneration feature for a release. `review.html` is a
standalone looping player with state/seat selection, 1× stage, 3× crop and actual
cells; it opens without a server. GIFs deliberately repeat one-shot states for
review, while the stage API holds their last frame during normal playback.
