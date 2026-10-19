# Boss art pipeline

A `.boss` keeps its name, text, attack headers, grid envelope, collision radius
and feet anchor. Paint only its `art` section. Do not change the master inks.
The default kit is the first acceptance gate; v2 cannot excuse a weaker v1.

## Native concepts and conversion

Use image generation for each original boss and effect family. Request the
**actual logical sprite size**, not a painting to shrink later. The reusable
prompt in `scripts/boss-art.py::PROMPT` is available with:

```sh
TMPDIR=/work/tmp ANGEL_CARTRIDGES=/work/tmp/empty-cartridges \
python scripts/boss-art.py prompt 'furnace guardian' --size 28 23
```

The prompt requires `N x N px sprite, 4-tone hue-shifted ramps, 1 px coloured
outline, no dithering, flat neutral background`. Use the real rectangular
width/height when appropriate. One front/three-quarter subject, a big face,
two or three body masses, a clear limb gap, rounded connected pixel clusters.
No black contour, white, floor, haze, loose particles, gradients or speckle.
Effects need distinct projectile, landing motif and impact silhouettes.

Generation may still overdraw the requested grid. Reject that detail. Flood-key
the neutral border, crop, nearest-sample to the **existing** native dimensions,
and quantise to **at most ten ink slots per boss**, shared across its body
and effects. Existing inks are the starting point. Explicit `pigments` on a
frame can lock local colours the realm palette cannot express, such as plum
and ember `#c8501e` / core `#ffc860`. These bake into PNG caches; never change
the realm palette or its tests. Pure white is forbidden; a red channel of 255
in yellow is not white. Arrange them in overlapping 3–4 step hue-shifted material ramps,
e.g. slate `sSuU`, ember `PR@6`, warm rim `Ro@6`; walnut `bPot`, brass `PotT`,
teal `sSU2`; bark `bPRo`, cream `PotT`, rust `bR7o`, moss `mAY`.

```sh
TMPDIR=/work/tmp ANGEL_CARTRIDGES=/work/tmp/empty-cartridges \
python scripts/boss-art.py boss concept.png \
  --boss cockpit/assets/dungeon/bosses/example.boss \
  --output cockpit/assets/dungeon/bosses/example \
  --signature-lights lights.json --separation separation.json \
  --inks sSuUPRo@6 --ramps sSuU PRo@ Ro@6
```

`despeckle` replaces isolated colour pixels with connected neighbouring ink;
`contour` places a one-pixel inner selective edge in the local ramp's dark
tone, with its next tone on lit upper edges. Neither expands the silhouette.
The gate rejects more than ten inks, ragged rows, black/white and unreviewed
singletons. Save explicitly hand-placed eyes/highlights as coordinate pairs;
never grant an automatic exception to every surviving sample. Inspect and
merge small clustered texture islands too: the singleton gate is only a floor.
Hand-clean the sampled shapes into full native rows, saved in the recipe.
No blur, interpolation, dithering, or automatic detail restoration.

**Signature-light rule:** every boss has one or two named signature lights
(maw, lens, gills, seams). Hand-place these **after** quantisation, contour and
despeckle. Save exact `pixels` and locked `hues` in `signature_lights`;
`signature_rows` reapplies them last. They stay inside the existing alpha
silhouette and must never pass through cleanup again. These explicit pixels
are reviewed singleton exceptions, not an exemption for sampled noise.
Generic conversion requires `--signature-lights` with separate `v1`/`v2` lists.

**Theme gate:** the boss's theme must read in one glance at native 1× and in
the real terminal cells. A clean but generic sprite fails. **Originality
gate:** reject any famous commercial character or franchise likeness. A red
cap, white spots and face on a cream stem fails; Mother's dusky plum cap,
lilac spots, branching arms, moss and luminous gills establish her identity.
Record both human verdicts in the recipe's `review` fields; the script checks
that records and protected layers exist, not that taste is automatic.

The accepted pilot recipe is now the source of truth for every frame:

```sh
TMPDIR=/work/tmp ANGEL_CARTRIDGES=/work/tmp/empty-cartridges \
python scripts/boss-art.py recipe scripts/boss-art-pilots.json
```

It writes exact `.boss` grids, PNG previews and `.art` frames. Re-running it
must produce no diff. `frames` contains v1, v2, two projectiles, four warning
stages and five impacts. Other singleton exceptions include Cinderjaw's two hand-placed v1 eyes and one
retained silhouette pixel each on the Index's foot and Mother's root. Prompts and concepts are retained under the `boss-1b/` and `boss-1c/` review
directories in `/work/artifacts/boss-art-20261015/`.

## Combat art and geometry

OnceLock kits decode studies, rotate both projectile frames and bake landing
warnings once. Figure sizes and hit poses are cached. Stamping is integer
pixel work; do not quantise, rotate, resize or grade pixels per frame.
Projectiles are 9×9 native, with a coloured rim: a tailed ember slug, a turning
rune card, a light-cored spore puff. Impacts are 28×28, five explicit drawings:
flash, opening, expansion, breakup, fade. The original six-tick life remains.

Warnings belong on the ground at the actual destination, never on the boss
or its nameplate. A braking mine's discrete steering equation determines its
stopping point; a stone-blocked prediction is not drawn. Its warning fills
inward over its existing 51-tick (1.7 s) fuse. Held marked shots use their
actual spawn point, and beams use the authoritative reach, angle, sweep and
half-width. Point motifs must read at the hostile point shot's
11×11 knight-centre collision envelope (`HERO_RADIUS + 0.16`, rounded out).
Author at that size, or inspect the cached reduction of a larger study.
If a motif fails at 11 pixels, reduce it to two or three strokes. Cinderjaw's
approved crack reduction stays; Mother uses three cream/chartreuse branches
inside a plum ring with four stepped fills. Her ring must not read as green
sulphur gas. The Index uses 24×11 lane tiles with one teal baseline, spaced short ticks
at varied angles, and four lengthwise fill stages.

Ordinary moving projectiles have no landing area. Do not fabricate one or
call a source-charge circle a danger perimeter. The Index's present spiral,
curve/weave/seeker phases have no committed landing target. Crossfire's
18-tick (0.6 s) and beams' 21-tick (0.7 s) warnings predate this pass and stay
unchanged. Converting those attacks to charge-ups with at least 23 committed
ticks requires a separately authorised gameplay pass. No density, damage,
health, collision, cadence or projectile lifetime changes belong in this pass.
Appearance tags still follow emitted shots, split children and actual impacts;
old snapshots read the optional impact tag. Gameplay regressions compare
trajectories, damage, heroes and monsters with and without appearance tags.
Index-tagged held crossfire shots mark the stone-clipped travelling lane at
the point-shot hit width. Beam tells retain authoritative reach, sweep and
width. Cached teal baselines and short varied glyph ticks fill along lanes; no letterforms.
Crossfire fill follows `CROSS_MARK - wait`: path age does not advance during
the hold. This changes neither the emitter roster nor warning duration.

## Acceptance and review

Review every final asset at native 1× and 3× nearest. Reject painterly speckle,
mushy ramps, merged limbs, lost faces, dot projectiles, resized one-frame
splats and recoloured source rings. Compare **old / rejected pilot / new** in
the same room at 1× for every boss, both kits. Keep the old v1 for a boss if
the new one does not read better. Crypt bodies retain their alpha masks. The original-presence gate permits
small pilot body repairs for confirmed losses; Cinderjaw keeps its hunched
pose, lit maw and four broad plates.

Then inspect the real `paint_room_ui` half-block/braille buffer, both kits;
RGB and zoom sheets alone do not pass. The pilot bodies have cached, hue-preserving cell-path exposure up to 1.75×
before lighting and coverage, with a common gain bounded by a 242 peak.
Cinderjaw uses 1.75× after the original-presence review; the protected orange
core still owns its half-cell and is measured separately against v1.
The separately baked `v2-lights.png` exposes protected signature
pixels after room light: authored hues in RGB and a hue-preserving 242 peak
in cells. The exact cached furnace-core pigment owns its half-cell instead
of being averaged into the plate; other materials keep normal coverage. Hurt and rage flashes
retain their existing colours. This lets a warm emitter survive the half-cell
coverage sampler instead of losing to a broad plate. Independent channel
clipping washes warm accents grey. Body materials and scenery keep
their existing grade. Do not brighten the room
or loosen palette tests.
The ignored cockpit test `write_boss_art_terminal_shots` writes native RGB and
96×28 cell JSON for old/current sprites, resting and after 133 simulated ticks.
Set `ANGEL_BOSS_SHOTS=OUT/terminal` and `ANGEL_BOSS_OLD=OLD_BOSS_DIR`.
`scripts/boss-art-review.py` renders those actual cells and comparison sheets.
Every review sheet must be at most 1800 px wide and contain at most 12 views.

Build the host and browser bundle with the assigned target directories and
`TMPDIR=/work/tmp ANGEL_CARTRIDGES=/work/tmp/empty-cartridges`. Use only the
assigned host/CDP ports, headless `shot.mjs` / `render.mjs`, and both kits with
`--scene boss --boss ID --bots 3`. Measure before/after with
`crates/delve/smoke`; run the complete delve tests once at the end. Check the
headers byte-for-byte and never change ink tests merely to accept new art.

### Mandatory separation, light-form and player-stage gates

`recipe` validates the entire batch **before writing any asset**. `check` runs
those gates without writing. Each v2 frame records `separation`: the actual room
floor colour, head/cap and body material boxes (excluding the emitter), its
nearest-scaled room size/origin and an unobstructed floor sample. The outer ring
means **every** opaque pixel touching transparency in four directions. Its minimum
CIE L* must be at least 15 above the floor. Median head/cap and body L* must differ
by at least 15. A ramp step is 7.5 L*: two steps give visible native separation
without making broad surfaces compete with lights. Empty material boxes fail.
Never select a box containing just the eye to approve a dark head.

Signature light components larger than 2x2 (at least two pixels thick) cannot have a
complete axis-aligned four-sided rim (filled rectangles fail too). Interior
ink clusters at least 3x3 are checked too: a rounded sticker cannot hide a
rectangular core. One-pixel seams and thin gradient bands remain possible. Curves, cut
corners, notches and narrow connected seams retain their authored silhouettes.
Each component must touch an explicit `falloff` pixel in neighbouring opaque body
at exactly one pixel distance, at least 7.5 L* below its adjacent light. These
locked-hue pixels are applied after cleanup but excluded from the emissive layer.
They do not grant immunity to the shape gate. Isolated sparks need collars too.

Every effect core is checked on black: a four-connected cluster of two or more
pixels with **all RGB channels >=240** fails hypercontrast. A warm cream with
one bright channel is not near-white. Consecutive authored tells must differ.

The final gate uses **actual** `write_boss_art_terminal_shots` exports:

```sh
python scripts/boss-art.py review scripts/boss-art-pilots.json \
  --room-captures OUT/terminal --output OUT/review
```

Missing RGB/cell captures fail. This repeats the 15 L* separation measurement on
the nearest-scaled, room-lit sprite at its true position. It crops each of the
four wind-up stages at 3x beside native 1x and actual cell-buffer glyphs. Every
consecutive native pair must change by >=7.5 L* in at least one pixel **and**
change at least one terminal cell. This excludes lighting-only noise from the
RGB diff. The fifth fixture is release/impact, not a fifth authored tell stage.
The projectile and impact sheet includes a labelled actual-cell row for each
type; body sheets also show actual cells. Sheets stay below 1800 pixels and at
most twelve views. Stage diff counts and hypercontrast results go to
stdout; a failed measurement exits nonzero. Keep the log with the review.

The fixture coordinates are explicit acceptance evidence, not a general scene
finder. A new room/boss must export its true anchor, scaled size, clear material
boxes and effect positions; extend the fixture/crop layout before batch use.
These gates establish measurable floors. Human native room/cell/3x review of
character, shape, danger placement and originality is still required.

Boss concept conversion also requires `--separation`; it validates both kits
before replacing either. Effect conversion produces candidate studies; ship
only via a checked native recipe and the final actual-room review. The Index
review crops the **whole beam**, including all four advancing fill quarters;
a crop inside the first already-filled quarter cannot demonstrate progression.

Cinderjaw also compares the actual orange core against the unchanged v1
orange core in RGB and both cell halves (orange has R/G >=1.25 and G/B >=1.5;
yellow highlights are excluded). Either path becoming dimmer fails review.

The projectile fixture sits away from the Index beam. Its crossing with the
horizontal lane and the legacy projectile rows are excluded from the vertical
beam diff, so another animated object cannot pass a frozen tell. Empty effect
crops/cell rows fail. `review-layout.json` records the fixture's projectile
crop; legacy pilot captures use their original crop when that file is absent.

## Crypt / wax batch

`scripts/boss-art-crypt.json` owns Waxen Warden, Bone Choir and Tithe Marshal.
It preserves each original grid's alpha mask and feet, and uses that exact
mask doubled for the v2 study. Run it through the same recipe/check/review
commands as the pilots. `boss-art-crypt-concepts.py CONCEPTS OUTPUT` retains
nearest/palette candidates; the recipe contains the subsequent native cleanup.
`boss-art-crypt-review.py OUT --before BEFORE` adds both kits' native before/after
rooms and actual cells, plus all five impact frames in terminal cells.
It also checks and labels every default-kit tell stage and effect cell row.

These three ordinary attack rosters contain travelling volleys, not landing
areas. Wax's reaim picks a knight only when it releases. Their four tell
drawings therefore dress the existing 36-tick rage ring, with its actual
eighteen base directions and three-step nominal safe sector. The preview also
reads existing party-fan offsets and the ordinary shots that consume fan slots,
so it never promises that the nominal gap is clear of fanned shots. Short strokes
and themed glyphs show those directions at their true angle, without the old
vertical squash. Stone clips the preview. They are direction previews, not
circular area boundaries. Counts, cooldowns, target selection and warning
duration stay unchanged; ordinary volleys gain no invented wind-up.

The real cockpit fixture records each boss's body, ring, flying projectile,
impact and central exclusion boxes in `review-layout.json`. Ring-stage diffs
exclude the central boss so its rage flash cannot pass a frozen warning.
The smoke fixtures include all three bosses, including the Marshal's checked
route-leader file, so both native and wasm costs cover this batch directly.
Pass `--rage` to the native smoke binary or `smoke.mjs` to measure these three
bosses through their active warning cycle as well as the normal full-health run.

## Reliquary / cavern batch

`scripts/boss-art-reliquary.json` owns Crypt Tax Keeper, Crypt Relic Liege and
Iron Tusk Chief. Their source grids remain 21x25, 22x32 and 23x27, including
every opaque pixel and foot. V2 doubles those masks exactly. The chief keeps
the normal chief stature floor of 24 pixels; its 27px source is never shrunk.
Both fixtures and the live host admit the three authoritative checked files.

Tax Keeper carries a brass coffer under a bone mask; amber crescent eyes and
the notched coffer slit identify its lights. Its shot is a double-ward key,
and its impact breaks open lock jaws. Liege's crown and caged turquoise
heart accompany a winged ossuary clasp and unfolding reliquary ribs. Chief's
green torso and arms, domed steel helmet, curved bone horns and hooked
walnut pick carry a recessed amber visor; the turning pick tooth breaks
into lifted, bevelled ore wedges.
No projectile silhouette repeats a previous kit.

Chief's aimed, lead and gap-wall volleys travel. Gap-wall spawns two offset
moving walls, with a delayed follower; it has no marked landing area or beam
corridor. Tax Keeper and Liege also have no committed ordinary landing AoE.
Their four authored glyphs therefore dress only the existing 36-tick rage
ring using `warning_target` and `ring_preview`. Target visibility, decoys,
party-fan slots, stone clipping and the nominal safe sector retain their
existing meaning. No counts, cooldowns or other simulation numbers change.

Run `boss-art-reliquary-concepts.py` for retained nearest/palette candidates,
then the common recipe/check/review commands. `boss-art-reliquary-review.py`
adds native before/after rooms, all actual-cell stages and full impact crops.
`check-boss-art-reliquary.py OUT` verifies approved bytes, headers, dimensions,
alpha masks, feet, nine projectile silhouettes and protected room/cell exports
against `OUT/protected-hashes.json` and `OUT/before/`. Prompts, source concepts,
rejected candidates and native cleanup evidence live in the BOSS-3 review folder.

The smoke crate's `parity` binary and `parity.mjs` record the timer's same
60-tick horizon with independent wire, actor, shot-physics and count hashes.
`plain_art` supplies an appearance-free control; these diagnostics run outside
timed frames. Use `cargo run --release -p angel-delve-smoke --bin parity -- ID`
and `node crates/delve/smoke/parity.mjs PATH_TO_SMOKE_WASM ID` when a frame
checksum differs across architectures. Iron Tusk's rage release has an existing
native/Wasm difference at tick 35: 48 versus 49 shots in this fixture. The same
physics/count difference occurs with custom art disabled; enemy and hero
state remain exact. Keep that simulation issue separate from art acceptance.

## No weaker than the original

`check` and `review` require `--room-captures` and fail if either kit loses
body presence against **7d711a03**, the pre-pilot dev art. A successful `recipe`
only stages assets for rendering; it is not approval of an art build. Export
that revision's `.boss` files with `git show 7d711a03:<path>` into
`ANGEL_BOSS_OLD`, then run the ignored cockpit test
`write_boss_art_terminal_shots` with `ANGEL_BOSS_SHOTS=OUT/terminal`. Its `old`
fixture changes the boss ID to bypass the new cached study and paints the old
grid through the original draw fallback. Never use the previous batch as the
reference. Built-in figures without grids need an explicit fixture of their
old draw code before they can be approved; a new study is not a reference.

For each kit, at resting tick 0 in the same room, the gate samples **every opaque
body pixel**, including limbs and equipment, at its actual 1x room size and
anchor. Old and new use their own alpha masks, scaled with the runtime's integer
floor sampler. It measures (1) median CIE L*, and (2) silhouette presence: the
number of those pixels at least **15 L*** above the unobstructed floor sample.
Both new values must be **>= the original** independently; there is no percentage
allowance and losing even one presence pixel fails. `1e-6` only handles numeric
equality. Signature pixels are included, but a small bright eye cannot raise a
body median. Hue is unrestricted. Floor contrast/head separation, alpha/feet,
light-shape and human originality checks remain separate requirements.

The same two measurements run on the **actual 96x28 emitted cells**. Each native
body pixel maps to the glyph's 2x4 logical coverage: upper/lower half-blocks use
their fg/bg half, braille uses its real dot bits, and spaces use only bg. An
unused bright foreground cannot pass a sparse or blank glyph. Coverage is
weighted by the native pixels it represents, rather than by a font's antialiasing
or counting an entire cell because one dot is bright. HUD text in the body
fixture fails. Floor samples are decoded through the same cell path.

Run both recipes as one JSON array to audit the six shipped bosses in this
worktree. `review` writes one `*-original.png` per boss: before/after in both
kits, each with 4x and native 1x room crops and actual cells (12 views, 1520px).
Keep the `ORIGINAL` output lines with the review. The initial six-boss output is retained with each review; measured regressions must be checked by eye before any pilot exception.

The Crypt send-back uses local body PNG pigments in **both** kits; `.boss`
headers and gameplay rows retain their meaning. The cached v2 material floor
keeps authored ivory/tallow values after room shading, with local light allowed
to increase them. Default bodies also read the cached body PNG, so a local
pigment cannot silently disappear into the shared grid ink. Neither mechanism
adds glow to the floor or per-pixel floating-point work. Crypt masks remain
exact; documented pilot regressions may restore lost body coverage while
retaining native dimensions, the bottom two foot rows and all gameplay anchors.

Crypt tell glyphs grow through 5/7/9/11-pixel extents in their unchanged 11x11
canvases; the existing cached 9x9 ring reduction is checked in actual cells.
Radial strokes stay one pixel wide at half value. Projectile phases differ by
at least six source pixels, and late wax/coin impacts use droplets/wedges.

## Reliquary send-back cell gates

`check-boss-art-sendback.py OUT/terminal` measures the actual emitted cell
buffers, with effect-free buffers from the same fixture and tick. Projectile
frames must change at least two occupied half-block cells; colour-only changes
do not count. Each consecutive warning stage must change at least 30% of the
active union's decoded glyph-coverage masks outside the recorded boss/HUD exclusion. Liege impact
frames 1 and 2 must cover at least sixteen cells spanning four columns and
four rows in both kits. Authored warning inks stay at or below CIE L* 80.

`room-fixtures.json` records the authoritative dungeon for each old/current
pair, and the exporter asserts equality. The Chief's cavern now uses its
native static cavern scenery, baked at integer 2x in v2, for both original
and current figures. The original-review ID remains v2 eligible in testing;
it only bypasses the new body study. This prevents the previous comparison
between a fallback cave and the generic v2 Crypt tombstones. Other approved
boss scenes retain their existing scenery.

The combined pre-repair BOSS-3 audit has one original-presence failure:
Tax Keeper v1 cell median 55.53 L* versus original 65.64. Other original
presence measurements and existing native/room gates pass. The stricter
send-back audit rejects all six projectile frame pairs, all eighteen warning
transitions, eight over-bright authored tell frames, and Liege impact 1 in
both kits: 34 failures. Retained evidence is in the BOSS-3B review folder.
Passing the original floor does not approve the Chief's farmer silhouette
or the bodies' pale cell materials; these remain independent visual failures.

The BOSS-3B repair uses one four-tone plum hood in both kits, a native 5x4
Tax coffer with two coins, and notches large enough to remain open after cell
reduction. Liege keeps wine robes with deep two-pixel sockets and teal
pinpoints. Chief keeps the original alpha/feet, with a narrow domed steel
helm, curved bone horns, a broad green torso and arms, and a hooked walnut
pick. The v2 material cache retains their authored values; restrained cell
exposure keeps wine and green material saturated rather than pale.

Each projectile changes its actual cell silhouette: the key turns a quarter
turn, the clasp opens from five to nine pixels, and the steel pick tooth
turns its hooked bite. All impacts are separate authored stages; the Liege's
opening ribs occupy four cell rows. Tax impact frames 1-2 additionally need
an open notched ward in the decoded cell buffer; bars and filled squares fail.

Four cached rotated warning tiles progress from single-pixel dotted arcs to
three-pixel hooks, five-pixel inward hooks, and a joined final band. Party-fan
rays also progress through visible one/three/five/eight-pixel strokes on their
real angles; they cannot hide inside the nominal open lane. The final band
joins only consecutive primary directions with unobstructed reach. All
raster work uses integer stamping or filling; rotation and material exposure
are baked in the kit caches. Original ring release, targeting, party-fan slots,
stone clipping and all simulation numbers remain unchanged.


## Crypt body detail pass

The Crypt recipe also retains the body-only BOSS-2C cleanup. Tallow has
curved melt channels, recessed lids and overlapping lower folds; the Choir
has shaded forehead/jaw shelves, deep cool sockets and jointed bones; the
Marshal has bronze bevels, paired rivet clusters and a four-value blue cloth
ramp. Both kits share material pigments. The larger kit refines lids, socket
interiors and metal edges on its native grid. All signature-light records,
projectile/tell/impact frames, separation boxes, masks and foot pixels remain
exact. No renderer changes or master-ink changes are required.

`scripts/boss-art-crypt-detail-concepts.py CONCEPTS OUTPUT` keeps the three
nearest/palette candidates before hand cleanup; `boss-art-crypt.json` owns
the final rows. Concepts, prompts, rejected passes and six identical-room
comparison sheets live in `/work/artifacts/boss-art-20261015/boss-2c/`. Each
sheet compares ORIGINAL / BOSS-2B / NEW at 3x, native 1x and actual cells,
with one kit per sheet (nine views). The original-presence, native/room,
light-form and player-stage gates retain their existing thresholds.

## Identity props before drawing

Before every later boss pass, name **two or three identity props** from the
boss's name and unchanged `.boss` `text`. Iron Tusk needs jaw tusks and iron;
Tax Keeper needs the treasure chest; Relic Liege needs a cool bone skull
under a separate gold crown. Record these before drawing in the task brief.

Fill `review.identity_props` in the recipe after inspection: `props`, exact
`source_text`, `compared_original_1x`, `compared_original_cells`,
`inspected_cells_2x`, and two `evidence` paths (one per kit). All three review
checkboxes must be true. Compare ORIGINAL / previous pass / NEW at native 1x
and in actual emitted cells **in the identical room**, then personally inspect
a **2x nearest-enlarged actual-cell crop**. Identity props must remain visible
in cells. Passing presence or silhouette metrics cannot substitute for this.
The check/review command validates the filled checklist, mandatorily for the
identity-repair trio. Untouched archived recipes keep their existing review;
when another batch is next edited it must add the same checklist.

Keep a distinct effect geometry per boss: coin dots and key-ward notches,
reliquary thorns/ribs, and rising triangular ore shards. Retain dotted seed /
three-pixel hooks / five-pixel inward hooks / joined band progression and all
existing stage-difference floors. The final joined band has a dark core and
lighter outer edge; cached authored motifs remain visible above it. Assigned
projectiles now use 24x24 visual canvases to retain their turning / unfolding
silhouettes across at least 3x3 actual cells. Collision radii and anchors are
unchanged. Other approved projectile canvases remain 9x9.

`scripts/boss-art-identity.py` reproduces the native cleanup and keeps rejected
nearest/palette concept samples. The ten-ink gate still applies; small material
shades share slots where needed. Actual room and cell review decides whether
the resulting identity and hue separation succeed.
The clasp uses hollow ribs and the pick tooth keeps a narrow hooked profile,
so overlapping volleys leave more of the room and boss visible. The outer
24x24 canvas includes clear space; its ink does not fill that envelope.

## Powder, harness and debt batch

`scripts/boss-art-pit-debt.json` owns Red Pick Chief (26x24), the Foreman
(28x23) and Late-Fee Leviathan (32x26). The converter / native cleanup is
`boss-art-pit-debt.py`; it retains nearest/palette candidates alongside the
concepts. Both kits preserve the exact source alpha and double it for the
study. Red Pick keeps the 24px chief floor; the Foreman's inherited 23px grid
still receives its original stature enlargement. Each identity checklist
names the unchanged text and two inspected identical-room cell sheets.

Red Pick retains the red mining pick, powder keg and bandolier. Its turning
cartridge has a hooked fuse and a dark binding; the impact has three unequal
blunt powder lobes. Four mine glyphs progress through fuse points, horizontal
caps, opposed vertical jaws and a closed charge. All stay inside the true
11px point collision envelope, use `mine_landing`, reject stone-blocked
predictions and retain the original 51-tick fuse.

The Foreman's helmet lamp, leather pit-pony harness and two descending whips
remain visible. His projectile opens a leather loop into a long hooked lash;
the impact splits into two asymmetric forked whip cracks. Four distinct
bridle / lash glyphs rotate on the existing rage-ring directions, including
party-fan slots and the original safe sector. Ordinary aimed/fan/gap-wall/
bounce/lead shots gain no fabricated destination or beam. Gap-wall's
unmarked 24-tick follower stays unmarked.

Leviathan carries a bookbound maw, wet teal body / tentacles and a brass
retrieval anchor. Its grapnel turns, and five impacts raise hooked tentacles.
`Ravage.boss_art` is optional appearance data, defaulting to none in old
snapshots; it survives after the caster moves. The existing cast anchor,
ring count, 2.2-unit spacing, BAND half-width, clear-lane heading / width and
36-tick first warning own the art. Cached annulus pixels charge together,
then each ring bursts on its original beat. Integer lane-edge tests leave
the actual lane clear; stone clips the pixels. Geometry is baked once for
the existing eight possible radii, rather than cached again for each angle.
The Laser uses a separate true-reach / BEAM_HALF corridor and keeps its
existing 21-tick warning. No attack numbers or physics change.

`boss-art-pit-debt-review.py OUT` makes per-kit ORIGINAL / NEW 3x, native 1x,
1x cells and 2x cell sheets, plus every effect stage. `check-boss-art-pit-debt.py
OUT` retains the original-presence / cell-shape thresholds, checks all authored
ink below L*80, protected bytes / captures and distinct effect families.
Pass `--negative-controls` to reject repeated authored stages, frozen cell
warnings (with boss/HUD exclusions) and a connected near-white core.

The cockpit's two `write_boss_art_` exporters must run with
`--ignored --nocapture --test-threads=1`: their art setting is shared. The
area exporter captures all five true annulus beats and the five burst stages.
The smoke roster now includes all twelve kits; `--ravage` stages the real
Leviathan cast for native/Wasm active-area frame costs. Normal / rage modes
and appearance-free parity controls retain their original horizons.

Normal and staged-Ravage native/Wasm frame pairs match. The rage fixture
differs at tick 35 for the Foreman (81 versus 82 shots) and Iron Tusk
(48 versus 49). The same shot-physics/count differences occur with custom
art disabled; enemy and hero hashes stay exact. Keep that simulation issue
separate from art review, and do not tune art or gameplay to force parity.

## Preserve colour identity before adding detail

KEEP THE ORIGINAL'S HUE FAMILIES AND VALUE CONTRAST (face, prop and accent
colours) AND CHANGE FORM, NOT PALETTE. This applies to every later pass.
Judge the **2x actual-cell crop first**, in both kits and the identical room:
the face and each identity prop must be a distinct colour block. Then inspect
native 1x and 3x. Fine detail cannot repair a merged or recoloured identity.
The identity checklist records `preserved_original_hues`,
`preserved_original_value_contrast`, `judged_cells_2x_first` and
`identity_colour_blocks_distinct`, alongside the existing review evidence.
Changed recipes and all new bosses require these records.

V2 effects must match V1 brightness; exposure is baked into the assigned
OnceLock effect caches with a common hue-preserving gain. Impacts grow, peak,
then break apart: all five native silhouettes differ, and consecutive frames
must differ in the **actual decoded cell silhouette**. Colour-only changes
fail. The pit/debt checker includes a frozen-impact negative control in both
kits. Final Ravage annuli retain two clean teal bands without speckle; their
true committed radii, lane and burst beats remain authoritative.
The cached enlarged effect pigments map back to their native-kit inks when
they own a terminal half-cell; sparse coverage cannot dim their tells or
impacts. Only exact local pigments match. Pick steel/red haft and brown whips
also retain exact pigment ownership. Other materials keep normal coverage.
The requested #d8cfb0 ivory exceeds the unchanged L*80 authored-ink gate;
this pass uses #cbc2a5 ivory (L*78.5) for teeth over the wine gullet. A thin
dark brown inner book lip retains body presence without closing the maw.
Live browser review also rejects a travelling effect whose bright overlapping
inks wash out its caster. The Foreman's projectile uses dark steel-blue
#344f62 / #577c90; brighter blue belongs to its short crack impacts and tells.
Its canvas, collision, shot counts and timing remain unchanged.
