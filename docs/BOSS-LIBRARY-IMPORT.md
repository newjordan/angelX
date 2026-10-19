# Boss library integration and display policy

The read-only library owns the designs. The normal build uses 24 of the 26 checked
overhead bodies: the earlier seven plus 17 from BOSS-ROLL-2. Grinwick and Slag
Sovereign retain their old bodies pending the measured corrections below. `boss-library-review`
adds size trials, rather than controlling production activation. Both browser
and terminal still default to v1. No onboarding files or source-library files
are modified. The retained lossless crops and their provenance records
include complete-sheet/crop SHA-256 and unchanged `.boss` definition hashes.

## Display, feet and simulation

`scripts/boss-library-registry.json` is the display authority. Each entry names
its class, independent v1/v2 envelopes, bottom-centre registration, identity
props, trial envelopes and material grades. The importer and renderer read the
same data; a mismatched PNG size fails at cache construction. Each camera bank
is bottom/centre registered before BOX sampling, binary alpha at 128 and shared
75-ink fitting. Aspect ratio is retained; transparent space belongs to the
source silhouette. No silhouette painting, axis stretching or new ink bank.

| Class / pilot | INTEG-0 v1 → selected v1 | INTEG-0 v2 → selected v2 | Selected envelope / canvas |
| --- | --- | --- | --- |
| Chief / Iron Tusk | 23×27 →46×54 | 46×54 →82×96 | v1 2.89%; v2 2.29% |
| Guardian / Cinderjaw | 29×24 →64×54 | 58×48 →116×96 | v1 4.02%; v2 3.24% |
| Guardian / Mother | 30×24 →68×54 | 60×48 →120×96 | v1 4.27%; v2 3.35% |

These are maximum display rectangles including source negative space, not
hurtboxes. The common 54px v1 /96-sample v2 height ceiling has per-boss widths:
Tusk's spear consumes height while the guardians retain broader body mass.
Future imports start at these class envelopes, preserve their own aspect and
must repeat the same room/cell review; this is not automatic approval.

`stature::boss_figure` remains the compatibility/sizing reference for unassigned
bosses. Library figures bypass radius/stature enlargement. Their foot point is
unchanged: v1 left=x−w/2, top=feet−h+1; v2 left=round(2x)−w/2,
top=round(2feet)−h+2. Bars, names and status marks use the selected kit's display
height (v2 logical height=ceil(h/2)); shadows follow display width. Native
bodies, breath, anticipation, hurt/rage variants, logical masks and shadows are
cached in OnceLocks. Live stamps use integer pixels; no new per-pixel float
work. Two source poses remain two poses: the one-sample breathing compression
is not a full acting/attack/recovery/defeat animation.

Collision radii, aggro, stone/path clipping, `.boss` headers, damage, health,
attack counts/cadences and trajectories remain simulation-owned. The existing
36-tick rage ring and 24-tick arms anticipation keep their exact geometry and
release. Point tells still use HERO_RADIUS+0.16, held shots their actual point,
and mines their discrete braking endpoint. Spawn marks use the simulation
radius rather than the enlarged drawing. Knights and shots draw over bodies;
warning/name/bar layers remain over the body. The painted upper body is
intentionally outside the foot-centred hurtbox; it does not become a new target
surface. A wide decorative shadow is not a danger boundary.

## Size study and rejected trials

Artifacts: `/work/artifacts/boss-art-20261015/integ-1/`; selected and trial
records in `study/records.json`. All rooms use the existing simulation fixture,
seat1 plus three extra seats, tick 95/rest and tick 228/shots in flight. The
headless hosts additionally run three moving/firing native bots beside a browser
seat. RGB is native 384×224 or 768×448; terminal evidence is the cockpit's actual
half-block/braille painter at 96×28, with 2× crops for identity review.

V1 trials: Tusk29×34,40×47,52×61,58×68; Cinder36×30,51×42,65×54,72×60;
Mother38×30,52×42,68×54,75×60. V2 trials: Tusk61×72,82×96,109×128;
Cinder87×72,116×96,155×128; Mother90×72,120×96,160×128.

Rejected the smallest trials: faces, jaws, gills and equipment merge; even 72
v2 samples are marginal in the room. Rejected 128 v2 and the 2.5× v1 extremes:
more surface detail, but excess upper-room/label occupancy, more overlap with
warnings and misleading upper-body target area. Selected 54/96 keeps the spear,
buckler, furnace/jaw, cap/gills and root masses readable while the party and
shots remain distinct in busy rooms (13/70/75 real shots at tick 228).

Native faces/equipment improve substantially. At96×28 cells, coarse equipment
and material blocks read; individual eyes, tusk caps and gill striations remain
below this buffer's grain. Increasing sample density alone cannot recover
those details. Do not claim full facial fidelity in cells or enlarge further
to chase it. The source side bank is retained for a separate camera/socket
registration task; v2 currently falls back in side-on halls.

Rejected ungraded larger stamps: brightness still fell below the original.
Rejected an aggressive uniform cell grade: pale furnace metal and acid-green
moss passed numbers but lost material contrast. The final cached grade protects
the darkest source contours; source pixels/alpha in the imported PNGs remain
unchanged. Renderer grades are bounded at peak228 and recorded independently
for native RGB and terminal reduction in the registry. No exposure is computed
per live pixel. Review the native and cell images, not only the gate result.

## Gates: an explicit change of coverage policy

The INTEG-0 gate had 21 failures. It compared absolute visible-pixel counts at
old envelopes, penalising source negative space. For library IDs only,
`boss-art.py::original_gates` dispatches to the display gate. Other IDs retain
the original absolute-area and median comparison, including its one-pixel
regression rejection. No shared palette or fidelity threshold is relaxed.

Library gate definition:

- VALUE-1 replaces the old sprite's absolute median with local floor contrast.
  In both native RGB and decoded cells, median **L*(body pixel) − L*(graded
  floor at that same pixel)** must be at least **15 L***. Check rest and
  anticipation independently. The paired floor retains the same generated
  guardian room, pack, camera, renderer route, lighting and barred state.
- Retain the rest-pose presence gate: at least **80% of source-alpha pixels**
  must be at least **15 L*** above their local floor. This is algebraically
  the previous source-alpha/display-normalised requirement. Anticipation
  coverage is also reported, but was never part of this rest-pose area gate.
- Require a silhouette in both poses: at least **50% of the outer two logical
  pixels** of opaque source alpha must differ from the floor by **10 L***
  in either direction. A dark contour against a lighter floor and a light
  inner rim against a dark floor both count. The two-pixel band includes
  thick ink and its rim; the 10 L* threshold allows that band less separation
  than the 15 L* body, while the majority requirement rejects an isolated
  eye/glint standing in for an outline. Names, bars and unused cell foreground
  colours are excluded. Decode emitted half-block/braille coverage exactly.
- Source fidelity stays silhouette IoU≥0.98, body colour agreement≥0.90 and
  boundary agreement≥0.85 within RGB distance60; compare against the sampled
  unquantized source, before the documented rendering grade. Binary alpha and
  membership in the 75 shared inks are exact. `--check` rebuilds byte-identically.
- Identity review requires both kits, actual2× cell crops, named props and
  SHA-256-bound evidence. This human check records coarse prop readability,
  with the facial-detail limitation above; numerical gates do not certify art.
- Existing effect PNGs remain byte-exact and their native gates still run.
  The cockpit exporter also compares identical authoritative room fixtures.

Negative controls reject a body matching its local floor, insufficient
normalised presence, a bright core with an invisible boundary, deleted spear,
painted-over materials and palette/alpha drift.
The previous non-library original-presence regression tests still apply.

Final production output: **21 INTEG-0 failures → 0 library display failures**.
All 54 selected/trial imports pass: minimum silhouette IoU 1.0000, colour
agreement 0.9299 and edge agreement 0.9920. The unchanged shared palette has
75 inks. Median values below are original → selected CIE L*:

| Pilot | v1 room | v1 cells | v2 room | v2 cells |
| --- | --- | --- | --- | --- |
| Iron Tusk | 39.24 → 44.93 | 55.53 → 61.79 | 39.24 → 40.20 | 46.85 → 51.99 |
| Cinderjaw | 40.88 → 41.42 | 77.15 → 82.14 | 40.88 → 43.54 | 61.29 → 61.77 |
| Mother | 34.96 → 40.88 | 82.24 → 84.58 | 34.96 → 40.88 | 61.73 → 65.35 |

Tusk's retained v1 warning stages change 32.9%, 55.8% and 32.2% of active glyph
shapes, above the unchanged 30% floor. Existing point-mark phase distinctions
still merge in some v1 Cinderjaw/Mother cells; the baseline has the same
collapse. The gate reports that limitation and rejects a newly introduced
collapse. Replacement effect deliveries must address it rather than inherit
visual approval from an unchanged source file.

## Effect delivery slots

`scripts/boss-effect-slots.json` supplies all twelve registered bosses' 2 flight,
4 tell and 5 impact file slots. `build.rs` embeds the selected filenames and
tracks each input, so an artist's delivered PNG can replace a slot without Rust
edits. The fixed phase slots reflect existing simulation timing; they do not
create new attacks. Decode, four turns and ring orientations are cached. The
existing 11×11 point-tell clip, stone-clipped outgoing rays and impact lifetime
remain authoritative. `docs/ART-REQUESTS.md`, Bosses, specifies nine missing
pilot effects, exact sizes, sockets, delivery format and rule-derived timings.
Seven delivered banks now replace the older effects, as detailed in BOSSFX-1
below. Other banks retain their earlier files until their own deliveries and
readability review.

## Reproduce

Set TMPDIR=/work/tmp, ANGEL_CARTRIDGES=/work/tmp/empty-cartridges and
UV_CACHE_DIR=/work/tmp/uv-cache for every build/test. Use the assigned target dirs.

```
uv run --no-project --with Pillow scripts/boss-library-import.py --check
uv run --no-project --with Pillow scripts/test-boss-library-import.py
# Actual cockpit painter, without linking unrelated application services:
ANGEL_LIBRARY_STUDY=<out>/study cargo test --manifest-path scripts/boss-display-review/Cargo.toml write_display_study -- --ignored --nocapture
uv run --no-project --with Pillow scripts/boss-display-review.py <out>
uv run --no-project --with Pillow scripts/check-boss-library.py <out>/after/terminal --before <integ-0>/before/terminal --identity <out>/identity-review.json --metrics <out>/gate-metrics.json
# Additional trials in review builds, native or exported wasm:
boss_library_review(0) # selected; 1..4 select the registry trials (v2 has 3)
```

The production cockpit exporter remains `write_boss_art_terminal_shots`, with
ANGEL_BOSS_SHOTS and ANGEL_BOSS_OLD set to output and immutable original grids.
The isolated exporter uses the very same cockpit cell painter, and the v1
frame_for(96,56) → half-block route; it omits only the separate text HUD.
Headless captures use dev-host ports 8940–8949 and CDP 9040–9049, 3 bots and both
kits. No visible browser session is used.

## Default-kit decision evidence and gaps

The default is unchanged. `coverage-1.png` compares kingdom and crypt, and
`coverage-2.png` compares the side-on hall and Book Hop, in both kits and cells.
Performance tables and browser bot samples are in the task report/artifacts.
Kingdom v2 improves depth, but generic repeated houses lose building identity;
its foliage/ground speckle is busy. Crypt v2 improves knight/monster form and
shot separation at native size; 96×28 reduction still averages away faces.
Hall and Book Hop are v1 fallbacks: selecting v2 only enlarges their existing
pixels, and their actual cell buffers are identical between kits.

Before considering browser-default v2, deliver/register source-directed
hero/knight anatomy and complete class/gear/seat/action states; replace the
four procedural monster studies and fill the rest of the bestiary; add
recognisable building/prop families; cover fungal, forge, side-on and other
missing tile/camera banks; update the many v1 mini-game/room surfaces and UI
chrome/tray/panels; finish full boss animation and the requested VFX. Keep
terminal density and browser sample density as separate decisions.

Live headless browser samples use a playing seat plus three moving/firing bots.
Observed WASM p95 milliseconds (v1 / v2): kingdom 3.7 / 15.4; crypt 3.5 / 9.3;
side hall 12.1 / 13.5; Book Hop 5.4 / 10.5. These captures overlapped compilation,
so they are evidence of the live path under load, not the clean comparison.
The kingdom v2 sample reached only 51 FPS under that load. Raw counters,
including presenter cost, are retained in `live/*.json` and
`default-kit-browser.json`; the paired unloaded smoke measurements below use
native canvases and four seats without browser/network scheduling.


The final normal-build `crates/delve/smoke` WASM pair uses the immutable original
fixture on both sides, 120 warm-up frames and 360 measured live frames per
case, stepping every other measured frame. No compilation ran during timing.
The three-boss p95 comparison is in milliseconds:

| Pilot | v1 before → after | v2 before → after |
| --- | --- | --- |
| Iron Tusk | 1.89 → 1.85 | 5.96 → 5.47 |
| Cinderjaw | 1.99 → 1.97 | 6.08 → 5.40 |
| Mother | 2.27 → 1.63 | 5.40 → 6.46 |

Four-seat production fixtures, p50 / p95 milliseconds:

| Scene | v1 | v2 |
| --- | --- | --- |
| Kingdom | 0.61 / 0.69 | 8.02 / 13.28 |
| Crypt fight | 0.78 / 1.99 | 5.30 / 6.27 |
| Side-on hall | 0.58 / 6.30 | 2.77 / 5.94 |
| Book Hop | 1.01 / 2.20 | 4.06 / 5.98 |

These are machine-local samples, not a universal FPS guarantee. The raw cold,
median, p95 and maximum samples are in `frame-cost.json`; the v2 Mother sample
adds 1.06 ms at p95. Nine thousand full-wire/enemy/projectile/player/count hash
comparisons remain equal to the original across both kits and 300 stepped ticks
per pilot, with live rendering at every tick. All 414 unassigned-boss RGB/cell
captures remain byte-identical. The policy changes drawing and layout only.

## Exact next rollout groups

Apply the selected54/96 class-height policy as a starting envelope, fitting
source aspect without distortion, per-boss data, unchanged feet/hitboxes and
these same gates. Capture both kits with 3 bots before activating each batch.

1. Registered archive/crypt: Index, Waxen Warden, Bone Choir, Tithe Marshal.
2. Registered reliquary/mines: Tax Keeper, Relic Liege, Red Pick, Foreman,
   Late-Fee Leviathan. Preserve bespoke Ravage/laser/lane geometry.
3. Remaining checked fungal: Rootwarden, Spore Crown; Hellforge: Ash Castellan,
   Chain Keeper, Free Captain, Fuel Reeve, Slag Sovereign.
4. Remaining checked: Archive Bellkeeper, Shoggoth Heart; Grinwick and Big
   Ripper with their staged engine/ripper semantics; Salvage Claimant; Memory
   Warden and Silence Warden. These complete the remaining 14 checked IDs.
5. Dragon/Pit Tyrant/threshold plus Flesher/Silkmother/Humphrey: measure each
   centre/foot/stage anchor first, then source-aspect 54/96 trials and exceptions
   only with captured evidence. Preserve jump, reveal, stage and turn rules.
6. Register overhead/side foot/root/head/hand sockets and four detached parts
   per camera, beginning with the three pilots at their selected envelopes.
   New Launderer/Wheyward/Goose guardians need their own 72/96/128 review and
   later optional encounter/rules work; do not auto-install proposed weakpoints.

Terminal identity acceptance names the primary coarse props explicitly in
`cell_props`: Tusk's spear/buckler, Cinder's furnace torso/fire grate and
Mother's cap/root arms. Tusks, jaw articulation and gill striations are retained
and fidelity-tested in the source/native sprites; they are not claimed as
resolved details in 96×28 cells. Preserving the exact old cell median requires
a stronger v1 reduction grade than native RGB. That path remains a compromise:
large light material blocks, with the darkest contours protected. Mother's
moss is mixed halfway toward bounded warm parchment during cell grading to
avoid acid-green saturation; native source colour is untouched. Do not use a
passing brightness gate to describe the cell route as matching native fidelity.

A final reduction check found that a decorative material pixel could beat
Tusk's thinner warning ink inside one 4×4 v1 reduction footprint. The existing
warning pigments now receive explicit priority in reduced frames while a
registered warning is active (`cell_warning_priority` in the effect-slot
manifest). Source PNGs, the 36-tick timing, trajectories and native geometry
remain exact. The ink set is cached once; a regression test rejects accidental
matches to any cached library material/pose/flash colour. The glyph-shape gate
keeps its 30% floor. For this registered v1 bank it decodes the exact emitted
warning pigments (still requiring 15 L* above dark paper), including strokes
over the body. The old background-difference mask could count an idle versus
anticipation body change or miss a darker stroke over bright material. Rejected
a larger body-exclusion crop: it also discarded real warning strokes. Known
warning pigments cannot be a body pose; the runtime test proves that separation.
Other banks retain their original difference masks/crops and thresholds.

Name/bar layout follows the projection actually being painted, including a v2
preference that falls back to v1. When a library body's head leaves the top of
the seat's view, the existing label/bar stack stays inside the viewport; its
world-space foot point does not move. Regression coverage checks both fallback
projection and a clipped head with a still-visible health bar.

## ROLL-1 archive and crypt rollout

Index and Tithe Marshal are enabled in normal builds. Waxen Warden and Bone
Choir retain their earlier bodies: their library candidates fail the unchanged
original-median gate. `active:false` in the registry excludes a candidate from
normal body, shadow, bar and name layout and from the normal embedded body bank.
The explicit `boss-library-review` feature includes candidates for study only.
Default v1, all definitions, feet, hitboxes and effect PNGs remain unchanged.

| Boss | v1 envelope | v2 envelope | Decision |
| --- | --- | --- | --- |
| Index | 55×54 | 98×96 | Enabled; open tome and stacked blue books read as coarse cell masses |
| Tithe Marshal | 42×54 | 75×96 | Enabled; steel helm and red tabard read; staff detail is native-only |
| Waxen Warden | 43×54 | 77×96 | Candidate only; four original-median failures |
| Bone Choir | 49×54 | 87×96 | Candidate only; four median failures and v2 native presence failure |

All envelopes fit the joint idle/anticipation source canvas without stretching.
Trials use v1 heights34/42/54/68 and v2 heights72/96/128. Smaller trials lose
faces, book divisions and the Marshal's equipment; the largest add crowding
and upper-body target ambiguity without recovering facial detail in96×28.
The54/96 choices preserve useful room space with four seats and real volleys.
Index's initially stronger352/384 grades were rejected for pale cell materials;
its selected native/cell grades are256/256(v1),288/256(v2). The v2 native256
trial missed normalised presence;288 restores it. Marshal uses320/320(v1) and320/256(v2), keeping more tabard colour than
the rejected384 cell grade. Its ungraded v1 cell trial failed the original
65.64 L* median floor;320 retains that floor.

Waxen's blue mantle and Choir's red robe cannot meet the existing native
brightness floor with the supported hue-preserving grade. Even independently
maximising each visible pixel to the renderer's228-channel ceiling gives median
76.16 L* versus Waxen's required82.74, and65.30 versus Choir's72.16. The actual
candidates retain their materials and fail, rather than changing source hues,
raising the ceiling or weakening the gate. Increasing size cannot fix this
value conflict. Choir's three skulls remain clear natively but merge into a
coarse light cluster in cells;68/128 is retained as a larger native proposal,
not a solution to the value gate or a production size decision. These two need
an explicit art/value-policy decision before another activation attempt.

`scripts/boss-library-retain.py` reproduces all twelve measured crops per boss
from the active refined sheets. Their provenance includes sheet/crop SHA-256,
unchanged definition hashes and state mappings. Both camera banks and all eight
detached parts are retained. Only overhead idle and anticipation are wired:
idle gets the existing one-sample breath; ring/arms warnings select anticipation;
attack retains idle/breath; hurt uses the two existing cached hit grades; rage
retains heat flashes and the real warnings. Side banks, detached sockets and
extra action/recovery/defeat frames remain unwired proposals. No new animation
state or warning is inferred from a drawing. Twelve rule-sized effect requests
for these four bosses are now in `docs/ART-REQUESTS.md`.

Reproduce with the assigned TMPDIR, cartridge and target environments above:

```
uv run --no-project --with Pillow scripts/boss-library-retain.py the-index waxen-warden the-bone-choir crypt-tithe-marshal
uv run --no-project --with Pillow scripts/boss-library-import.py --check
# Normal bodies, authoritative original grids and unchanged effect fixtures:
ANGEL_BOSS_SHOTS=<out>/after/terminal ANGEL_BOSS_OLD=<original-grids> cargo test --manifest-path scripts/boss-display-review/Cargo.toml --no-default-features write_boss_art_terminal_shots -- --ignored --nocapture
# Candidate size study, including the two deliberately inactive bosses:
ANGEL_LIBRARY_STUDY=<out>/study cargo test --manifest-path scripts/boss-display-review/Cargo.toml write_display_study -- --ignored --nocapture
uv run --no-project --with Pillow scripts/check-boss-library.py <out>/after/terminal --before <integ-0>/before/terminal --identity <out>/identity-review.json --metrics <out>/gate-metrics.json
# Expected rejection: nine failures; this does not activate candidates:
uv run --no-project --with Pillow scripts/check-boss-library.py <out>/study --include-inactive --before <integ-0>/before/terminal --identity <out>/identity-review.json --metrics <out>/candidate-gate-metrics.json
uv run --no-project --with Pillow scripts/boss-rollout-review.py <out> the-index waxen-warden the-bone-choir crypt-tithe-marshal
```

The isolated production exporter uses the cockpit's authoritative room/effect
fixtures and actual cell painter; it omits only the separate text HUD. Review
artifacts and twelve bounded sheets live under
`/work/artifacts/boss-art-20261015/roll-1/`. Headless review uses8900/CDP9000,
a browser seat plus three moving/firing bots, both kits, with the review feature
explicitly enabled to inspect the rejected candidates too.

Exact next work: resolve the two inactive candidates' art/value-policy conflict;
then roll out Tax Keeper, Relic Liege, Red Pick, Foreman and Late-Fee Leviathan,
preserving their bespoke Ravage/laser/lane geometry. Continue with groups3–6
above in order (fungal/Hellforge, remaining checked bosses, special-anchor
encounters, then camera/part sockets). Effect deliveries remain separate slots.

## BOSSFX-1 delivered effect integration

Seven independent effect banks are enabled by `boss-effect-slots.json`, including
Waxen Warden and Bone Choir while their library bodies remain inactive. The
importer `scripts/boss-effect-import.py --retain /work/artifacts/art-prod/slots`
retains lossless measured crops, accepted-sheet and original-sheet SHA-256, then
reproduces the checked files from those local crops with `--check`. Waxen's
impact metadata selects its accepted `source-sheet-2.png` retry; the earlier
five-column sheet is recorded but rejected. Marshal's impact delivery has14
measured pieces; only the first five contact-cycle cells are wired.

Frames are mapped by inspection: ordinary flight cells0/1; Tusk's fuller heavy
flight cells5/7; four tell stages0/1/2/3; impact0/1/2/3/4, except Tusk's final
settle uses debris cell11. Indices are zero-based. All unselected source cells
remain retained proposals. There is no new attack state or simulation timing:
flight2×4 ticks and impact2/1/1/1/1 ticks stay exact. Directional tells,
dissipation, alternate impacts and dense-volley switching await actual runtime
states; no appearance-based simulation branching was added.

The source-body/contact socket is registered before BOX downsampling, alpha128,
shared75-ink fitting and existing defleck conversion. A thin translucent red
matte edge is despilled into adjacent solid material. Cinder/Mother use uniform
source exposure0.85, Waxen0.90, others1.00 to fit the dark inks without changing
hue ratios. Each `effect-import.json` records raw-source fidelity separately
from palette-prepared fidelity; the existing0.98/0.90/0.85 import thresholds
apply to the prepared reference. Body imports and their fidelity thresholds are
unchanged. No new ink or dither was introduced.

Tells retain every legacy alpha sample: the art skins the existing marks rather
than turning a source splash into an invented landing circle. Runtime point
clipping, ray angles/reach, stone clipping, mine endpoints, safe gaps and Index
lane half-width remain authoritative. Tusk explicitly identifies only its amber
highlight ink as reduced-cell warning priority. Decorative source
pigments cannot claim body priority; the gate reads the same explicit ink bank.
The15 L* and30% stage-change floors are unchanged. Its regression now recognises
that shared signal-bank highlights already win ordinary reduction, while still
requiring disjoint library material and successful explicit warning reduction.

V1 flight display sizes are14px (Cinder/Mother/Index/Waxen/Choir),16px Marshal,
and32px Tusk's sparse heavy family. V2 has independent native samples26px
(Cinder/Mother/Waxen),24px(Index/Choir),28px Marshal,64px Tusk. These envelopes
include transparent space and are independent of point collision. V2 flight
stamps under the existing warning/HUD overlay; decode and four turns are cached
once. The older effect PNGs remain byte-exact, with new manifest filenames;
old/new active-file hashes are recorded rather than overwriting legacy recipes.

Review builds expose `boss_effect_review(0)` for production and1..3 for manifest
size trials: v1 12/18/24 and v2 20/32/40. Trial resize caches initialise only when
selected; normal frames perform integer stamps. `write_effect_study` in the
isolated cockpit exporter writes both kits, four seats, dense real attack
states, native RGB and actual96×28 half-block/braille buffers. The default kit,
all rule definitions, hitboxes, counts and netcode remain unchanged.

The manifest also owns the normalised per-role source sockets, per-kit display
sizes and fixed flight/impact timing metadata. Changing a display size or socket
then rerunning the importer updates registration without Rust changes. The
import record binds each bank to its unchanged definition SHA-256. Tells retain
the existing80 L* peak ceiling in addition to the unchanged palette/alpha gates.

Cell-stage review rejected the fixed gold point rim: it hid Cinder's four-stage
progression after reduction. `point_rim` now supplies four source-palette value
steps for Cinder and Mother. These cached colours replace only the existing
circular rim samples; radius, centre, clipping, fuse and release stay exact.

## VALUE-1: graded guardian-floor contrast

This supersedes ROLL-1's original-median rejection. Waxen Warden is enabled;
Bone Choir retains its earlier body. All five prior activations remain enabled.
Only policy/provenance and Waxen's reviewed visual hash change: source PNGs,
material grades, sizes, definitions, hitboxes, rules and simulation baselines
are unchanged.

The exporter now uses a generated depth-2 guardian Stairs hall (36×20 tiles,
seed17) of each boss's own pack, with four visible knights. It captures rest
and actual low-health ring-warning anticipation, between existing heat flashes.
A paired floor moves the same boss outside the view: deleting it incorrectly
switches the great hall out of v2 and loses the Cavern chief floor selection.
Each opaque source-alpha position samples its own underlying graded floor;
there is no remote floor swatch or assumed black RGB value. The black floor
has median0 L* in these samples; sampled floor p95 values and all56 path/pose
measurements are retained in `candidate-metrics.json`.

| Boss | Minimum local median ΔL* | Minimum rest presence | Minimum edge coverage | Decision |
| --- | ---: | ---: | ---: | --- |
| iron-tusk-chief | 35.39 | 85.71% | 83.02% | Enabled |
| cinderjaw | 41.42 | 84.63% | 75.85% | Enabled |
| mother-of-spores | 32.88 | 88.21% | 76.62% | Enabled |
| the-index | 24.59 | 82.12% | 72.54% | Enabled |
| waxen-warden | 40.88 | 92.44% | 85.50% | Enabled |
| the-bone-choir | 23.56 | 74.41% | 77.23% | Retain earlier body |
| crypt-tithe-marshal | 32.26 | 81.40% | 79.58% | Enabled |

Minimums above span both kits, native RGB and decoded cells, and both poses;
presence is the existing rest-only gate. Thresholds are15 L*,80%,50% respectively.
Waxen's rest native presence is96.45%/92.44% (v1/v2), and decoded cell presence
is99.70%/99.33%. Its warm crown and blue mantle separate from the room natively;
in cells they become a coarse pale upper mass with a blue lower edge. Individual
candle tips and facial features do not survive cell reduction.

Choir's rest native presence is79.35%/74.41%: deficits0.65/5.59 percentage points,
or9/246 additional visible pixels out of1395/4397 opaque source pixels. Its
rest median separation is27.70 L* in both kits; decoded cells pass at99.50%/95.00%.
This is now a local body-presence failure, not an absolute brightness failure.
Raise interior robe/bone shadow values so at least9 additional v1 samples and246
additional v2 samples reach their local graded floor+15 L*, without changing
alpha, contour, hue families or geometry. Keep the three skulls separated as
native shapes; they currently merge into one pale cell cluster. The precise
replacement request is in `docs/ART-REQUESTS.md`; no art is hand-edited here.

Review evidence: `/work/artifacts/delve-int-20261015/value-1/review/` contains
rest/windup native1×, actual cells and paired-floor thumbnails for all seven,
and actual2× identity crops for Waxen/Choir. Every sheet is at most1570px wide
and eight views. All were inspected. Rejected captures that changed renderer
route, hid two knights behind room-entry blink, or labelled rest as anticipation
were corrected before final measurement. These captures are offline through
the production painters; no play server is started by this task.

Reproduce from the worktree root with the assigned build environment:

```sh
TMPDIR=/work/tmp ANGEL_CARTRIDGES=/work/tmp/empty-cartridges CARGO_TARGET_DIR=/work/build-cache/val-cockpit ANGEL_CONTRAST_SHOTS=<out>/captures systemd-run --user --scope -q -p MemoryMax=40G nice cargo test --manifest-path scripts/boss-display-review/Cargo.toml write_floor_contrast -- --ignored
python3 scripts/boss-floor-review.py <out>/captures <out>/review
python3 scripts/check-boss-library.py <out>/captures --identity <out>/identity-review.json --metrics <out>/gate-metrics.json
# Expected two native presence failures for the retained Choir candidate:
python3 scripts/check-boss-library.py <out>/captures --include-inactive --identity <out>/identity-review.json --metrics <out>/candidate-metrics.json
```

Existing effect-room exports still require `--before` when their
`room-fixtures.json` is present. Supply paired guardian captures separately with
`--contrast`; `boss-art.py check/review` accepts the same bank via
`--floor-contrast`. Other bosses retain their exact original-area/value gates.

## INT-2: contrast against the merged dungeon grade

This recheck supersedes the activation decisions in VALUE-1 above. All seven
library bodies retain their earlier production bodies; their new flight, tell
and impact banks remain active. No art, material grade, hitbox, timing or
contrast threshold changed. The same four-seat, depth-2, seed-17 guardian-room
fixture now samples the DUNGEON-INT-2 floor. Its local floor median is no longer
black (native minima/maxima across fixtures: see the recorded metrics).

| Boss | Minimum median ΔL* | Minimum rest presence | Minimum edge coverage | Decision |
| --- | ---: | ---: | ---: | --- |
| cinderjaw | 14.79 | 47.56% | 61.42% | Retain earlier body |
| crypt-tithe-marshal | 1.30 | 34.09% | 59.89% | Retain earlier body |
| iron-tusk-chief | 10.26 | 46.17% | 62.50% | Retain earlier body |
| mother-of-spores | 10.50 | 52.70% | 45.06% | Retain earlier body |
| the-bone-choir | -1.95 | 26.74% | 46.60% | Retain earlier body |
| the-index | -1.95 | 27.28% | 63.42% | Retain earlier body |
| waxen-warden | 10.37 | 57.95% | 66.57% | Retain earlier body |

The unchanged rule requires median signed local ΔL* ≥15 in both poses,
rest source-alpha presence ≥80%, and absolute outer-two-pixel edge contrast
≥10 L* for ≥50% of samples. All seven fail rest presence; some also fail
median or edge separation. The five previously active bodies and Waxen flip
to rejection. Choir remains rejected, with additional failures on this floor.
The all-candidate gate reports 37 failed requirements (29 among the formerly
active six). An empty production library selection is intentional; the retained
bodies continue through their existing rendering and visual regression gates.

Evidence: `/work/artifacts/delve-int-20261015/int-2/candidate-metrics.json`
contains all 56 path/pose measurements; `contrast-summary.json` aggregates
minimums; `contrast-review/` contains bounded paired native/cell/floor sheets.
Reproduce with the VALUE-1 commands above, using this tree and a fresh output
directory. `--include-inactive` must retain the 37 failures until replacement
art passes the same rule against this floor. The source sheets remain intact.


## BOSS-READ-1: standing-area shade and reflected silhouette light

This supersedes INT-2's production rejection above. All seven bodies are active.
The unchanged rule still requires signed median local ΔL* ≥15 in each pose,
≥80% rest-alpha presence at floor+15 L*, and ≥50% outer-two-logical-pixel
coverage at absolute floor contrast ≥10 L*, in native RGB and actual cells,
both kits. Source PNGs, import sizes, alpha, anchors, material exposures,
effect banks and gameplay definitions remain unchanged.

The presentation uses a cached, soft elliptical shade receiver behind each
body, including a tight contact falloff at its feet. It moves with the boss
without rebaking the room. Dry ground receives it; hazard lips and voids do not.
The cached body bank applies reflected light inside the original silhouette's
outer two logical pixels, strongest on the dark upper/left edge. The gain uses
the source material's RGB ratios, without adding white or extra alpha.
Stairs and Lair halls use a lower ambient band; existing wall light pools stay
at the perimeter. Other room kinds retain their exact previous grade.

Paired floor export now hides only the boss drawing and nameplate, retaining
its position and shade receiver. Moving the boss offscreen would incorrectly
remove the floor shade being measured. Review scope is restored on exit and
never mutates the run. The generated own-pack, depth2, seed17 Stairs fixture,
four seats, both poses/kits and the production cell painter are unchanged.

Minimums across the 56 measurements (presence is rest-only):

| Boss | Median ΔL* | Rest presence | Edge coverage |
| --- | ---: | ---: | ---: |
| iron-tusk-chief | 34.59 | 93.24% | 91.73% |
| cinderjaw | 39.34 | 86.10% | 86.81% |
| mother-of-spores | 32.88 | 90.91% | 88.89% |
| the-index | 23.63 | 86.54% | 82.23% |
| waxen-warden | 39.24 | 96.51% | 89.05% |
| the-bone-choir | 22.52 | 81.46% | 91.84% |
| crypt-tithe-marshal | 30.62 | 88.75% | 93.65% |

Evidence: `/work/artifacts/delve-int-20261015/boss-read-1/after-metrics.json`;
paired before/after native 1× and cell sheets in `after-review/` beside it.
The first, darker arena grade passed numerically but was rejected because too
much v1 floor detail disappeared. The accepted grade retains slab seams,
hazards and four knights; terminal reduction still merges small face/prop
features. No candidate needs replacement value art under this floor treatment.

Reproduce contrast with the existing `write_floor_contrast` exporter and
`check-boss-library.py`, without `library-trials`, so production activation is
also exercised. `write_dungeon_snapshot_bytes` in the same review crate accepts
`ANGEL_DUNGEON_SNAPSHOTS` and `ANGEL_DUNGEON_BYTES` to compare exact native RGB
and cell JSON from preserved four-seat dungeon states across revisions.


## BOSS-ROLL-2 delivery status

Rollouts 2–4 are imported from the measured library crops. Seventeen additional
bodies pass the unchanged graded-floor and source-fidelity gates and use the
54/96 class-height policy. Grinwick retains its original staged body: v1 rest
presence 63.82%, short 254 opaque samples. Slag Sovereign retains its old body:
anticipation colour agreement 0.8921/0.8814 fails 0.90. Exact correction requests
are in `docs/ART-REQUESTS.md`; inactive candidates remain reproducible in review
builds without being embedded in normal builds. Grinwick's inherited
centre+radius feet are shared by native/v2 bodies, floor receivers and evidence.

All nineteen flight/impact banks and seventeen tell banks use their own delivered
sources; the Free Captain and Slag Sovereign tells are rejected opaque
checkerboards and retain byte-exact legacy warning masks. `legacy_mask_boss`
records the inherited mask for a newly registered bank. Source exposure is 1.10
for every flight. Source-derived core/rim inks, bounded saturation, <=12 local
flight inks and the actual-cell core-dot path follow FX-READ-1. The optional
quantizer choice uses the same fixed fidelity thresholds. Shared 75-ink body and
tell/impact gates stay exact. No source design, collision, damage, cadence,
projectile count, beam/lane, Ravage, mine endpoint or staged rule is changed.

`bosses::CHECKED` makes the delivery definitions available to render/review code;
`BUILTIN` and the live population remain unchanged. Fourteen newly registered
appearance IDs add cosmetic shot/impact/Ravage tags on the wire. Parity compares
all physical state, excluding only those appearance tags and cosmetic impacts;
the earlier registered scenes also retain exact full-wire comparisons. No replay
baseline is re-recorded. Smoke's catalogue now covers all 26 checked bosses.
