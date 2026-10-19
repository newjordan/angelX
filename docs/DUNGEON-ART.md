# Delivered dungeon art

The dungeon kit is integrated from read-only art-production deliveries. Every
slot map pins the sheet, crop metadata and notes by SHA-256. Rich sources stay
external; native PNGs, registration maps and conversion records are committed.
No rules, collision, spawns, timings, seat camera or default kit are changed.

## Floor plates

All seven destinations use full-bleed illustrations, independently BOX sampled
at 384x224 and 768x448. A centre crop preserves aspect without stretching or
upscaling. The original bottom cartouche, title/depth wording, palette and
54-display-step iris remain. Playback is seat-local and tick-only. Static
illustrations were chosen over artificial dust or resampling during playback.
The procedural scenes remain the exact fallback when a slot is absent.

The art judge's 2026-10-15 verdict accepts six plates directly and requests
Keep saturation -20%, value -15%; that offline grade is in Keep's map. Plate
sheet-edge contact is intentional full bleed, so the importer exempts only the
new `plate` role from the isolated-subject edge warning. Missing crop metadata
is explicitly recorded as inferred opaque image bounds. Other gates stand.

The importer additions are optional request manifests, contain/cover sampling,
map-controlled inset/grade and embedding both native kits. Existing actor
maps and embedding retain their defaults. Reproduce a slot with:

```
python3 scripts/slot-art-import.py dgn.plate.crypt --requests crates/delve/assets/art-slots/dgn.requests.json --check
```

Legacy plate tests still run against the exact procedural fallback. A new
contract checks all nine kind/depth destinations at both grains, exact title
pixels, deterministic held illustration, palette membership and independently
sampled v2 detail. The old requirements for dust motion, two illustrations for
Crypt/Mines and black outer margins apply to the fallback; the delivered kit
intentionally supplies one full-bleed illustration per Kind. No goldens or
palette definitions are relaxed.

## Outstanding chrome

F/SPC/R/X action plates and HUD button chrome are outside this kit and retain
their existing rendering. The art team still needs empty panel/button skins
registered to the existing labels, dimensions, anchors and four-seat HUD.

## Room integration

Both grains use all six packs' floors, blocking props, hazard edges/corners,
doors/stairs and three-frame decor flames. The default stays v1. A solid prop
is placed only on a Block or solid perimeter cell; floor continues underneath.
Blocks sort with v1 bodies by feet y; v2 has a per-body depth mask. The existing
furniture, traps, chests, seals and gameplay cues retain their rules-owned art.
Lights Out, side-on rooms and onboarding retain their existing paths.

PNG banks decode once. Composed scenery is keyed by layout/seed/pack/barred,
independent of ticks/camera. Source glow adds a restrained material lift using
a cached palette lookup. Three flame frames advance every six ticks; sparse
fixture pixels have a separate cache. No live resampling or new pixel floats.

The late dgn.owall wave supplies all six packs' north faces, corners, vertical
cap samples and south caps. Old wall cells 5/6/9/10 turn diagonally, and Mines/
Fungal cells 7/8 are diagonal runs. Old gate cell 4 and row 2 are angled; Keep/
Archive/Unknown stairs are diagonal. None are warped or rotated into the grid.
Their native banks remain staged for provenance; rejected pieces are inactive.

Door replacements remain partial despite the judge's late verified-OK update:
Mines metadata has 5 components, Keep 9, Fungal 9, instead of 12. These three
need separated measured poses. Crypt/Archive side doors are horizontal strips;
Unknown open side doors become solid-looking cap strips at native fit. Those
four states per pack are rejected after native review. Side doors retain the
original bars while fought and a visibly clear passage afterward. Accepted
Crypt/Archive/Unknown north/south doors and stairs use the orthogonal sheets;
other packs use the accepted old north door/axis-aligned stair subset or the
original stair fallback. North half-open poses are staged: the rules expose
only barred/open, with no authoritative opening age. No timer was invented.

Exact next art request: dgn.odoor.<all six packs> west/east barred/open must be
VERTICAL 1x2-tile strips, showing a transparent middle passage when open, one
measured crop per complete state, matched pivots, no floor pedestal/perspective.
Mines/Keep/Fungal also need twelve separated north/south/stair states. Native
side envelope 16x32 / 32x64; north 32x24 / 64x48; south 32x16 / 64x32. A shared
presentation-age contract is needed before a deterministic opening animation.

## Cleanup and validation

Floor bevel inset: 4.5%, Mines/Fungal 7%. Unknown crops contain faint-alpha
padding, so alpha>=128 content bounds are trimmed before the inset. Native
floor fills alone use a 3x3 RGB median before palette reduction to remove
isolated specks; alpha, entities, props, contours and hazard edges are intact.
Mines value .50/saturation .55; Fungal further quieted to .62/.60; Archive value
.60. Other grades live in the maps. Keep lava cracks use seeded 1/23 selection,
under 10%; ordinary Keep floor uses plain/worn/patched. Eye/rich tiles are rare.

Rejected trials: loud Fungal floor, excessive Keep cracks, Unknown horizontal
alpha seams and black beneath narrow props. Fixed all four. A first plate cell
proof averaged native title pixels; final proof uses the actual 96x56 half-block
route and its existing small title font. No dithering or new palette inks.

No room/plate golden hashes existed to regenerate. The Mines v2 test now
requires the new runtime frame instead of the prior v1 fallback. Added checks
cover both grains/all packs, filled floor edges, all 256 hazard neighborhoods,
collision-only obstacle placement, depth masks and four-seat render immutability.
The importer's source-copy provenance warnings remain visible: production notes
record padding/resizing, and the judge reviewed those same delivered copies.

The first full crate run found two legacy-only assertions: >50% black empty
room paper, and open-door black-pixel counts. Both remain unchanged on the
procedural fallback. Delivered-room replacements require opaque quiet floor
tiles (mean luma <65/255), unchanged collision, and changed artwork inside each
of all four doorway rectangles at both grains. This intentionally replaces the
empty-void presentation contract; it does not relax palette or collision checks.

Final live-path checks: the v1 slot renderer is classified as its own scenery
path so the browser's staged torch bake cannot replace it with procedural art.
Dressed, rolled and other special knight bodies also preserve earlier overlay
pixels behind opaque foreground props; normal v2 bodies use their depth mask.
The same y-order contract now covers both body paths without resampling them.

## Measured review

Six real seeded pack rooms and Cinderjaw/Mother of Spores rooms were stepped
with four simultaneous bot inputs. Every seat was rendered over 60 changing
ticks per case, at both native grains. Sixteen before/after Run snapshots are
byte-identical. Solid source props never enter a nonblocking floor cell. Special
forge/nook/threshold furniture keeps its original composite instead of an
extra generic block. Ten final review sheets include native, 2x nearest and
actual 96x28 cell buffers, plus all seven plates and both boss fights.

Milliseconds per seat; CPU 4 affinity controls concurrent build interference:

| Case | Cold exact compose range | Warm mean range | Worst warm p95 |
|---|---:|---:|---:|
| original-final v1 | 5.056–15.162 | 0.303–0.710 | 0.611 |
| original-final v2 | 3.471–9.190 | 1.333–3.024 | 4.420 |
| final v1 | 0.895–1.811 | 0.096–0.301 | 0.558 |
| final v2 | 5.752–8.579 | 2.735–3.592 | 3.968 |

The 8.333 ms warm paint budget and 16.667 ms larger delivery budget are met in
the controlled run. Cold exact snapshots are synchronous; the existing native
v2 worker keeps cold bakes off live paint. An unaffined v2 p95 spike of 9.984 ms
is retained in the timing logs; no check or budget was lowered to hide it.
The standard smoke busy fight changed from 0.92 to 0.53 ms warm in v1 and
7.66 to 5.68 ms in v2 (its original two-seat fixture, secondary evidence).

Validation: the one full crate run passed 1,174 tests and found the two old
presentation assertions described above. After updating their contracts, the
complete arena renderer suite passes 68 tests (61 ignored). The final four
dungeon contracts pass, all six importer unit checks pass, wasm32 cargo check
passes, and all 52 imported banks reproduce with --check. No golden hashes
were regenerated and no shared palette definition changed.

Review: /work/artifacts/boss-art-20261015/dungeon-int-1/REVIEW.md. Raw states,
cells, images, before/after timing logs and the reproducible review harness
are beside that index. The first boss capture placed its boss outside the
seat's view; it was rejected and recaptured with the boss nearer that knight.
The camera itself was never altered.

Requested replacement pivots (bottom centre): side (8,32)/(16,64), north
(16,24)/(32,48), south (16,16)/(32,32), v1/v2 respectively. Barred/open
are one held frame each; half-open is one staged pose with playback pending
the shared presentation-age contract. Fourteen unrelated standard-smoke
scene/grain hashes (town and mini-games) remain exactly unchanged.

## INT-2 illumination and depth

Material samples keep the 75-ink/binary-alpha gate unchanged. Lighting is a
cached RGB post-process (the same distinction as v2 irradiance), never extra
source inks. The four tick/4 flicker poses use integer +-8% emission at 7.5 Hz;
no random generator, wall clock, or per-frame pixel floats are involved.
Room-space pools reach three tiles, with pack-coloured fill and a 110/255
luma ceiling on lit floor. The perimeter falls to 42% of source value, rising
to 132% inward. Contact shadows fade over 4/8 native pixels at north faces
and one tile at side walls. Native walls now draw after the floor, at their
full 1.5-tile height, so their lower face is no longer erased by the next row.

Floor-only median cleanup is removed and source contrast rises 15%; original
slabs span two collision cells to match the establishing plates' material
scale. No sprite, collider, or actor registration changes. Delivered fixtures
cluster in pairs along solid wall cells, five tiles apart; centre lanes stay
clear. Hazard lips stay exactly inside hazard collision, with two shimmer
poses for pits and four flow poses for lava plus an underglow into dry tiles.

Crypt (32x20), Cinderjaw (40x24), and Spores (36x20) exceed the unchanged 24x14
seat view. Their north wall is truthfully outside that crop; it is not missing
from the scene. All source walls remain at the collision boundary. Existing
boss pillar formations provide internal framing. Moving a false wall into
the camera would misrepresent traversable space; camera changes remain out
of scope. Review includes full-room proof as well as the real seat crop.

The second pass keeps source silhouettes and grades static Mines ore and
Fungal obstacles down in saturation; Crypt bone decor is darker, and Keep
crack decals lose their hot orange. Ordinary legacy foes get a selective
cool rim; boss drawings and their embedded nameplates are excluded after
rejecting an over-bright rim trial. Actor contact is a 60% black ellipse.
Friendly plain arrows/bolts now use a direction-bearing narrow core, a one
pixel contour and a short trail instead of the old disc halo. Geometry,
empowered/card/class shots, hostile patterns and boss effects stay rules-owned.
Fungal decorative flame/gem pixels may not borrow the exact health-bar inks;
those seven v2 source samples map to the existing dark teal material at decode.

Terminal-only grading keeps the full raster, darkens ground to 75%, boosts
actor chroma 45%/value 15%, and boosts hazard chroma 25% without floor dimming.
Native RGB output is unaffected by this terminal conversion. The common
half-block/braille painter remains unchanged; very small actors still have
less coverage than the v1 rank-preserving reducer, so complete shape parity
needs a semantic coverage interface in that shared painter, outside this task.

All six rejected side-door banks retain swappable pack-graded iron placeholders:
dim material bars, an amber/red lock while barred, a dark mouth and two-tile
warm spill when open. Door logic is unchanged; no opening timer was invented.
The latest Fungal twelve-crop bank supplies north/south doors, stairs and seals;
its side caps are staged but rejected for ambiguous native state silhouettes.
Mines/Keep reruns still report ten merged/missing components. HUD chrome verifies,
but skinning its bar/button registration and four-seat labels is a separate pass.

Final bake composition evaluates illumination at logical v1 grain for both
kits, retaining independently sampled native v2 materials. Hazard patterns stay
at full grain. Prop occlusion derives the same geometry directly, so a v2
legacy-body mask cannot trigger an unused full v1 lighting bake. A second
full-room ground clone was also removed; the seat marks its actual ground
once, after flicker and before actors. All live pixel operations are integer.

The broad crate run exposed the v1 transport's indexed-palette contract.
That gate is retained: an exact RGB-to-existing-ink memo at the end of v1
composition snaps illumination and contact shadows back to the same 75 inks.
Native v2 remains continuous RGB after lighting. Neither shared palette nor
binary alpha is changed. The original effect-library sheet explicitly selects
the unchanged fallback; its eleven golden hashes remain pinned. The dungeon
arrow's thin directional stroke has a separate geometry/readability test.
Ripper/Engine retain their existing braille atmosphere; sparse pack motes do
not replace those established guardian effects.
Material quadrants are baked once per bank and stamped as packed rows at room
admission; this preserves every sampled pixel while avoiding repeated source
sampling over great-room floors. The v1 palette memo also preserves strict
indexed output at every transport size. All broad-run palette assertions stand.

Only dungeon room captures change: Crypt, Mines, Keep, Archive, Fungal,
Unknown, Cinderjaw and Mother of Spores, both grains. No fixture golden hash
is regenerated. Sixteen four-bot serialised pack/boss/grain states compare
byte-for-byte with the pre-change harness; source plates and default kit stay
unchanged. Review tools, raw buffers, timings and nine comparison sheets live
under /work/artifacts/boss-art-20261015/dungeon-int-2/.

Remaining art/renderer requests, in order:
1. dgn.odoor.<pack>: twelve separated measured orthogonal states, especially
   west/east barred/open with a clear vertical 1x2-tile passage. Existing
   bottom-centre pivots and sizes above remain authoritative. Keep/Mines
   metadata must refer to the same sheet hash as the PNG, without merged cells.
2. dgn.floor.fungal: same twelve states and 16/32-pixel native tile envelopes,
   centre anchor, no collision; broad earth slabs/root seams with fewer small
   patches. Source still looks busy at game grain; do not erase its identity
   with another heavy median filter.
3. A semantic foreground/ground interface for the shared variable-size cell
   painter, preserving v1's subcell body priority while retaining v2 colour.
4. Register verified dgn.hud.chrome to existing F/SPC/R/X, bars and four-seat
   labels with the unchanged metrics and a fallback. Opening animation waits
   for a rules-owned deterministic presentation age; no timer is fabricated.

Final four-seat CPU-4 measurements (ms/seat, 60 changing ticks): warm maximum
mean 1.022 v1 / 4.362 v2; maximum p95 2.426 / 6.367, inside 8.333. Cold exact
composition peaks at 15.045 / 23.552. The v2 cold 16.667 target is NOT met:
Crypt is 23.552 and Cinderjaw 21.415. The established native bake worker keeps
these cold compositions off live paint, but synchronous screenshots/wasm room
admission can still hitch. No budget or test was relaxed. Next renderer work
must reduce cold v2 bank/large-room composition, preferably with visible-region
composition while preserving room-space fields and four independent cameras.

Final validation: 1,193 crate tests pass (150 ignored), wasm32 check passes,
six importer unit tests pass, ten touched banks reproduce with --check, and
240 native PNGs pass the unchanged ink/alpha gates. Against exact starting
commit 0a5be57a, fourteen unrelated standard-smoke scene/grain hash pairs are
unchanged; only first-room and busy-fight change. Its standard two-seat busy
fight is secondary: warm v1 .51 to .95, v2 5.73 to 4.18 ms. Primary four-seat
raw timings and the earlier contested/outlier runs are retained in the review
folder, including the 9.989 ms p95 during parallel checks.

## Evening finish

Mines and Keep reruns now use clean per-grid measured crops instead of the
old 5/9-component split. Their source sheets and the matching measured retry,
Fungal and HUD sheet are retained under `assets/dungeon-src`, with original
sheet/metadata hashes. `scripts/dungeon-finish-import.py` reproduces that crop
review from the read-only delivery. Normal conversion uses `slot-art-import.py`
with `--source-dir crates/delve/assets/dungeon-src/<slot>` and `--requests
crates/delve/assets/art-slots/dgn.requests.json`; `--check` verifies it.

North doors draw their complete connected frame from the north wall baseline,
rather than losing their top eight pixels above the canvas. Fungal and Keep
orthogonal south/stair states are active. Mines' south-open retry is a solid
timber crosspiece that reads closed; its readable fallback remains. Side states
remain inactive after native review. Half-open poses remain
staged. No rotation, perspective warp, door collision or timing changes.

Empty HUD chrome is BOX reduced and palette bound offline. Cached nine-slice
skins fit the existing action wells, health meters, boss bars/nameplates and
cell sidebar. Icons, readiness shading, counts, keys and text draw afterward;
the four-seat sidebar keeps its existing information and positions. Minimap
chrome surrounds the existing known-room map. A trial four-seat corner overlay
was rejected because it covered a Keep warning ring and reduced poorly in cells.
No panel is added over the playfield. Pressed/status pieces are registered for
future authoritative signals; they do not create presentation timers.

Fungal floor variation is plain earth with worn material in one of 29 seeded
slabs; wall trim is omitted and wall dressing is one in six fixtures instead of
one in two. Offline floor contrast .55, saturation .48 and value .58 calm the
source roots; decor value .62/saturation .55 keeps it behind actors. Hazard
edges, collision-owned props and lighting fixtures remain exact. Broader quiet
Fungal source tiles and clear side-door states remain requested in ART-REQUESTS.

Iron Tusk's ore is now a 14x14 / 26x26 source-centred display envelope, from
32x32 / 64x64. Conversion records retain the same source crops/socket; damage,
point collision, velocity and frame timing are unchanged. This keeps the large
downward trail within the existing lower playfield margin.
## BOSS-READ-1 arena separation

Stairs and Lair halls keep the same slabs, wall fixtures and hazard geometry,
with a lower central ambient band (200/256 maximum material gain, versus the
ordinary room's 338/256). Wall pools remain off the central standing area.
Library bosses add a cached soft dry-floor shade receiver that follows their
feet, plus a tight contact falloff. Hazards do not receive this new shade.
Their cached body banks lift only dark silhouette edges using the material's
own hue ratios. Ordinary room grades and scenery source art are unchanged.
All seven registered bodies pass the unchanged native/cell floor-contrast rule;
measurements and reviewed before/after captures are in
`/work/artifacts/delve-int-20261015/boss-read-1/`. See BOSS-LIBRARY-IMPORT.md for
acceptance values and the paired-floor exporter.

## INT-4 entrance rooms and vertical doors

Six ART-18 room masters now replace the entrance backdrops for the Alchemy Lab,
Smithy, Bathhouse, Tavern, Workshop and Siege Muster. Supplemental slot ids are
`{cinder,latch,mossbell,brasswake,rafterwake,pennon}.entrance`. Their contracts
are exactly 384x224 and 768x448, BOX-reduced separately from the delivered
1640–1643x957–959 masters. Actor drawings, prompts and mini-game screens retain
their existing stamps. The room painter reads the original collision grid:
walkable perimeter pixels receive quiet source-floor slabs, existing blocked
furniture/hazards remain at their rule coordinates, and unused source doorway
gaps receive the authoritative wall. Neither collision nor exits are moved.
The v2 backdrop uses its own source reduction under the unchanged 2x actor
and prompt layer. Decode and masked room composition are cached. Four cached
poses of the existing furniture painter preserve ambient movement on the
existing six-tick scenery cadence; selecting a pose does no pixel grading.

`dgn.sidedoor-{crypt,mines,keep,fungal,unknown}` provides fixed 16x32 / 32x64
west/east closed, open and locked canvases, stamped at the original door tile.
A shared source centre/scale aligns states; thresholded detached chips are
removed before BOX conversion. Closed is the existing live-foe closure; locked
is the existing empty-room wave/activity bar; open is the existing unbarred
state. All scenery cache keys include that display distinction. No door rule
or collision changes. Archive retains its rejected-sheet placeholder.

Reproduce with `python3 scripts/entrance-art-import.py [--check]`. Each bank's
`provenance.json` binds the accepted original source/metadata SHA, cleaned
source SHA and preparation recipe. Maps retain all twelve door components;
six worn alternatives remain staged. Whole-room sheet-edge/crop flags are
intentional opaque-backdrop diagnostics, retained rather than waived.
The first green-fleck grade and the second mottled floor were rejected during
native/cell review. Final source cleanup and low shadow-ink merges are explicit
in the maps; the shared palette and admission gates are unchanged.

`slot-art-review`'s `entrance-review` binary captures all six entrances and
both side doors in all three states for all packs, with four seats, both native
grains and the actual cockpit cell painter. It records entry/walk state bytes
and asserts that painting does not mutate them. The dev host accepts
`--snapshot FILE --idle-bots` for repeatable four-seat room review; its normal
scene and bot behavior remain the default. Use assigned integration ports.
