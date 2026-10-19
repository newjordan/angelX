# Slot art import

`scripts/slot-art-import.py <slot>`, or `--all`, consumes the art team's
read-only `/work/artifacts/art-prod/slots/<slot>/v01` deliveries. It never
runs the production lane or writes its sources. Only native assets, maps,
provenance and reviews enter `crates/delve/assets/art-slots`. Rich sheets stay
external, bound by absolute path and SHA-256. The source library and onboarding
are untouched. Use Python with Pillow, or the script's `uv` shebang.

```
python3 scripts/slot-art-import.py --all --quality /work/artifacts/boss-art-20261015/art-imp-1/QUALITY.md
python3 scripts/slot-art-import.py pennon.knight --v1-height 48 --v2-height 48
python3 scripts/slot-art-import.py --all --check
python3 scripts/test-slot-art-import.py
```

The Luna production lane delivers source-sheet.png, measured source-sheet.json,
notes.md, prompt.txt and generation.json. `status.json` must start with `OK`
for inclusion in `--all`; `OK?` is staged with a provenance suspicion flag.
FOREIGN and NOSHEET are excluded. `--resume` reuses matching source, metadata,
notes and map hashes. If a delivery changes during a batch, its existing
native bank is retained and flagged for manual mapping; the incoming hash
and reason are recorded. Archived originals are located by SHA under `_old`. Single continuous backdrops currently lack
crop JSON: the importer measures their alpha bounds, records that exception
and requires review. It does not fabricate isolated-piece metadata.

## Maps and registration

Every `<slot>.map.json` is editable data. It records source/notes hashes,
request fit/geometry/timing, both kit sizes, registration mode, native anchor,
and one record per measured piece: measured index and rectangle, delivered
reading-order label, proposed state/frame/name, source pivot, approval and
runtime selection. Initial maps are proposals. Count equality alone is not
approval: a source can reorder poses, introduce recovery or repeat cycles.
Review the labels and source image, correct the map, then re-run. A changed
source hash or crop requires explicit map review; it never silently rebinds.

Row clustering uses vertical centres followed by horizontal position. Sorting
only top edges misorders raised weapons (Latch pieces 1/2 demonstrate this).
A count mismatch preserves *all* delivered components and lists the request
count. Missing/merged actors are never split by an invented regular grid.
Cinder Crock's 8, Clasp's 9, Captain's 10 and the first Crown's 18 components remain
manual mapping work. Detached shards/lids and alternate cycles cannot silently
become whole-body action frames.

Actors use one scale for their entire bank, one native anchor and one feet
baseline. Source pivots are initially estimated from the lowest alpha band;
reviewers must correct airborne poses, shadows and detached equipment.
Registration pads onto a common transparent canvas rather than stretching each
crop. Props/effects use their own bounds and recorded pivot; a map can request
a shared scale for a registered effect cycle. All sampling is BOX from the rich
source, never enlarged. Both kit defaults equal the existing request canvas:
the request specifies no separate v2 envelope, and these mini-games currently
render at 384x224 before the v2 presentation enlargement. Height overrides are
explicit map parameters, not a default kit or camera change. Staged v2 PNGs
are available for a later renderer that actually consumes their native grain.

## Despill, alpha and colour

The importer erodes the alpha>=128 mask by three source pixels. Trusted
interior colours propagate through the connected alpha region; boundary RGB
is replaced only where alpha is below 224 or the magenta detector triggers.
Opaque dark contours are preserved; the original alpha is retained. Interior
warm copper/cloth is untouched. Magenta candidates require red and blue both
well above green; residue is measured at the boundary, not on intentional
interior colour. Thin features without an interior remain unchanged and are
flagged if contaminated. BOX sampling uses Pillow's premultiplied RGBA path,
then alpha is made exactly 0/255 at 128. Fully transparent RGB is zero.

Nearest shared ink conversion excludes dead black and signal ivory. There is
no dithering, exposure boost or invented detail. The 75-ink definition is
unchanged. `slot.json` records recoloured counts, fringe residue, source scale,
output hashes, occupancy and colour agreement against the unquantized sample.
Knight seat masks are derived before quantization from blue material dominance
and require visual review. They do not recolour steel, brass or leather by
matching a shared shadow ink. Their four shade levels drive cached seat ramps;
source-provided material masks should replace this inference in future banks.

## Gates and runtime scope

No existing art gate is relaxed. Legacy assets and their tests remain intact.
Slot conversion requires exact binary alpha, membership in the existing shared
palette, no upscaling, preserved crop rectangles and source hashes, unique
output names, deterministic byte-for-byte `--check`, and valid dimensions.
Colour agreement within RGB distance 60 is measured; values below 0.90
are flagged needs-rerun. It is not proof of identity. Weak occupied silhouettes and fewer than two covered 4px
half-cells are flagged for review; that diagnostic is not the terminal writer.
`QUALITY.md` distinguishes OK, needs-rerun (suspect provenance/edge clipping)
and needs-manual-map. Extra delivered components keep the mismatch visible,
even when the existing pilot subset has been reviewed and activated.

Only a map with runtime=true and individual approved=true/runtime=true frames
is embedded by build.rs. Missing directories, maps, frames or mask files leave
the original PNG fallback available. Decode/recolour happens in OnceLock
caches; the live draw path selects/stamps cached images. Only Latch knight's
five existing states and strike's first four frames are selected. All stamp
positions, timers, damage, rules, four-seat stakes and default kit are unchanged.

The isolated `scripts/slot-art-review` crate extracts the actual cockpit cell
painter and Latch panel function at build time. It captures identical native
states and 96x28 cell buffers before/after, plays a full four-bot round twice,
and asserts byte-identical simulation outcomes. It links no cockpit services.

```
TMPDIR=/work/tmp ANGEL_CARTRIDGES=/work/tmp/empty-cartridges CARGO_TARGET_DIR=/work/build-cache/gameshelf-cockpit cargo run --manifest-path scripts/slot-art-review/Cargo.toml -- /work/artifacts/boss-art-20261015/art-imp-1/captures
```

## Remaining renderer work, by game

All paths below start at `crates/delve/src/render/overworld/arena/`. The request
audit has **18 hard-coded slots**, plus mixed/recipe slots with further
composition work. The list is implementation work, not a request to redraw.

| Game | Hard-coded slots and functions | Next integration task |
|---|---|---|
| Cinder | cinder.rs: draw (knight), pot, room, draw/center (chrome) | Expose knight-specific states without changing knights::figure; add pot/ghost/slag-cell slots; layered room and empty chrome skins. Review Crock component/damage/terminal registration before replacing its arrays. |
| Latch | latch.rs: room, draw/center (chrome) | Review Clasp's 9 merged pieces against its source labels and visor/damage overlays. Add parry/clash slots only after selecting complete cycles. Slice empty panels independently of labels. |
| Mossbell | mossbell.rs: knight, art (bumper/ball/trail/steam), draw (flipper lines), room, draw (chrome) | Add banks for knight, targets, flippers, ball/trail, steam, room and chrome. Preserve flipper pivots/contact surfaces and vine x-projection/safe gaps. Review missing 3 target variants and merged crown/root crops. |
| Brasswake | brasswake.rs: world/room, draw (chrome) | Register keeper/porter pose reordering and damage overlays; add optional cask/bread/contact/slam/steam loaders. Layer the room without painting over lane boundaries; slice chrome. |
| Rafterwake | rafterwake.rs: draw (chrome) | PNG overrides for recipe banks in art/frame/knight/keeper, including tagged terminal detail and seat ramps. Preserve plank endcaps, viewport and camera. Reject foreign chrome delivery; backdrop needs depth layers. |
| Pennon | pennon.rs: draw (chrome) | PNG overrides for art/frame/knight/captain/sentry and props/effects, with directional banks, seat/detail masks and captain damage variants. Preserve exact danger/aim marks; supply absent bolt/repair/merlon/brick/pennant/drop/hook/dust slots. |

Adding a slot means reviewing its map, retaining every supplemental component,
selecting only states the rules already own, adding an optional loader at the
existing stamp, and capturing all four seats before activation. Do not use a
numerical pass to approve a mismatched design, hidden danger or illegible face.

Late deliveries are audited with `scripts/slot-art-quality.py <terminal-slot-metrics.json> <QUALITY.md>`. It validates every staged PNG hash, dimensions, alpha, palette and seat mask, then attaches native-SHA-bound actual cell metrics. Re-run the review crate first after importing arrivals. The current delivery count is reported in QUALITY.md, not frozen in this document.

The first despill trial recoloured all boundary pixels. Enlarged proof exposed lost dark contour, so it was rejected. Version 2 preserves opaque source ink and has an explicit negative-control test for that case.

When artists rerun v01 concurrently, freeze integration to each map's source
SHA. Reproduce an archived bank with `--source-dir <archive-slot>/v01` and the
same slot id. This preserves the recorded incoming-delivery warning while
rebuilding the mapped source. Review and replace the map to adopt the new
sheet; do not silently reinterpret an approved runtime pose.
The converter also excludes the palette's signal ivory (e4fff8); highlights
use the existing material ramps. No palette definition or legacy art changes.

### SLOT-INT-A: Cinder and Latch rollout

All 17 assigned sheets were visually inspected. Maps now name the selected
existing states and explicitly retain supplemental poses. The newer Cinder
pots/slag and Latch strike/room sources replace their archived deliveries.
Every original source SHA is recorded in `source_preparation.original_sha256`;
only derived native files are embedded. No library, onboarding or rules change.

`crates/delve/assets/art-slots/cinder.chrome/prepare.py` (relative to `crates/delve`) is the
reproducible preparation recipe for these two games. Run it from the worktree:

```
python3 crates/delve/assets/art-slots/cinder.chrome/prepare.py --output /work/artifacts/boss-art-20261015/slot-int-a/prepared
python3 scripts/slot-art-import.py latch.clasp --source-dir /work/artifacts/boss-art-20261015/slot-int-a/prepared/latch.clasp --check
```

Repeat the second command for each assigned slot. The preparation recipe checks
reviewed original SHAs, thresholds alpha at 128, removes translucent red fringe,
clamps pale effects and grades saturated orange cores into existing ochres.
It supplies explicit crop metadata across inspected Crock/Clasp gutters and
never edits the importer or the read-only deliveries. Prepared sheets and their
metadata stay outside git; their hashes, crop rectangles and recipe stay in it.
Source metadata and mapped metadata differ deliberately for these manual cuts.

Cinder activates its miniature, three pots, slag/drop and cell, breath, clear,
impact, backdrop and chrome. Dashed ghosts are staged: one disappears entirely
at 24x20, so all retain the exact inset outlines. Terminal pot copies hue-grade
warm emblems into their blue/green material ramps; shape, alpha and value steps
are unchanged. Crock remains staged because a second flinch, registered damage
acting poses and terminal seam masks are absent. The fixed red seated knight
also remains staged: its camera, gear and inferred seat mask cannot preserve
the modular overhead player portrait. Both slots have fallback loader hooks.

Latch activates nine manually separated Clasp poses, the five existing knight
states, first-row parry, a four-stage copper clash, first-row strike, backdrop
and chrome. High guard reuses the horizontal brace source, with that duplicate
explicit in metadata. Dedicated high patch/shins-low guards and damage acting
poses remain art gaps. Reviewed face sockets relocate the existing twin-eye
and opaque-body seam treatments; feet, 100x100 Clasp envelope, 128x80 knight
canvases, contact stamps and exact 25x2 high/low lanes retain their positions.
The imported background is dimmed under fighters; labels retain black wells.

Both games cache decode, colour grading, room variants and sliced trim. Source
chrome contributes only a three-pixel border; renderer-owned text wells and
all typography stay intact. The first full-fill slice was rejected because
stretching rounded source corners invaded the podium. The decorative layers
remain independent of the exact landing/column and counter-lane marks.

No gate or palette was relaxed. Whole-sheet count/provenance/weak-extra flags
remain visible in `slot.json`; they are separate from reviewed runtime subset
approval. Full-frame backdrop edge contact is intentional. Bright source cores
were rejected and material-graded rather than admitting new inks or raising
contrast thresholds. The exact 0.90 colour-agreement diagnostic remains; the
prepared runtime effects are measured against their explicitly graded source.
Four-seat pixel tests protect both exact counter lanes and every landing-frame
edge; before/after four-bot captures also compare byte-identical simulation.

The global art-request inventory hashes renderer sources. Its `--check` becomes
stale after these hooks/line moves. SLOT-INT-A's file allowlist excludes that
shared JSON, so it is deliberately left unchanged; regenerate it once after
merging the parallel game integrations. A ready diff and regenerated copy are
in the review artifact directory (`art-requests-refresh.patch` and
`art-requests.refreshed.json`); only Cinder/Latch facts and renderer hashes move.

### POLISH-A: Cinder and Latch readability

The verified 2026-10-15 knight reruns replace both knight banks. Cinder activates
the upright portrait; Latch maps the raised shield, shins-low guard, level brace,
forward thrust and upright recovery by eye. All supplemental poses remain in
the maps. Source SHA-256 and crop metadata hashes bind each reviewed delivery.
The damaged Crock/Clasp extras remain inactive: their delivery notes reject the
merged/unregistered acting banks. Existing damage frames remain authoritative.

`cinder.chrome/actors.py` prepares the two knight sheets and their maps;
`polish.py` prepares Latch's continuous radial backdrop; `masks.py` applies
source-registered cloth masks and removes the detached neighbouring boot from
the contact crop. These scripts live in `crates/delve/assets/art-slots` and use
`/work/artifacts/boss-art-20261015/polish-a/prepared` for reproducible inputs.
Run preparation, the unchanged importer for the three slots, then `masks.py`.
Importer `--check` applies before the documented cleanup; `masks.py --check`
verifies the final masks and native cleanup. No importer/loader gate changes.

The portrait's ivory plume/tabard and Latch's painted plume/tabard/shield field
carry saturated seat ramps; metal, leather, brass and warning effects retain
their materials. Cool steel grading is cached independently. Cell sprites use
six inks and a four-native-pixel seat/cool boss rim. Latch cell knights fit a
89x56 copy inside the same 128x80 socket, clearing the shared COUNTER row
without shifting the feet. Room copies use 30% of the
native backdrop luminance. Puzzle cells and exact counter/landing marks retain
their original positions and inks. Crock's existing four damage banks are
cached at 104x104 (1.30x), keeping the floor anchor; clear flashes shake by the
existing damage step. Only the boss alcove reveals dim delivered kiln masonry.

Native Latch cues move to the upper left bay, clear of the raised hook/shield;
text plates have a four-pixel inset. Three race pips now occupy distinct slots.
A stage-wide 1.25x zoom and HUD diet are not adopted: the exact contact lanes,
Cinder's rival column heights and full control information remain visible.
The shared cockpit cell-label painter owns terminal cue coordinates and hints;
its matching relocation/compact HUD needs a follow-up outside this task's scope.

## Mossbell and Brasswake rollout (SLOT-INT-B, 2026-10-15)

All 24 source sheets and their measured pieces were visually reviewed. Maps
retain every source rectangle and SHA, select existing states explicitly, and
leave supplemental poses as proposals. None of these sources changed SHA from
the initial staging; the unrelated eight reruns belong to other integrations.
An approved subset is separate from whole-sheet diagnostics: count mismatches,
unselected hot cores and continuous-background edge warnings remain visible.

| Slot | Runtime subset / remaining work |
|---|---|
| mossbell.keeper | Nine existing poses; root recovery and walking remain proposals. Delivered damage idles replace legacy socket paint; damaged action variants are still absent. |
| mossbell.knight | First four lever combinations, 20×24 at the original stamp; native masks remove posts/base plates and recolour garment/plume for four seats. Extra crouch/nudge/drain/cheer poses remain proposals. |
| mossbell.targets | One idle ring, diamond and seed-petal body; existing code makes four contour flashes and lit indicators. |
| mossbell.ball | One upper-left glint marble, 9×9; other glints/sizes unwired. |
| mossbell.trail | Delivered sheet rejected; a dim 5×5 echo is cached from the active marble, with the legacy trail when that marble is absent. Optional independent trail hook remains available. |
| mossbell.root | Staged, hook with exact legacy fallback: the delivered bush becomes a tiny clump in 179×11. Needs four registered advancing band fronts, both directions, with the existing plunger margin. |
| mossbell.crown | Staged, hook with exact legacy fallback: diagonal jaws fail the horizontal 51×24 four-corner landing gate. Needs four registered gate stages. |
| mossbell.flippers | Staged, pair hook with original line fallback. Raised-pair cells omit a paddle; isolated broad hinges do not match the five-pixel contact rail. Future complete pair canvas is 114×26 at (OX+field_x(37)−3, OY+108), with pivots (3,11)/(110,11), raised tips y=3 and resting tips y=22. |
| mossbell.bumper-burst | Reviewed retained-unused bank, no new event/stamp; optional loader exists and runtime remains off. |
| mossbell.impact | Four ignition/expansion/peak/breakup frames; same-family alternate peak avoids first-cycle highlight loss. |
| mossbell.jackpot | Seed/open bloom/smoke/remnant subset, four existing frames. |
| mossbell.steam | Compact curl at 8×14, cached in atmosphere only, moving through the same nine rise offsets; other curls remain proposals. |
| mossbell.room | Dimmed continuous conservatory behind the original opaque field, plunger, drain, rails and gutters. Fountain/drain scenery cannot show through the playfield. |
| mossbell.chrome | Empty title and panel crops, six-pixel borders; flat shared-ink centres keep source wear away from live labels and all four podium rows. |
| brasswake.keeper | Corrected all twelve pose mappings; source right-facing bank mirrored once to original left-facing convention, then mirrored for right-facing foes. Native ground-puff/speck cleanup; delivered damage poses retain their own face. |
| brasswake.porter | Ten mapped existing poses; missing standing damage variants use the reviewed idle plus existing cached fracture treatment. Seated/prone alternates are not substituted for living damage idles. |
| brasswake.knight | All twelve poses, 48×48 feet anchor; explicit garment/plume masks preserve four seat identities, steel and leather. Existing overlap contour remains. |
| brasswake.cask | First intact upright cask only; held/airborne/damage/lid/hoop extras unwired. |
| brasswake.bread | First intact loaf only; bob/collection/variant poses unwired. |
| brasswake.contact | Four warm fist-contact frames; alternate ignition/peak plus first-cycle expansion/breakup preserve the colour diagnostic. Guard cycles remain proposals. |
| brasswake.slam | First complete four-frame ground burst. |
| brasswake.steam | First complete four-frame rush curl; exact lane arrows and borders remain code-owned. |
| brasswake.room | 254×146 crop, tiled at original 240-pixel section spacing; source wall rows 0..96 register to the 36-pixel vat band and remaining floor to 110 pixels. Original stage/gutters/camera remain. |
| brasswake.chrome | Empty title/plain panel, cached six-pixel borders and flat centres; dynamic labels, values and four-seat podium stay live. |

No gate or palette definition changed. Every selected native frame clears the
existing 0.90 colour-agreement diagnostic (unselected proposals can still fail).
Cached decorative highlights above 224 map to existing inks at/below 224;
background dimming also snaps to existing inks. Legacy art keeps its old gates.
Initial oversized nine-slice corners and mottled panel centres were rejected;
source corner regions now reduce into six-pixel borders, with plain centres.
Brasswake re-stamps the exact two-pixel danger borders after decorative effects
and bodies so attacks cannot hide them. Geometry, arrows, timing, rules, stakes,
canvases, seat cameras, feet stamps and the default kit are unchanged.

Native cleanup is sparse, reviewed data in each affected map's
`runtime_cleanup`, with a readable `cleanup.json`. The corresponding loader
embeds the same data so absent slot directories still compile with legacy art.
Importer change request: support these sparse erase/material masks directly,
including non-blue cloth, and hash their runtime result. Until then the importer
and its PNG/hash checks stay untouched; cleanup/recolour runs once in caches.

The shared `art-requests.json` checker detects renderer SHA/line drift after
this integration. This task leaves that shared file untouched: the merge owner
must regenerate it once after merging the sibling renderer changes. Inventory
extraction still finds the same 67 slots and unchanged rule/geometry requests.
Review evidence, four-bot equality checks, native/96-column captures, source
hash audit and warm/cold smoke logs: `/work/artifacts/boss-art-20261015/slot-int-b/`.

### POLISH-B: Mossbell and Brasswake display corrections

Mossbell projects its unchanged 160-pixel rule table at 8/5 instead of 5/4:
the cabinet is 268/384 pixels wide (69.8%), with 56-pixel side cards. Scores
use two lines per seat; all four ranks, short names, scores and drains remain.
Root fronts and crown corners are sampled horizontally through that same
projection; their rule coordinates, safe gaps, feet baseline and ticks do not
change. The three staged danger/flipper deliveries were re-inspected and stay
off: raised pairs omit a paddle, roots collapse into bushes, and crown jaws
run diagonally. No invented replacements were drawn.

The source Keeper has teal foliage but woody gold limbs and shell. A reviewed
material grade restores the requested moss identity while retaining the small
amber face, silhouette, contour, pose registration and source tone steps. The
six body inks are existing shared colours: 132724, 182414, 263b1c, 35543f,
3a6d5d and 6fa46d. This requires no shared palette change. Native source PNGs
and their hashes stay untouched; grading is cached by pose.

Both knight banks now use reviewed sparse textile masks from their slot's
cleanup.json: plume, scarf and tabard, excluding eyes, steel, gloves, leather,
belts and brass edging. Four local four-tone textile ramps supply saturated
red, blue, green and gold, without changing the shared 75-ink admission gate.
Brasswake's neutral armour grades to cool steel; rival armour stays warm.
Cell banks use five body colours and a four-pixel seat rim (one 96-column
half-cell); enemies use one fixed cool rim. Cached cell backdrops are 30% of
the already-dim native backdrop. Text clears entire underlying terminal cells
before the existing cockpit writer places glyphs, keeping trim in gutters.

Brasswake's Keeper is nearest-sampled at 3/2 once in the cache, then stamped at
the same world feet. All danger borders are still redrawn from rule rectangles.
The empty decorative vat band contracts from 36 to 12 pixels; camera, floor
projection and lane geometry remain. Chrome uses two-pixel source borders,
four-pixel text insets, a centred title and inset right-aligned score columns.

The new brasswake.keeper-damaged delivery is mapped by eye into seven existing
action/rest keys: stage-1 slam/rush tell/action and stage-2 idle/slam tell/action.
Other components remain reviewed proposals, including recovery states that
have no live playback. Stage-2 rush actions retain the base bank. The source's
pre-enlargement is disclosed in its map; native conversion only downsamples.
Run its local import.py [--check] to reproduce through the unchanged shared
converter using request.json; the shared request inventory lacks supplemental
slot IDs. Mossbell's damaged delivery fails transparency (baked checkerboard,
one merged border-touching crop) and is rejected without trying to erase it.

Each game's chrome/hud.json holds a display-only compact HUD trial selected by
ANGEL_MINIGAME_HUD=compact before starting a host. It places four score pips in
a top strip and hides walk/punch/grab hints after 150 elapsed ticks. Full HUD remains
the default: the trial frees panel area but adds no playable pixels, crowds
the header and loses live short names. All information stays available in the
default profile and the four-seat podium; no rules or simulation data change.

### Rafterwake and Pennon integration (SLOT-INT-C)

### Rafterwake and Pennon integration (SLOT-INT-C, POLISH-C)

All 26 slots have optional hooks and reviewed runtime selections. Maps retain
external source hashes and every supplemental component. Rules, hit boxes,
feet, exact danger areas, safe gaps, timing and payouts remain authoritative.
The four-bot simulation JSON is compared byte-for-byte across renderer changes.
Decode, registration, colour grading, actor variants and sliced source trim are
cached; no per-frame floating-point pixel processing is added.

The isolated Captain rerun replaces the previous mixed-style subset. All twelve
rule poses now have imported frames, including hook tell/action, damage-2 and
defeat. Its shared native bank is 64x80, with foot pivot (32,78); the existing
+2 draw offset places that pivot on the rule floor. The old padded 128x160 bank
remains understood by the loader, but is no longer active. Repeated anticipation
keys reuse the delivered tell pose while exact danger marks retain their four
stages. Damage action variants still reuse base actions; no timings are invented.

`crates/delve/assets/art-slots/pennon.captain/prepare.py` (relative to `crates/delve`) freezes
the reviewed source SHAs and creates reproducible importer inputs. Run:

```
python3 crates/delve/assets/art-slots/pennon.captain/prepare.py --output /work/artifacts/boss-art-20261015/polish-c/prepared
python3 scripts/slot-art-import.py pennon.captain --source-dir /work/artifacts/boss-art-20261015/polish-c/prepared/pennon.captain --check
python3 scripts/slot-art-import.py pennon.sentry --source-dir /work/artifacts/boss-art-20261015/polish-c/prepared/pennon.sentry --check
```

The Sentry preparation appends the standing-damage delivery to the original
sheet. Guarded standing and low last-stand poses replace the old grounded damage
rests; the other ten original runtime PNGs remain byte-identical. Unused standing
actions and the old slumps remain staged proposals. Original and prepared hashes
are recorded in the maps. Count/provenance diagnostics stay visible; neither the
shared importer nor loader was edited, and no palette gate was relaxed.

The display prototype lives in `rafterwake.chrome/presentation.json`. It selects
an 8:5 nearest-pixel Keeper stamp, anchored at the same feet, with its blue fabric
graded to slate; removes the added diagonal scan lines; shows 160 world rows
(about 4.4 decks) through a display-only camera translation; and crops the top
24 of Pennon's 144 stage rows before a 4:3 nearest-pixel presentation. The latter
occupies 338x160 display pixels. Every danger edge uses the same stage transform.
The shared danger ramp uses subdued teal rather than the knights' brass.

The compact HUD keeps four colour pips, places, names and scores in the top
strip. Clock, pot, focused progress/ward, falls, best and combo remain live;
full names and finish times remain on the podium. Controls appear for five
seconds on each round/restart. Set `hint_seconds` to 75 to keep them visible,
or `compact_hud` to false for the side-panel layout. Hints overlay 18 stage rows
while visible; hiding them exposes 4,572 Rafterwake / 6,084 Pennon display pixels.
Source trim stays outside the live text; combo now sits below and to the right
of the title. Terminal labels reserve complete 4x8 glyph cells. Podium labels
retain the panel background because the writer centers/right-aligns those fields.

Both knights have per-pose reviewed fabric/crest regions in `seat-materials.json`:
cloth gains saturated seat ramps, while steel, brass, leather, skin and effects
stay outside the material selection. The actual cell path grades the backdrop
to 25%, posterizes actors to at most six inks, adds a one-cell-width seat rim
and a fixed cool boss rim, and retains the existing half-block/braille writer.
The first broad-rim trial with subpixel contour promotion was rejected: it filled
the body with seat colour. Cached rims now use the ordinary body sampler.

Remaining art gaps: four distinct anticipation poses per attack, damaged action
banks, Rafterwake run-C/crouch, additional VFX cycles, directional projectiles,
alternate props and spare badges remain proposals. Importer/loader requests:
reviewed material masks and separate common-scale participation would remove
runtime cleanup workarounds; neither shared file changed. Sources/library and
onboarding remain read-only and untouched.

## Shared loader follow-ups (SLOTLIB-1)

`arena::slot_art::warm(Game)` fills the existing native and terminal OnceLocks
using a private, unstepped four-seat session and the normal painters. `warm_all()`
runs in `DelveClient::boot`, before snapshots/display, on native and single-threaded
wasm alike. It is synchronous startup preparation, not a live-frame job: hosts
embedding the arena without `DelveClient` should call it before their display
loop. It never edits a live run, changes settings or consumes its random stream.
All six games, both cell/native paths, four default seats and podium trim are
prepared. Custom equipment palettes still populate their own variants on demand.

Four renderer cell-sprite helpers now call one shared baker. The cache compares
dimensions, binary alpha, exact cleaned/graded RGB, ordered palette and rim;
identical damage-stage/facing inputs reuse the conversion. Hash lookup is followed
by exact equality. `runtime_banks()` reports stable dimension/alpha/RGB hashes of
both the actual graded input and final cell bank. These FNV fingerprints are
review identities, not cryptographic source provenance. `art-requests.json` now
also SHA-256 binds active PNGs, seat masks, reviewed cleanup/material JSON, maps
and preparation recipes, including supplemental banks. Renderer hashes bind
runtime procedures. This distinguishes source-sheet provenance from final pixels.

The real cockpit cell-label painters all consult `slot_art::cell_layout(game)`.
`crates/delve/assets/art-slots/cell-layouts.json` maps native label origins to new origins and
terminal left/center alignment. Arbitrary fields (including four score fields)
can use a compact top strip; hint rows can have `hide_hints_after_ticks`. Podiums
bypass both moves and hiding. Latch's HIGH/LOW, COUNTER and PERFECT now match its
native cue bays; full Cinder/Latch/Mossbell/Brasswake HUD information remains.
Rafterwake/Pennon's renderer-selected compact strips retain their coordinates.
No gameplay area, camera or simulation setting changes with these label maps.

### Optional reviewed import controls

Old maps retain version-2 conversion and byte output. No control is inferred or
added to an existing map. Supplemental ids such as `brasswake.keeper-damaged`
may carry a complete `request.json` beside their map (`id`, `role`, `fit`,
`timing`, `geometry`, `frame_groups`); they must not shadow an inventory request.
The build-time loader already discovers approved runtime maps by directory.

A map may specify `scale_reference_pieces: [0, 2, 4]`. These measured ids must
be approved and active. They alone determine the common scale for a registered
bank; all supplemental pieces are still exported and count diagnostics stay
visible. Their canvases may clip unused long equipment, so review those exports.
Without this field every measured piece participates exactly as before.

Each piece may add `reviewed_processing` (explicit `reviewed: true` required):

```json
{
  "reviewed": true,
  "cleanup_rects": {"v1": [[90, 45, 38, 35]], "v2": []},
  "erase_runs": [[12, 9, 3]],
  "replace_inferred_material": true,
  "material_runs": [[10, 5, 4, 2]],
  "grade": {"527597": "456070"},
  "material_mask": {
    "path": "reviewed-cloth.png",
    "sha256": "REVIEWED_FILE_SHA256",
    "space": "source"
  }
}
```

`source_crop_xywh`, when present, is a reviewed subcrop relative to the
measured source rectangle. It retains that original measurement and adjusts the
source pivot and material mask together; it never fabricates new crop metadata.
Rectangles are native `[x,y,width,height]`; erase runs are `[y,x,width]`;
material runs are native `[y,x,width,shade]`, shade 0–3. Each list may instead
be keyed by kit. Coordinates must fit the canvas. Erase is applied after native
sampling; material coverage is clipped to the final opaque body. Material runs
can select ivory, ochre or other non-blue fabric without any hue test; an empty
explicit replacement keeps the mask empty. A file mask is RGBA, with shade
values 0/70/140/210 and alpha coverage. `space: source` means the measured crop's
canvas, registered with exactly the art's scale/pivot/BOX sampling;
`space: native` means the output canvas. Keep reviewed masks inside the slot
folder, bound by SHA-256. Grade maps admitted native RGB hex inks to admitted
shared inks; it cannot add palette colours. Control hashes join output records.

`--source-dir` accepts one SHA-matching delivery, including prepared or archived
sources; source, metadata, notes and map hashes all remain checked. During
`--check`, moving a matching archive does not rewrite the recorded source path.
`--all --source-dir` is rejected because one directory cannot identify a batch.
`--resume` cannot bypass reviewed masks or the checks requested by `--check`.
Existing banks with documented post-import cleanup still use their preparation
and cleanup recipe; these optional controls do not silently migrate their data.

Fresh entry measurements (240 frames, a tick every two frames, new process/module
per game/kit) use `delve/smoke --entry <3..8> <0|1> [--warm]` and
`node delve/smoke/entry.mjs <wasm> [--warm]`. The first 120 frames measure entry;
the last 120 measure the warm distribution. Preparation time is separate and
reported explicitly. Pixel checksums must agree before/after and across targets.
`scripts/slot-art-review`'s `slot-entry-review` binary runs all six four-bot rounds
before/after preparation, compares full simulation JSON and RGB, and captures
96-column buffers through the actual cockpit writer.

### DAMAGE-1: supplemental acting banks

Five supplemental slots use `cinder.crock-damaged/prepare.py` and its reviewed
`registrations.json`, relative to `crates/delve/assets/art-slots`. The recipe
checks original delivery SHAs, cuts complete figures across inspected gutters,
despills, thresholds alpha, and pads each bank onto one registered canvas.
It calls the unchanged importer with local `request.json` contracts; rich
sources remain external. Reproduce with:

```
python3 crates/delve/assets/art-slots/cinder.crock-damaged/prepare.py --output /work/tmp/damage-prepared --check
python3 crates/delve/assets/art-slots/brasswake.keeper-damaged/import.py --check
```

The per-bank metadata retains original measured-mismatch/provenance warnings,
source rectangles, pivots, original hashes and rejected cells. The prepared
canvas is registration padding, not a new source design. Actor native sizes,
world feet and hit boxes stay fixed. Crock's matching base design is now active;
its wider transparent margins receive a common display fit that restores the
previous roughly 100px occupied body at the existing floor socket. No legacy
mouth, eyes or seam overlay is painted onto the delivered Crock drawings.

| Rival | Active supplemental coverage | Retained fallback |
| --- | --- | --- |
| Crock | Moderate/severe windup; strong recoil then recovery; severe idle; first/final collapse through existing clear flash | No separate punch/attack rule exists; those source cells remain proposals. Matching healthy base and cracks use `cinder.crock`. |
| Clasp | Moderate high/low tells, attacks and recoveries; severe idle/high tell/high attack; recoil and two collapse poses | Severe low attack/recovery lacks art. The rightward moderate high-hook pose is mirrored toward the knight. |
| Mossbell Keeper | Moderate root/crown tells, crown action and rest; severe idle/root/crown anticipation; recoil; two collapse frames driven by `hit` | Both long punches have amputated hands at hard source edges and remain unapproved; severe crown action keeps its base pose. New face sockets preserve the source amber eye through the existing green material grade. |
| Vat Keeper | Existing seven keys plus both moderate recoveries, recoil and first/final collapse | Severe rush and recovery absent. Recovery selects in the last 12 existing rest ticks, including distinct rush recovery. |
| Hoist Keeper | Moderate drop/sweep anticipation, action and separate recoveries; severe idle/drop anticipation; recoil and two collapse frames | Source severe raised-weight cell is anticipation, not a landing impact. Severe drop/sweep action and sweep tell retain base art. Pivots exclude the hanging weight. |
| Pennon Sentry | Moderate walk; severe limp; damaged anticipation/punch/recovery/flinch; severe last stand | Existing standing damage and defeat remain available; Captain's already integrated bank is unchanged. The source bank is mirrored toward the knight at left. Two extra idle/stagger drawings remain proposals. |

All selection uses existing HP/counter/crack thresholds and rule counters.
Native/cell colour variants, mirrors and display fits bake in existing caches.
Mossbell's three geometry reruns were reviewed again: raised paddles move their
pivots, thin roots cannot fill the full band at one scale, and the crown still
contracts horizontally when fitted into its 51x24 gate. Their exact old danger
art remains active; no danger gate or palette test was relaxed.

### Integration startup preparation

`warm_all()` also prepares the delivered dungeon banks for all six packs and
all registered boss effect/display banks. These use the same existing caches
as normal painting; no run is stepped or edited. `DelveClient::boot` selects
the host's art kit before preparation, so a v2 welcome prepares v2 resources.
The first valid client snapshot also calls `arena::prepare_entry` to compose its
actual exact/live scenery and actor caches before the first displayed frame. Composition
remains keyed to the actual room and camera; the run is never stepped.
Startup preparation time is separate from the first displayed-frame measurement.

The smoke scene ids 0–8 retain the game-shelf fixtures. Boss fixtures occupy
9–20, followed by the side hall (21) and Book Hop (22). `warm_entry` prepares
the selected mini-game or dungeon banks, after `scene` selects the art kit.
The slot reviewer imports the same shared cell painter as the cockpit and
retains all six per-game label painters.

The standard native/wasm smoke drivers mirror client preparation before their
first displayed-frame timer. `--unprepared` retains raw admission diagnostics;
`--entry ... --warm` reports startup preparation separately in fresh processes.
Smoke `state_hash(5)` projects only the six absent shelf sessions and the 30
new shelf tuning fields out of boss fixtures, so all pre-shelf wire fields can
be compared exactly against the boss branch's existing module. Normal wire
state (`state_hash(0)`) remains unchanged and is recorded separately.

### Entrance room registration

Only Latch's Smithy, Rafterwake's Workshop and Pennon's Muster use the ART-18
entrance backdrops. Cinder's Lab, Mossbell's Bathhouse and Brasswake's Tavern
retain their original room painters, including their floors and ambience.
Reproduce the three active backdrops with:

```
python3 scripts/entrance-art-import.py --slots latch rafterwake pennon --output /work/tmp/entrance-prepared
```

The active rooms retain the master's interior at its original coordinates,
BOX reduced to 384x224 and 768x448, with a 0.55 value / 0.65 saturation grade
and the existing palette. No median filter, repeated floor patch or shadow
flattening is applied. The collision-owned furniture, actual exits and whole
settlement title plaques come from the existing painter. Knights, prompts and
HUD occupy the solid terminal layer; the braille layer is for atmosphere.
The opaque full-sheet measurement and intentional sheet-edge contact warnings
remain in the import records. Source and measured metadata hashes are retained.

### Lanternward props (second wave)

The great brazier, chest, gate, crystal and beacon come from the
`/work/artifacts/art-prod-rt/slots` deliveries through the unchanged importer
(`--source-dir <delivery>/v01`; each has a local `request.json`, one registered
`shared-feet` bank on the prop's base). The beacon's tower is drawn again in
every delivered cell and its stone drifts between cells, so
`crates/delve/assets/art-slots/lanternward.beacon/prepare.py` locks the tower to the first
cell and lays each cell's own fire over it (every pixel is a delivered pixel),
cutting seven identical windows so one scale and one pixel grid register the
tower the same in every frame:

```
python3 -I crates/delve/assets/art-slots/lanternward.beacon/prepare.py --output /work/artifacts/monolith-private/art-import-lw/prepared/lanternward.beacon
python3 -I scripts/slot-art-import.py lanternward.beacon --source-dir /work/artifacts/monolith-private/art-import-lw/prepared/lanternward.beacon --check
```

Its flame frames measure 0.89-0.91 colour agreement (the shared palette has no
saturated orange) and were approved by eye.

### Lightgallop (third wave: the land, the road and the props)

Ten `/work/artifacts/art-prod-rt/slots/lightgallop.*` deliveries went through the
unchanged importer (`--source-dir <prepared copy>`, each slot with a local
`request.json`). They came without a `source-sheet.json`, so
`crates/delve/assets/lightgallop/art-import.py` measures the pieces from each sheet's alpha
(projection gaps; the three layers and the road are one piece, the whole sheet),
merges the crate's two groups of scattered bits, gives each piece a reviewed
pivot, lays a byte-identical copy of the sheet and its metadata in
`/work/artifacts/monolith-private/art-import-lg/prepared/<slot>`, patches the
reviewed map and runs the importer (`--check` re-verifies everything, maps,
provenance and native PNGs):

```
python3 -I crates/delve/assets/lightgallop/art-import.py --all [--check]
```

| Slot | What the import is |
|---|---|
| layer-far, -mid, -near | 192x96, one frame each: the whole 1774x887 strip reduced by 0.10823 with no crop (so the transparent sky above the treeline and the fence is kept). Seams invisible (the step across the join is below the strips' own column steps). The far strip carries a reviewed ink grade (`192934` and `263b40` to `22303a`, no pixel moves) in a separate commit. |
| road | 64x48 from the delivery's keyed copy; the lip is row 9, the blades rows 4 to 8. |
| bell | 7 frames at one scale (yoke 41x52 in the 48x72 canvas, registered on the yoke's centre and foot); the sparkles of the peak swing set the scale. |
| hay | 5 frames registered by the bale (scale from the rest bale alone): the burst frames reach above the 22 px canvas and lose their highest straw. |
| post | 5 frames at the natural 0.475 wide:tall: its canvas is 26x56 (`slots.json`), anchored on the whole post. |
| crate | 6 frames; the three intact boxes set the scale; the burst and scatter are registered by their centres. |
| bramble | 2 frames at one scale (the low tangle is 25x14, its own hitbox). |
| sign | imported, `runtime: false`: the stage prints the plank's words, the painted face is one arrow. |

The stage (`arena/lightgallop.rs`) paints a slot whole or not at all. The three
strips replace the code's bands only when all three are painted: each repeats
at the code's own share of the camera (16, 7 and 3) in whole stamps, the far
strip's top and bottom rows carry on as flat colour, and the near strip is
shaded to 60 percent (one pass, to the nearest ink, in its own commit). The
road stamps its tile along the curve the rules ride, its lip on the surface.
With any art missing the previous pictures are reproduced byte for byte (the
one exception is the lantern post's block, now as wide as its painted canvas).
The 75-ink snap turns indigo to teal-slate, moss to taupe and brown earth to
grey-brown; `colour_agreement` stays at or above 0.96 because the diagnostic
allows 60 RGB units per pixel, which dark colours never leave, so the shifts
were judged by eye (before/after: `art-import-lg/snap-before-after.png`).
