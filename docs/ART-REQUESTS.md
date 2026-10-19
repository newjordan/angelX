# Art requests for the six mini-games

## Peasant class — PEASANT-1

Identity for every slot: **a bare-chested, barefoot deserter peasant in brown
shorts, Metal Slug-era arcade style, our dark colours**. No helmet, armour,
shirt, footwear or weapon other than loose stones. Art team owns the design;
the integration uses deliberately simple geometric placeholders in
`crates/delve/assets/peasant/slots.json`, cached once. Do not treat those
rectangles as a design reference. Four players and real half-block/braille
cells are the primary readability test; preserve brown shorts for every seat.

| Asset id | Game / role / camera | Requested states, frames and simulation timing (30 Hz) | Native size, anchor and gameplay footprint |
| --- | --- | --- | --- |
| `peasant-body` | Delve player class; overhead front/right/back, mirror right for left; side room uses right/left | idle 1×30 ticks; walk 4 frames per 3.4 world units; throw 3×4 ticks per 12-tick release cycle; sprint 4×3 ticks looping for 60 ticks; hit 2×3 ticks; defeat 4×3 ticks then hold. Current placeholder has one geometric body per facing, shared gait and held/throw stone layers. Deliver distinct poses registered to the feet. | 20×16 canvas, bottom centre (10,16), body in the inner 16 pixels; target native range 18–24×16–20 with unchanged anchor and 0.42 world-unit hero radius. No sprite-derived collision. |
| `peasant-stone` | Delve F projectile; overhead world lob, camera-facing stone, detached impact | flight 2×3 ticks looping during 18-tick flight; impact 3×2 ticks on contact. Logical ground sweep remains at world position, rendered shadow at that point; stone rises to a 1.5-world-unit visual apex at flight midpoint. | 5×5 flight, centre (2,2); delivered range 4–6×4–6. Impact 7–11×7–11, centre. Existing friendly swept collision, 0.16 projectile margin plus enemy radius; maximum flight 10.8 world units + 0.6 muzzle, walls stop it. |
| `peasant-scarecrow` | Delve R fixed decoy in discarded brown clothing; overhead front/right/back; clearly a prop, distinct from the fleeing bare torso | standing 2×15 ticks looping for exactly 90 ticks; collapse 3×3 ticks for 9 ticks then remove. Current placeholder uses a labelled geometric post and clothing bar plus a flat collapsed block. | 20×20, bottom centre (10,20); delivered range 18–24×18–24. Fixed 0.6-world-unit shot interception radius. Standing taunts all enemies in the current room and absorbs repeated hostile shots; collapse has no targeting/collision. |
| `peasant-sprint-dust` | Delve R ground trail, overhead, all movement directions; keep threats visible | 4×3 ticks loop during the 60-tick sprint; placeholder uses one shifting outline. | 9×5, bottom centre (4,4); delivered range 7–11×4–7; cosmetic, no hit box. |

Deliver transparent PNG source sheets plus measured crop JSON and SHA-256,
separate states/facings and material masks, palette/native masters with anchors.
Keep the unchanged slot names so final art can replace the placeholders.
Sound requests: short dry stone release/contact; panicked barefoot sprint and
scarecrow collapse. Temporary sounds reuse `shot_spark` / `boss_rise`.
Tuning: 70 HP (baseline 100), 1.15× movement, zero armour mitigation; stones
27 damage / 12 ticks = **67.5 base DPS**, Bard 67.5 and bow 72. Desertion uses
the shared full-charge/R-press rules: 60 ticks at 1.8× Peasant pace, invulnerable,
untargetable and able to pass enemies; attack verbs are disabled. The sprint
still respects room geometry. The decoy remains for 90 ticks independently.

## Forest Road and Old Oak (FOREST-1, 16 October 2026)

The complete first siege slice uses **temporary block placeholders** behind
[`forest-road/slots.json`](../crates/delve/assets/forest-road/slots.json).
Art team: original, chunky 1990s arcade run-and-gun drawing, thick warm-dark
contours, hue-shifted 5–7-tone materials, our dark colours, no dithering/noise,
no near-white cores. Side view, facing right, mirror left where appropriate.
Do not reproduce any existing game's characters, machinery or frames.

| Asset id | Identity and role | States / frames / ticks at 30 Hz | Native canvas / anchor / authoritative geometry |
|---|---|---|---|
| `forest.ent` | Old Oak, ancient, kind, mossy oak giant. Chained to captors' Feller; regains height, then speaks. Also kingdom forest-edge NPC, screen-facing three-quarter variant requested after side delivery. | chained 4×12 loop; freeing 4×6 (24 total); standing 6×11 (66 total); idle 4×12 loop; talk 4×6 loop during thanks/E dialogue. | 88×108; feet (44,108); no damage collider. Two independent chain targets x=7000, heights floor−48 and floor−24, each 240 HP; keep target sockets visible. |
| `forest.ent-kingdom` | Old Oak as a kingdom NPC at the Dark Forest edge, overhead three-quarter view facing the screen; same character as `forest.ent`, small enough to stand among the buildings. | idle 4×12 loop; talk 4×6 loop while a knight speaks with him (E). | 28×36; feet (14,36), standing on the bottom of kingdom tile (13,8); no collider. Keep it inside its canvas so it never covers place labels or doors. |
| `forest.scout` | Besieging army melee scout, light kit and readable aggressive stance. | idle/walk 4×6; tell 4 frames normalized across 60/53/46; strike 4×3; recover 3×15; hit 2×3; defeat 6×6. | 20×32; feet (10,32); centred 20×32 collider. Chase 2 px/tick; fixed forward strike danger up to 64×26. |
| `forest.archer` | Army bowman. Distinct raised bow and arcing-arrow release. | idle 4×6; tell 4 normalized across 60/53/46; loose 4×3; recover 3×15; hit 2×3; defeat 6×6. | 20×32; (10,32); collider 20×32. One gravity arrow, ~30-tick arc to locked aim marker, at most 60-tick life, 12 damage. |
| `forest.woodcutter` | Heavy army axeman, cutting the forest for siege timber. | idle 4×6; tell 4 normalized across 60/53/46; chop 4×3; recover 3×15; hit 2×3; defeat 6×6. | 20×32; (10,32); collider 20×32. Chop danger 128×44; keep art clipped to authoritative rectangle. |
| `forest.war-wagon` | Mid-boss armoured wagon with mortar, distinct from Feller. | idle 4×6; mortar tell 4 normalized across 60/53/46; fire 4×3; recover 3×15; hit 2×3; defeat 6×6. | 76×52; (38,52); collider 76×52; locked 64×64 ground blast, 12 active ticks. |
| `forest.feller` | Captors' siege sawmill on wheels. Ent is visibly tethered to it. | idle 4×6; saw/mortar/ram tell each 4 normalized across 60/53/46; each attack 4×3; recover 3×15; hit 2×3; defeat 6×6. Initial bank can share tell/attack slots, later keyed variants retain timings. | 104×76; (52,76); collider 104×76. Saw 210×17, mortar 80×72, wheel ram 208×34. All fixed warning/active masks are rules-owned; art must never imply a larger danger. |
| `forest.layer-far` | Distant forest canopy silhouettes, original dark forest landscape. | 1 tile, static; quarter-speed parallax. | 192×80 repeat horizontally; bottom centre (96,80), no collision. |
| `forest.layer-mid` | Middle woodland trunks and undergrowth. | 1 tile, static; half-speed parallax. | 192×72 repeat; (96,72), no collision. |
| `forest.layer-near` | Roadside roots / forest floor strip, dark enough for shots. | 1 tile, static; world-speed parallax. | 192×24 repeat; (96,24), no collision. |
| `forest.hurdle` | Fallen log traversal obstacle. | idle 1 frame. | 36×18; (18,18); solid 36×18 block, jump clears it. |

All actor attack anticipation is at least 46 ticks. Source sheets with measured
crop JSON, transparent native PNGs named `<state>-<index>.png`, manifest,
production plan and SHA-256 provenance; register through the existing slot import
pipeline. Timings and hitboxes remain governed by rules. Arrow/impact/tell art
is requested as a follow-up bank: arrow flight 2×4, axe impact 4×3,
saw/mortar/ram impact 4×3, chain snap 4×6. These effect slots are not wired yet;
current exact danger rectangles and seven-pixel hostile cores remain visible.

## Bard knight and effects (BARD-1, 15 October 2026)

These are **temporary swappable placeholders**, not final designs. Runtime slot
specification: [`bard/slots.json`](../crates/delve/assets/bard/slots.json).
The class is available in the company chooser, at the Scriptorium banner and
through the Sanctuary class cycle, for native and browser seats alike.

| Asset id | Role, camera and identity | States and timing at 30 Hz | Native canvas, anchor and geometry |
|---|---|---|---|
| `bard-knight` | Delve Bard body and held instrument; overhead front, right (mirror left), back. Original knight in the dark 1990s arcade run-and-gun style: enormous helmet, heavy gloves and boots, squat body; the art team designs the instrument and costume. | Idle 1 frame; walk 4 frames driven by the existing 0.85-world-unit stride quarters; attack 1 frame for each 4-tick pull; song 1 frame for 60 ticks (idle fallback until delivery); hit 1 frame using existing struck/ward timers; defeat 1 frame using the existing fallen-knight state. Additional animation frames may subdivide these states, never extend them. | 20×16 placeholder including item margin, feet at (10,16); candidate delivered size 20×20, preserve feet and test real cells. Hero radius **0.42 world units**, unchanged. Also deliver mounted head 7×5 and coat 5×8 (centre bottom anchors), and chariot head 4×5 at (2,5). |
| `bard-note` | Friendly, short-range cone projectile; overhead, readable in every travel direction. Simple note glyph placeholder; artist supplies the dark-contoured musical-note design. | 1 frame, projectile TTL 10 ticks. A pull every 4 ticks emits 3 notes across 30° at 18 units/s; 3 damage each, no base pierce. | 5×7, centre (2,3); candidate 5–7 px square. Existing friendly Arrow collision sweep; art never determines damage geometry. Travels 6 units after its 0.6-unit muzzle offset. |
| `bard-sleep` | Status marker over a sleeping enemy; overhead, screen-facing `Zz` placeholder. Separate from icy stun art. | 1 frame for 60 ticks on normal enemies; 15 ticks on bosses. Damage never wakes early; a song affects an enemy once, without refresh. | 11×7, bottom centre (5,7), placed above enemy feet at y−24; no collision. Delivered size 9–12×7–9; retain the anchor and test tall bosses. |
| `bard-song-ring` | Moving song boundary centred on the Bard; overhead floor layer. Plain circular boundary placeholder; art team supplies any musical ornament. Must show the exact reach. | 1 frame for 60 ticks. Enemy centres within **7 world units** in the current room are reached, through scenery, once per cast; late arrivals receive a full sleep. The Bard may move, attack and guard during the song. Already-fired enemy projectiles and laid hazards continue. | 113×113 at 8 px/world unit; centre (56,56), exact radius 56 px. No collision. Keep any delivered decoration inside this boundary, and preserve readable dangers beneath it. |

Delivery for every slot: transparent RGBA source sheet and crop JSON, original
source SHA-256, frame/state, facing, duration, native canvas and common anchor,
hard-edged native preview, palette/material masks, and detached instrument plus
socket/occlusion metadata. Use the delivery contract below. No source art was
generated or imported for this task.

Sound placeholders reuse `shot_spark` for notes and `boss_rise` for the ultimate.
Needed: a muted plucked-note stream loop with a release tail, and a two-second
lullaby phrase with a soft onset and ending. Normal ultimate charge rules apply:
1 per fight tick plus damage/2, full at 1000, R consumes the charge. The primary's
maximum base DPS is **67.5**, below bow 72 and crossbow ≈71.7; spread and short
reach limit practical single-target damage. Bosses sleep **0.5 seconds once per
cast** so the song interrupts without suppressing their whole attack cycle.

This is the artist handoff for the game shelf at `ef42711a` (15 October 2026).
Design and drawing belong to the art team; importing, registration, animation
wiring, performance and final QA belong to the implementation team. No art or
gameplay changes accompany this specification.

## How to read this

Each named asset is a delivery slot, sometimes containing a family of poses or
props. `Files` is relative to `crates/delve/assets/`; `none` means the renderer
currently draws it. `States` lists existing source poses, with `*` selecting
filenames and `@` counting procedural poses (`@unknown` for inherited dynamic banks). Damage variants are appearances,
not animation frames. `Need` is requested coverage that is **not wired yet**;
its frame count and playback timing are unknown until integration agrees them.
Do not invent new gameplay states or extend a tell to fit an animation.

All times below are engine ticks at **30 Hz**, from
[`rules.rs::HZ`](../crates/delve/src/rules.rs). They describe default tuning;
live F2 knobs can change the named tuned values. Freeze can extend wall time.
Canvas sizes include transparent margins, not just the occupied silhouette.
Pixel coordinates start at the upper left; anchors may lie on the bottom edge.
Larger safe canvases are **unknown, test** for every slot: image stamping can
clip at the viewport, UI or neighbouring body, even when collision is unchanged.
There is no blanket permission to scale up or move an anchor.

Run `python3 scripts/art-requests.py` after source changes; `--check` checks for
stale JSON without writing. The standard-library-only generator measures PNG
headers, reads native recipe states/variants, extracts scalar constants, enums
and timer expressions, and attaches source locations and SHA-256 hashes. Request
blocks below supply editorial identity, anchors and geometry references. These
remain human-audited: the script does not infer hitboxes from artwork or prove
that every loaded pose is actually selected. JSON is a specification, not a new
runtime asset loader. Retained unused files are called out explicitly.

## Delivery contract

- Deliver original source sheets plus a neighbouring JSON for **each sheet**,
  and a top-level `manifest.json` and `production-plan.json`. Follow the measured
  irregular-crop convention in the read-only
  [boss library handoff](./cockpit/assets/boss-library/HANDOFF.md).
  Do not assume a regular grid. The library's roster production plan is under
  `boss-library/existing/production-plan.json`; there is no root plan in this checkout.
- Each frame record needs `asset_id`, `state`, `frame_index`, `camera`, `facing`,
  `mirror_allowed`, `source_rect_xywh`, `native_canvas_wh`, `anchor_xy`,
  `duration_ticks` (or `simulation_state`), `source_sha256`, palette/material
  masks, and layer order. Record the source pivot and its native mapping; never
  trim each pose independently and lose the common baseline.
- Keep transparent RGBA around isolated subjects. Include measured alpha-core
  bounds and alpha threshold; the boss source convention measures at 128, while
  these mini-game PNG decoders accept any nonzero alpha. Supply hard-edged native
  previews as well as high-resolution sources; integration owns final palette
  reduction and cleanup. No resampling or floats in the per-frame draw path.
- Every actor needs idle, anticipation, action and recovery coverage, plus hit,
  damage and comic defeat where its rules support them. Preserve existing frame
  counts first. Extra breathing, settling and follow-through frames go in the
  production plan as unwired proposals, with unknown timing clearly marked.
- Detach the listed weapons, lids, banners, limbs, pickups and VFX; include
  socket coordinates, pivot, local layer/occlusion order and registration pose.
  Supply complete registered pose composites too: current renderers mostly
  consume whole bodies. Detached parts are future integration work, not present
  skeletal support. Left/right banks must identify which asymmetric details
  can mirror. No invented overhead bank for a side-only game.
- Deliver seat-cloth/plume and metal/material masks separately. Latch/Brasswake
  use the shared knight ink recolour mapping; Rafterwake/Pennon also expand a
  four-step cloth ramp. All moving side knights have cached left/right copies;
  Latch holds one facing. Mossbell currently selects four seat
  colours directly; Cinder uses the hero's normal figure. Keep eyes, brass,
  skin, leather and warning colours out of the seat mask.
- Hit boxes, feet, support lips, paddle contacts and danger areas below are
  **read-only simulation geometry**. Author decorative VFX separately from exact
  marks. Integration draws reliable telegraphs and safe gaps from the rules;
  never paint a larger implied threat, conceal the boundary or fill a safe gap.
- Review at native size, enlarged nearest-neighbour, and the **actual terminal
  half-block/braille writer**. The reference canvas is 384×224
  (`arena.rs::NATIVE_W/NATIVE_H`, 24×14 tiles at 16 px). The 96-column review
  path averages material into 4×4 half-cells; inspect at other supported sizes
  too. Preserve readable faces, paired lights, feet and danger boundaries.
  Solid bodies/contact cores use half blocks; atmosphere uses braille. Tagged
  narrow details in Rafterwake/Pennon have special cached cell paths.
- Style: original chunky 1990s arcade cartoon sprite principles, thick warm
  brown/indigo contours, heroic squat proportions, expressive comic acting,
  rich hue-shifted material ramps, tactile wear expressed by shapes. Keep dark
  rooms and black terminal space. No dithering, noisy flecks, tiny mottled
  patches, dense stripes, near-white cores or full-frame glow. Never copy SNK,
  Mario, Sonic, Mega Man, Kirby, Castlevania or other commercial characters,
  signature costumes, vehicles, bosses or frames. This restriction applies to
  **every** slot, including props, VFX and UI.
- The numeric ink mapping is
  [`ink.rs::ink/PALETTE`](../crates/delve/src/render/overworld/ink.rs), exported in
  the JSON; dark-room/material guidance is in [ART-V2.md](ART-V2.md). The current
  operator brief overrides that document's older contour/style language.
  Existing recipes can contain deliberately tested local colours: Rafterwake
  has local keeper inks, and Pennon has its own recipe bank. These are not
  permission to expand the shared palette. Richer ramps need measured native
  and cell trials and an explicit integration proposal; never loosen tests.
  UI keeps black-surrounded navy/brass plates, tonal text and reserved seat and
  danger colours. Do not redraw labels into source sheets or change wording.
- Knight anatomy must follow the art team's existing knight reference:
  enormous helmet, nearly helmet-sized gloves, heavy boots, short limbs, round
  belly, springy stance and repaired asymmetric kit. Read-only source assets
  and onboarding stay untouched. If integration later copies source crops,
  record their SHA-256; this documentation task copies no art.

## cinder — Cinder Stack

Alchemy Lab: drop matching pots to crack a mischievous kiln spirit. Each seat
owns its board. Sources: [rules](../crates/delve/src/rules/cinder.rs),
[renderer](../crates/delve/src/render/overworld/arena/cinder.rs),
[current art notes](../crates/delve/assets/cinder/ART.md),
[GAME-1C report](/work/artifacts/boss-art-20261015/game-1c/REPORT.md).
The lore's guardians/helping tone applies; Crock is not Cinderjaw, the separate
Mines guardian in [DELVE-LORE.md](DELVE-LORE.md).

### cinder.crock — Crock

- Role: boss
- View: side/front three-quarter; fixed front, no runtime mirroring.
- Files: cinder/crock.png, cinder/crock-crack-*.png, cinder/flinch-*.png, cinder/windup-*.png, cinder/seam-cells-*.png
- States: idle=crock; damage-variants=crock-crack-*; flinch=flinch-*; anticipation=windup-*; terminal-variants=seam-cells-*
- Timing: Idle bobs 1 px each half of a 36-tick period. Damage at 12/24/36 cracks; two flinches divide the 8-tick clear at flash>4 / <=4. Two wind-ups divide tuned TELL=27 at windup/2; attacks rest 165/135/105 ticks by cracks phase. Cascades pause tell progression. No separate recovery timer (Board::step, crack_stage; renderer::draw).
- Fit: 80×80; stamp top-left (270,78+bob), pivot (40,80) by canvas convention. Damage overlays are registered in x=4..76, y=24..76 in art(); retain that registration.
- Geometry: Board cells and cracks own hits; no spatial boss hurtbox. Renderer::landing and draw own the exact slag destination; body art must not suggest a second attack area.
- Brief: Overcommitted ceramic kiln spirit; teal chimney/lid, mitten hands, amber furnace mouth and eyebrows. Puff up before coughing, then visibly wince; terracotta body, cool glaze, restrained warm core. Detach lid, hands and smoke.
- Need: Breathing idle, actual cough/action and settling recovery, comic exhausted finish; counts/timing unknown. Preserve all damage levels across acting poses and terminal seam masks.
- Slot: png; embedded PNG pose arrays, but damage composition, terminal face, seams, bob and selection are hard-coded in art()/draw().

### cinder.crock-lab — Lab miniature

- Role: prop
- View: side/front three-quarter; fixed front.
- Files: cinder/crock-small.png
- States: idle=crock-small
- Timing: Static room sign, no independent timer (renderer::sign).
- Fit: 28×28; sign stamps at (325,94), canvas top-left; pivot convention (14,28).
- Geometry: cinder::KILN=(40,20) is the room interaction point; sign is decorative, not a collision proxy.
- Brief: Same Crock, readable furnace cavity and brows at small size; teal/terracotta/amber. Lid remains an identifiable silhouette; no separate miniature redesign.
- Need: Optional idle acting, unknown frame count; detachable lid as for the boss.
- Slot: png; fixed miniature in art()/sign(), no general manifest.

### cinder.knight — Board-side knight

- Role: player knight
- View: overhead room figure through knights::figure; facing follows hero data, not a side-fighter bank.
- Files: none
- States: existing-hero-figure=@unknown
- Timing: Reuses hero pose/gear rendering; no puzzle-specific animation clock (renderer::draw, knights::figure_with).
- Fit: Dynamic figure dimensions depend on class, gear, helmet and avatar; unknown, test. Canvas bottom-centre anchored at (45,40).
- Geometry: Board::pair_at/fits define puzzle contacts; the figure is a seat portrait with no puzzle hurtbox.
- Brief: Recognisable seated knight watching the lab, with shared helmet/glove/boot proportions; preserve own cloth/plume colours. Detach head, gloves and plume in the source bank.
- Need: Watch/turn/drop reaction, clear cheer, cough flinch, victory; unknown counts/timing, not currently selected by puzzle state.
- Slot: hard-coded; shared knights::figure call, no Cinder-specific pose slot. Do not edit onboarding.

### cinder.pots — Match pots and ghost

- Role: prop
- View: side/front icons; no mirroring.
- Files: none
- States: colour-shapes=@3; slag-cell=@1; ghost-outline=@1
- Timing: FALL=20 ticks/cell, SLIDE=5 repeat, LOCK=10 grounded; hard drop immediate. Clear persists 8 ticks, with 8 integer flight positions toward Crock; retry=15 (Board::step, renderer::pot/draw).
- Fit: Cell canvas 24×20; top-left (BX+column×24,BY+row×20), BX=96, BY=9, plus shake. Next pair uses the same grain; ghost uses inset 2 px.
- Geometry: COLS=6, ROWS=10; Board::pair_at/fits/ghost/matches. Three colour identities must retain distinct shapes; slag does not match and clears beside matching pots.
- Brief: Round jar, low square crock, tall flask; blue, terracotta, green roles respectively. Slag is a broad ceramic shard. Keep clay rims and simple shape symbols; no noisy texture. Detach flying pots from flashes.
- Need: Land/compress and clear break-up art, unknown frame count; gameplay cell occupancy remains exact.
- Slot: hard-coded; pot() rect/line recipes and ghost outline, no swappable data recipe.

### cinder.slag — Falling slag

- Role: projectile
- View: side/front; downward travel, no mirror.
- Files: cinder/slag.png
- States: travel=slag
- Timing: Position interpolates through tuned 27-tick tell; landing inserts a slag cell immediately, not a ballistic projectile (Board::step; renderer::draw).
- Fit: 18×24; top-left (marked cell x+3,fall_y). Keep centre over the 24 px column; bottom may cross neighbouring art, not gameplay cells.
- Geometry: Renderer::landing finds the actual empty cell. Full column means cough/reset; no art-derived radius.
- Brief: Dark terracotta lump, one broad amber seam and short copper tail; same ceramic as Crock. Tail detached; never a bright fireball hiding its landing mark.
- Need: Travel loop/settle, unknown count; terminal currently replaces this PNG with procedural slag-cell art.
- Slot: mixed; PNG in RGB, hard-coded pot() replacement in terminal.

### cinder.tell — Kiln breath

- Role: effect
- View: side/front; fixed behind Crock.
- Files: cinder/tell.png, cinder/tell-cells.png
- States: smoke=tell; terminal-variants=tell-cells
- Timing: Visible throughout Board::tell>0 (default 27 ticks); no source frame cycling.
- Fit: 56×36; top-left (282,140), behind the 80×80 boss.
- Geometry: Decorative breath only. Exact column lines and destination frame are simulation-driven in renderer::draw, not in this image.
- Brief: Open cool teal curls supporting the kiln cough; dark falloff, broad negative space. Separate from lid/body and slag.
- Need: Anticipation inhale/exhale and dissipate, unknown counts/timing; keep cell-friendly open curls.
- Slot: png; two embedded variants; danger-column artwork remains hard-coded.

### cinder.impact — Slag landing splash

- Role: effect
- View: side/front at board cell; no mirror.
- Files: cinder/impact-*.png, cinder/impact.png
- States: impact=impact-*; retained-unused=impact
- Timing: Four active frames, 3 ticks each, Board::impact=12. Unnumbered impact.png is retained but not loaded (art()/draw()).
- Fit: Active 40×24 at (cell x-8,cell y-2); legacy 32×18 is not an active target.
- Geometry: Board::column/impact_row select the cell; splash is not additional danger.
- Brief: Low molten ceramic splash with broad attached shards, cool-off at the end; terracotta/ochre, no white centre. Detach shards and smoke from the core.
- Need: Keep current contact-to-cooled sequence; optional follow-through unknown.
- Slot: png; fixed array and frame indexing in renderer.

### cinder.clear — Pot clear flash

- Role: effect
- View: front icon, cell aligned; no mirror.
- Files: cinder/clear-*.png
- States: clear=clear-*
- Timing: Three frames selected by floor((8-flash)/3): nominal 3/3/2 ticks. Flying pots remain independent (renderer::draw).
- Fit: 24×24; top-left (cell x,cell y-2).
- Geometry: Only Board::pop cells light up; do not add false cleared cells or cover the next pair/landing warning.
- Brief: Filled radial pop in the pot's warm/cool room language; solid core and broad rays. Detach from travelling pots and CHAIN plate.
- Need: Preserve three-step readability in cells; extra explosion/smoke coverage unknown and unwired.
- Slot: png; embedded array; flight arcs and chain glyphs are hard-coded.

### cinder.room — Alchemy Lab interior

- Role: backdrop
- View: side presentation plate behind the puzzle; fixed camera.
- Files: none
- States: room-shake-variants=@3
- Timing: Cached static room at offsets 0/-2/+2; shake chosen from Board::impact (renderer::room/draw).
- Fit: 384×224; top-left (0,0). Board 144×200 starts (96,9); preserve its clearance and sidebar areas.
- Geometry: room() panel is (90+offset,3,156,210); collision is Board grid, not shelves. Keep pots, marks and UI unobscured.
- Brief: Existing rim masonry, posts, framed board and kiln footing. Request separate far masonry/recess, middle shelves and equipment, near trim/lights; keep black between lit objects. Ceramic racks, jars and kiln tools support the established lab identity.
- Need: Layered room kit instead of a flat empty void; dimensions of individual new props unknown, test. Any ambient loop count unknown.
- Slot: hard-coded; room() primitives cached in OnceLock; no backdrop manifest.

### cinder.chrome — Puzzle plates and podium

- Role: UI plate
- View: screen aligned; no mirror.
- Files: none
- States: title-and-hud=@1; chain-values=@20; cough-panel=@1; podium=@1
- Timing: CHAIN while flash>0 and chain>1; cough RETRY=15; podium on round end, rematch next fresh E (renderer::draw; cinder::Session).
- Fit: 384×224 layout; cached chain plates height 20, width from text_width×2+8. Panel geometry and exact labels remain renderer-owned.
- Geometry: draw()/room() and chrome::panel; no collision. Rival stacks reflect actual six-column heights; do not bake scores/numbers into plates.
- Brief: Dark navy/brass wells and muted lab trim, strong seat swatches; detachable empty plate/trim only. Keep typography and clear gutters.
- Need: Reusable empty title, panel and podium skins; unknown slice margins, test with longest live labels.
- Slot: hard-coded; chrome panels, numbers, glyphs, rival plots and layout.

## latch — Latch Duel

The Dented Anvil practice bay: catch the closing beat and buckle Clasp's bellows.
Sources: [rules](../crates/delve/src/rules/latch.rs),
[renderer](../crates/delve/src/render/overworld/arena/latch.rs),
[brief](../crates/delve/assets/latch/BRIEF.md),
[GAME-2C report](/work/artifacts/boss-art-20261015/game-2c/REPORT.md).
Defaults: TELL=36, WINDOW=7, REST=24, RECOVER=18, STOP=4; clean freeze pauses
the beat timer. Rest after a clean beat is 24−3×counters; miss returns directly
to tell. This is a stationary timing duel, **no spatial fighter hitboxes**.

### latch.clasp — Clasp

- Role: rival
- View: side/front three-quarter facing left; fixed, no runtime mirror.
- Files: latch/idle.png, latch/high-*.png, latch/low-*.png, latch/flinch.png, latch/damage-*.png, latch/defeat.png
- States: idle=idle; high-anticipation=high-tell; high-action=high-attack; low-anticipation=low-tell; low-action=low-attack; flinch=flinch; damage-variants=damage-*; defeat=defeat
- Timing: Nine source poses; tell 36, missed attack/recovery 18, clean flinch 18 plus STOP=4 freeze. Rest 24 initially, then 21/18 after counters. Damage poses at counters 1/2; defeat at 3 after final clean (Duel::step_at; renderer::pose).
- Fit: 80×80 sources enlarged once to 100×100; top-left (217+recoil+shake,56), base canvas foot (50,100). Face positions differ per pose in art() eyes[]; terminal visor snaps to cells.
- Geometry: Duel::height=-1/+1 selects contact y=120/144; draw() lane starts x=192, width=25, height=2. Counter legality is Beat/ticks/guard, never the image outline.
- Brief: Heavy copper practice automaton, leather bellows, iron feet, high hook/low rake and twin amber rivet eyes. Commit weight, buckle and slump like an embarrassed sporting rival. Detach head, hook/rake arm and bellows; keep both eyes readable.
- Need: Breathing idle and distinct settling recovery; counts/timing unknown. Maintain damage over action poses and the twin-eye terminal signature.
- Slot: png; embedded poses, cached damage/visor/resize and terminal exposure are hard-coded.

### latch.knight — Guard and counter knight

- Role: player knight
- View: side facing right; fixed stance, no runtime mirror.
- Files: latch/knight-*.png
- States: high-guard=knight-guard-high; low-guard=knight-guard-low; counter-brace=knight-thrust-brace; counter-contact=knight-thrust-contact; counter-recovery=knight-thrust-recover
- Timing: Five bodies. Guards held by input; Clean selects brace at freeze==STOP, contact while freeze>0 or recovery age<RECOVER/2, then recover. Defaults 4 freeze, 18 recovery; Miss drops stamp by 3 px (renderer::knight).
- Fit: **128×80 canvases**, stamped 1:1 at (126-shake,76+miss offset). They contain a much narrower body and long weapon clearance; do not trim to the body or mistake report body size for file size. Canvas foot convention (64,80), exact foot sockets unknown, test.
- Geometry: Same timing-only Duel as Clasp; high shield must remain helmet-high, low at shins, counter reaches contact near (232+recoil+shake,130).
- Brief: Shared chunky knight with seat tabard/plume and warm shield emblem. Whole-body brace, lean and thrust; broad shield moves with arm. Detach shield, thrust weapon, gloves, head and plume; preserve steel outside seat masks.
- Need: Idle breath, miss collapse/recover, victory; unknown counts/timing. Existing counter is a three-pose performance, not a new attack rule.
- Slot: png; embedded five-pose recolour cache; anchors and selectors hard-coded.

### latch.parry — Catch crescent

- Role: effect
- View: side, fixed contact orientation.
- Files: latch/parry-*.png
- States: parry=parry-*
- Timing: Four frames across RECOVER=18 via floor(age×4/18), clamped; STOP=4 holds age zero first (draw()).
- Fit: 32×32; top-left (contact x-16,130-16), contact x=232+recoil+shake.
- Geometry: Clean only, no danger radius. Must meet both bodies in the published contact gap.
- Brief: Cool blue/teal catching crescent, broad connected arc; separate from blade and clash. No near-white spark dust.
- Need: Keep catch-to-release sequence; additional frames unknown.
- Slot: png; fixed four-frame array, overlaid with strike and clash.

### latch.clash — Copper contact burst

- Role: effect
- View: side, centred contact.
- Files: latch/clash-*.png
- States: clash=clash-*
- Timing: Four frames over 18 recovery ticks plus freeze as above. Miss uses only clash-4 at the guard contact height for the miss beat (draw()).
- Fit: 32×32; clean contact−(16,16); miss top-left (147,contact_y-16).
- Geometry: No damage beyond timing judgement; do not obscure the high/low lane.
- Brief: Copper/amber blunt petals, forceful and compact; core and petals separate, no commercial impact motif.
- Need: Preserve clean versus miss placement; expanded/debris coverage unknown.
- Slot: png; fixed array and miss fallback.

### latch.strike — Counter slash

- Role: effect
- View: side, diagonal riposte direction fixed.
- Files: latch/strike-*.png
- States: strike=strike-*
- Timing: Four frames over 18 recovery ticks plus local freeze, same index as parry/clash (draw()).
- Fit: 32×32; top-left (contact x-20,130-16), intentionally offset from the other bursts.
- Geometry: Counter timing remains Duel-owned; blade arc is decorative.
- Brief: Muted teal curved wedge, widening then releasing; detachable from the knight weapon and copper contact core.
- Need: Retain common contact registration through the whole sequence; extra frames unknown.
- Slot: png; fixed array.

### latch.room — Smithy practice alcove

- Role: backdrop
- View: side room plate, fixed.
- Files: none
- States: room=@1
- Timing: Static OnceLock room (room()).
- Fit: 384×224 at (0,0); fighter baselines end y=156; title and score strips stay clear.
- Geometry: room()/draw(); no movement collision. Keep exact high/low contact lanes and narrow gap visible.
- Brief: Existing timber alcove, brick rim, forge recess, tool rack and quench tub. Deliver far recess, middle beams/tools and near floor/trim separately, dark navy recess and warm forge metal. No empty black stage detached from the smithy.
- Need: Layered enclosure and modest forge atmosphere; per-prop dimensions and loop count unknown, test.
- Slot: hard-coded; room() geometry, no room data slot.

### latch.chrome — Duel plates and podium

- Role: UI plate
- View: screen aligned.
- Files: none
- States: title-hud=@1; clean-chain-values=@3; podium=@1
- Timing: PERFECT/CHAIN during Clean; panel replaces bout on round end; E rematch (draw(), Duel::step_at).
- Fit: 384×224 layout; clean plate height 20, width derived from text. Contact label near y=76/88 must remain separate from bodies.
- Geometry: renderer::draw and chrome::panel; counter pips, timing, ranks and purse are live data, not source art.
- Brief: Quiet smithy navy/brass empty plates; steel/seat accents, dark text surrounds. Separate trim from fill, preserve all wording.
- Need: Empty title, result, panel and podium skins with tested longest-label clearance; slice sizes unknown.
- Slot: hard-coded; chrome, glyphs and all layout.

## mossbell — Mossbell

Bathhouse conservatory: a heavy brass marble, two paddles, three shaped targets
and a stubborn seed-pod Keeper. Sources:
[rules](../crates/delve/src/rules/mossbell.rs),
[renderer](../crates/delve/src/render/overworld/arena/mossbell.rs),
[native metadata](../crates/delve/assets/mossbell/native.json),
[brief](../crates/delve/assets/mossbell/BRIEF.md),
[GAME-3D report](/work/artifacts/boss-art-20261015/game-3d/REPORT.md).
Table physics is 160×140, Q=256 fixed units/px; display x maps through
floor(x×5/4), origin (114,46), plus shake. **Do not widen physics with art.**

### mossbell.keeper — Seed-pod Keeper

- Role: boss
- View: overhead table with front-facing three-quarter character; fixed camera, no mirror.
- Files: mossbell/idle.png, mossbell/root-tell.png, mossbell/root-attack.png, mossbell/crown-tell.png, mossbell/crown-attack.png, mossbell/flinch.png, mossbell/damage-*.png, mossbell/defeat.png
- States: idle=idle; root-anticipation=root-tell; root-action=root-attack; crown-anticipation=crown-tell; crown-action=crown-attack; flinch=flinch; damage-variants=damage-*; defeat=defeat
- Timing: Nine poses; starts Idle=120. Tell 36 ticks at hp>2, 30 at hp<=2; action 12; subsequent Idle=90+10×hp. Flinch=12, hit cooldown=10; damage stages hp 6–5/4–3/2–1. Defeat at hp=0 (Ball::step; draw()).
- Fit: 48×48 at (OX+field_x(80)-24+shake,OY-3)=(190+shake,43); source pivot convention (24,48). Signature sockets in native.json states[].
- Geometry: Ball::circle(80×Q,27×Q,19×Q) is Keeper contact radius including ball; do not derive body collision from shell. Root and crown geometry belong to separate slots.
- Brief: Rounded bronze seed shell, teal leaf antlers, root arms, split acorn mouth, curved amber chest/visor. Stubborn gardening guardian: spread, clamp, recoil and fold leaves comically. Detach antlers/crown, arms and shell panels; preserve warm signature against cool hood.
- Need: Breathing and a settling pose between commitments; counts unknown. Preserve protected signature lights and damage stages over all active poses.
- Slot: mixed; PNG composites plus native.json signature metadata; damage, terminal exposure and selection hard-coded.

### mossbell.knight — Paddle operator

- Role: player knight
- View: overhead/rear view at lower table; no mirror, left/right arms independent.
- Files: none
- States: neutral=@1; left-active=@1; right-active=@1; both-active=@1
- Timing: Four combinations, active while left/right>0; each stroke sets a 5-tick lever hold (Ball::step; knight()).
- Fit: 20×24 at (204,195), bottom-centre convention (10,24). Source must read at this small grain.
- Geometry: Decorative seat body; actual lever contacts are Ball::rail/swept paddle wedge. Do not move lever pivots with larger hands.
- Brief: Shared huge-helmet knight seen from behind, planted boots, articulated mittens working the two levers; seat cloth and steel. Detach forearms, hands, head/plume.
- Need: Compress/swing/release acting, launch/nudge, drain slump and jackpot cheer; unknown extra frame counts/timing.
- Slot: hard-coded; four procedural bodies per seat in knight() cache.

### mossbell.targets — Three shaped bumpers

- Role: prop
- View: overhead; circle, diamond and petal bank, no mirror.
- Files: none
- States: shapes=@3; flash-variants=@12
- Timing: Three shapes, four contour-flash frames each, 3 ticks/frame over flash=12; lit indicator persists until drain (art()/draw(), Ball::step).
- Fit: 28×28 at each display bumper centre−(14,14). BUMPERS=(44,57,10),(116,57,10),(80,83,11) in physics pixels/radii.
- Geometry: Ball::circle uses (bumper radius+3)×Q. Keep three silhouette identities and the centre target above root art; lit indicator below each target is code-owned.
- Brief: Copper conservatory fittings: ring, diamond and seed petals; broad gaps, amber hit rim and cool support shadow. Separate static body, flash rim and status light.
- Need: Compression/rebound within existing flash sequence, unknown extra frames; no altered rebound radius.
- Slot: hard-coded; art() builds shapes and recoloured contour flashes. Stored bumper PNGs below are not these props.

### mossbell.flippers — Wooden paddle pair

- Role: prop
- View: overhead, paired left/right; pivots independent.
- Files: none
- States: resting-pair=@1; left-raised=@1; right-raised=@1; both-raised=@1
- Timing: Two endpoints per paddle; held active up to 5 ticks per stroke; swept contacts also handled in Ball::step. Draw has no in-between source frames.
- Fit: Rule pivots (37,119)/(123,119); tips (70,130)/(90,130) resting, y=111 raised; renderer maps x×5/4 and draws 5-px thickness. Pivot local coordinates depend on future crop, unknown.
- Geometry: Ball::rail and swept wedge own contact; preserve the 3 px collision rail radius and the exact pivot/tip coordinates.
- Brief: Warm wooden paddles with dark support and restrained ochre upper lip; compress then spring from their real pivots. Detach lever from table and operator hands.
- Need: Registered paddle cutouts and optional arc poses, count unknown; poses must follow simulation angles, not invent reach.
- Slot: hard-coded; draw() lines, no image slot.

### mossbell.ball — Brass marble

- Role: projectile
- View: overhead round highlight, no mirror.
- Files: none
- States: marble=@1
- Timing: One cached image; launch speed moss_launch=1450, gravity=28, bounce=85%, nudge=180, flipper=1250 fixed units; local freeze=2 on paddle contact (Ball::step).
- Fit: 9×9 at display ball centre−(4,4); pivot (4,4). Visible when save=0.
- Geometry: Ball::rail uses 3×Q ball radius; wall/substep contacts and Ball::circle remain authoritative. Visible radius is not a new collider.
- Brief: Heavy brass with a single broad warm glint and dark underside; no white core. Separate contact flash and trail.
- Need: Optional roll/glint loop, unknown count; preserve tiny but solid cell core.
- Slot: hard-coded; art() ellipse and highlights.

### mossbell.trail — Marble afterimage

- Role: effect
- View: overhead, position history; no directional sprite rotation.
- Files: none
- States: trail=@1
- Timing: Same 5×5 image at four Ball::trail samples while save=0 (draw()).
- Fit: 5×5, pivot (2,2), sampled display centre−(2,2).
- Geometry: Historical positions only, not a wider ball or danger strip.
- Brief: Dim brass/brown echo, always quieter than the marble and warning bands. Detached from ball and impact core.
- Need: Taper/fade variants if integration adds them; count/timing unknown.
- Slot: hard-coded; art()/draw().

### mossbell.root — Root sweep tell

- Role: effect
- View: overhead band; source mirrored horizontally inside its band for both directions.
- Files: mossbell/root-vines-*.png
- States: root-progression=root-vines-*
- Timing: Four stages: floor(elapsed×4/tell_length), clamped to 3. Tell=36/30; action=12 holds stage 3 (Ball::step, draw()).
- Fit: Sources 184×11; runtime clips to 179×11 (field_x(150)-field_x(7)). Top-left (OX+field_x(7)+shake,OY+72)=(122+shake,118). Keep the authored plunger-wall margin.
- Geometry: Actual attack affects any ball y in inclusive [72,82] while Beat::Root, with direction from attack parity. Decorative crop is bounded between physics x=7 and 150; ball walls, not vines, bound traversal.
- Brief: Teal root tendrils with amber leading bud, four growing silhouettes; airy branches, no loose tip flecks. Detach from body and keep the centre bumper on top.
- Need: Preserve four-step cell-visible advance in both directions; settle/retract art unknown and unwired.
- Slot: png; native.json also records recipes, but runtime reads PNGs then clips/mirrors. Stage/geometry mapping hard-coded.

### mossbell.crown — Crown clamp tell

- Role: effect
- View: overhead, corner-anchored hooked jaws; fixed orientation.
- Files: mossbell/crown-vines-*.png
- States: crown-progression=crown-vines-*
- Timing: Four stages through 36/30 tell ticks, then stage 3 through 12 action ticks (Ball::step, draw()).
- Fit: 51×24 at (OX+field_x(60)+shake,OY+20)=(189+shake,66). Floor tint extracted once from RGB (24,43,43), drawn behind Keeper; jaws above it.
- Geometry: Exact inclusive gate physics x=[60,100],y=[20,43]. Do not extend jaws, tint or implied damage beyond it; retain visible target and Keeper silhouette.
- Brief: Cool jade hooked leaf jaws closing inward/downward around original gate; quiet floor tint, amber tips. Floor, jaws and Keeper must be separate layers.
- Need: Preserve four progressive silhouettes; release/recovery unknown and unwired.
- Slot: png; native recipe retained; floor separation and stage timing hard-coded.

### mossbell.bumper-burst — Retained bumper effect strip

- Role: effect
- View: overhead burst, no mirror.
- Files: mossbell/bumper-*.png
- States: retained-unused=bumper-*
- Timing: Four loaded frames in effects[0], **not selected by draw()**. Real bumpers use procedural contour flashes; no current playback duration.
- Fit: 32×32 source canvases; no active stamp/anchor. Proposed centre anchor (16,16) is an integration convention, not current placement.
- Geometry: No gameplay effect; must not be confused with BUMPERS collision bodies.
- Brief: Rounded copper bumper ring opening/receding; broad clean arc, no white or isolated sparks. Detach from static targets.
- Need: Art team can defer; decide whether to use or retire before drawing more frames.
- Slot: png; loaded but unwired effect bank.

### mossbell.impact — Marble contact burst

- Role: effect
- View: overhead contact, no mirror.
- Files: mossbell/impact-*.png
- States: impact=impact-*
- Timing: Four frames indexed floor((16-burst)/4); ordinary contact sets burst=12, so playback begins at frame 1 and uses three frames for 4 ticks each (Ball::contact, draw()).
- Fit: 32×32; display impact centre−(16,16).
- Geometry: Ball::impact reports the contact; burst adds no collision. Drain contact is (80,134).
- Brief: Connected blunt copper/teal contact star, grounded force instead of glitter. Core and lobes detachable; keep solid in half blocks.
- Need: Resolve whether frame 0 should remain reserved; no timing changes authorised here. Extra smoke unknown.
- Slot: png; four-frame bank with offset start hard-coded.

### mossbell.jackpot — Seed jackpot bloom

- Role: effect
- View: overhead, fixed centre.
- Files: mossbell/jackpot-*.png
- States: jackpot=jackpot-*
- Timing: Four frames, 4 ticks each when jackpot sets burst=16 at (80,63); normal contact formula is shared (Ball::step, draw()).
- Fit: 32×32 at mapped (80,63)−(16,16); no enlarged particle cloud.
- Geometry: Jackpot requires Keeper hp=0 and all three lit bits; art is celebratory, no damage.
- Brief: Broad seed petals and warm bronze bloom, distinct from small contact and root/crown warnings. Detach petals from flash core.
- Need: Comic payoff/cheer association, unknown extra frames; maintain target visibility.
- Slot: png; fixed strip and special impact-coordinate selection.

### mossbell.steam — Conservatory steam

- Role: effect
- View: overhead room atmosphere; fixed side positions.
- Files: none
- States: rising-offsets=@9
- Timing: Nine rise offsets, each 3 ticks, from run.tick/3%9; two line curls (draw()).
- Fit: Procedural near x=32/351, y=30..43 minus rise; no sprite canvas/pivot yet, unknown.
- Geometry: Atmosphere only; braille plane, keep clear of ball and tells.
- Brief: Cool low-contrast bath steam, broad calm curls; separate layer from solid contact effects.
- Need: Optional soft curl cycle, frame count unknown; do not add bright fog over gameplay.
- Slot: hard-coded; draw() atmosphere lines.

### mossbell.room — Bathhouse conservatory table

- Role: backdrop
- View: overhead table framed by a shallow front-facing interior.
- Files: none
- States: room=@1
- Timing: Static cached room (room()).
- Fit: 384×224; preserve 200×140 displayed field at (114,46), plunger lane, drain and operator at y=195.
- Geometry: Ball::rail/walls/flippers/BUMPERS own table edges; room() glazing and plants are decorative. Never cover drain or launch lane.
- Brief: Existing copper glazing, leafy silhouettes, bath fittings and enclosed table. Request far conservatory glass/recess, middle foliage/copper framework, table cabinet and near trim separately; dark green/slate with warm copper accents.
- Need: Readable depth layers, cabinet/plunger props, unknown individual sizes and loop counts; avoid a floating table in a void.
- Slot: hard-coded; room() primitives, no room recipe.

### mossbell.chrome — Table plates and podium

- Role: UI plate
- View: screen aligned.
- Files: none
- States: title-hud=@1; chain-values=@3; podium=@1
- Timing: Three CHAIN values shown while any bumper flash>5; podium at 60×HZ end, fresh E rematches (labels()/draw()).
- Fit: 384×224 layout; chain plate height 20 at y=20; result panel (18,20,348,197). Keep Keeper and live standings gutters clear.
- Geometry: Labels, scores, drains, jackpot timing, seat order and payout stay live, not baked.
- Brief: Dark conservatory navy/brass plates, seed/copper trim kept subordinate to text; separate empty plate and edges.
- Need: Title, live strips and podium skins; unknown slice margins, test with longest names.
- Slot: hard-coded; chrome panels, chain glyph cache and live text placement.

## brasswake — Brasswake

Brewery belt brawler: planted punches, a third-contact kick, cask throws into
rival aisles, then the vat keeper. Sources:
[rules](../crates/delve/src/rules/brasswake.rs),
[renderer](../crates/delve/src/render/overworld/arena/brasswake.rs),
[brief](../crates/delve/assets/brasswake/BRIEF.md),
[art notes](../crates/delve/assets/brasswake/ART.md),
[GAME-4B report](/work/artifacts/boss-art-20261015/game-4b/REPORT.md).
Side view with shallow belt depth. World x is in native pixels; visible stage
x=[12,266), y=[34,180). Draw translation: x−camera+12+shake, y−18.
Foot sorting and a cached seat contour preserve the local body during overlaps.

### brasswake.keeper — Vat keeper

- Role: boss
- View: side/front three-quarter, source faces left, cached mirror faces right.
- Files: brasswake/keeper-*.png
- States: idle=keeper-idle; walk=keeper-walk-*; slam-anticipation=keeper-slam-tell; slam-action=keeper-slam; rush-anticipation=keeper-rush-tell; rush-action=keeper-rush; flinch=keeper-flinch; damage-variants=keeper-damage-*; defeat=keeper-defeat; retained-recovery=keeper-recover
- Timing: Twelve source poses; walk 2 frames at 7 ticks each. Tell=max(brass_tell−3×stage,23)=27/24/23; action=9; rest=42/35/28. Flinch=10 after punch. Stage thresholds hp<=21 and <=10 from full 32. Recover file loaded but Rest selects idle/damage, not recover (Foe::stage/step, draw()).
- Fit: 80×80; canvas foot pivot (40,80), top-left (screen x−40,screen y−80). Report occupied height 74 px; do not substitute the older ART.md 64 px cell. Per-pose face anchors in anchors.json keeper[].
- Geometry: Foe::danger: rush 96×20 spanning x to x+dir×96,y−10; slam 48×28 at (x+dir×28−24,y−14). Lane::step_at clips to wave boundaries. Hits test knight feet; art must not enlarge these marks.
- Brief: Copper pear-shaped brewing vat, teal face/lenses, ladle arms, short wooden legs. Proud brewer plants a slam, vents a low steam rush, staggers with broken lens at damage two. Detach ladles, lid/face, legs and steam; copper body, cool lenses, dark-backed warm seam.
- Need: Breathing and actual recovery playback, unknown added frames; preserve existing damage across moves and both facings.
- Slot: png; embedded composites/anchors; damage, mirrored cells, pose selection and exact danger borders hard-coded.

### brasswake.porter — Cask porter

- Role: enemy
- View: side/front three-quarter, left source with cached right mirror.
- Files: brasswake/porter-*.png
- States: idle=porter-idle; walk=porter-walk-*; slam-anticipation=porter-slam-tell; slam-action=porter-slam; rush-anticipation=porter-rush-tell; rush-action=porter-rush; flinch=porter-flinch; damage-variants=porter-damage-*; defeat=porter-defeat; retained-recovery=porter-recover
- Timing: Same 12-pose scheduler and 27/24/23 tell, 9 action, 42/35/28 rest as keeper. Two walk poses at 7 ticks. Flinch=10; damage at hp<=6/<=3 from full 10. Recover loaded but unselected (Foe::step, draw()).
- Fit: 48×48; canvas foot (24,48), top-left screen feet−(24,48); face anchors in anchors.json porter[].
- Geometry: Same rule Danger family as keeper, independent of body size; feet/contact checks in Lane::step_at. Preserve visible local-knight contour at overlaps.
- Brief: Folded slate hood and cap brim over oak-cask torso, keg hoops, mitten fists and wooden boots. Comic barrel weight and braced stumble, not a disposable villain. Detach mittens, boots, hood and hoop pieces; cool hood, warm oak/metal.
- Need: Idle and recover acting, unknown extra counts; all existing attack/damage poses remain registered.
- Slot: png; whole-body arrays/face anchors; mirrored damage/cell conversion and selection hard-coded.

### brasswake.knight — Brawling knight

- Role: player knight
- View: side/front three-quarter, right source, cached left mirror; shallow belt movement.
- Files: brasswake/knight-*.png
- States: idle=knight-idle; walk=knight-walk-*; jab=knight-jab; cross=knight-cross; kick=knight-kick; grab=knight-grab; throw=knight-throw; hit=knight-flinch; knockdown=knight-down; guard=knight-guard; recovery=knight-recover
- Timing: Twelve bodies. Walk 2×6 ticks; jab/cross/kick 10 each, strike cooldown brass_combo=12. Grab=10, throw=12, guard contact=10, flinch=12, down=18, rise=12; hurt protection=24; contact freeze brass_stop=2. Guard/held cask can persist (Lane::hit/step_at).
- Fit: 48×48, canvas foot (24,48), same screen-feet stamp as porter; occupied height 43 px in report. Cloth ramp is I/p/7/o in scripts/brasswake-polish.py; knight() applies the shared hero ink mapping, not Rafterwake's ramp expansion.
- Geometry: Punch reach 38 px, kick 48, y tolerance 13, front tolerance −6; cask pickup distance<=28 and |y−150|<=20. All in Lane::step_at sampled contacts, not fist-image bounds.
- Brief: Shared chunky knight with massive gloves/boots, seat tabard and plume, slate armour. Plant jab, rotate cross, lean into kick, compress under cask then heave. Detach gloves, boots, helm/plume and carried cask.
- Need: Breathing and victory (finish currently idle); optional combo anticipation and follow-through, counts unknown. Jump/climb/shoot are not Brasswake actions.
- Slot: png; cached seat/mirror/contour poses, fixed anchors and action scheduler.

### brasswake.cask — Grabbable and thrown cask

- Role: prop
- View: side; fixed sprite, no rolling-frame bank.
- Files: brasswake/cask.png
- States: cask=cask
- Timing: Static during pickup/hold; rival incoming tell uses brass_tell=27 then burst=12. Throw pose lasts 12, grab 10; no continuous projectile flight sim (Lane::step_at, Run::tick_brasswake).
- Fit: 24×24; floor top-left (screen x−12,py(130)); held top-left (screen knight x−4,py(knight y−52)); incoming uses danger centre−12 and animated lift (draw()).
- Geometry: Incoming Cask::danger is 40×24 at (min(rival x+30,END−40),clamp(rival y−12,114,150)); Danger::contains owns the landing. Rectangle created in tick_brasswake; pickup check in Lane::step_at. Do not paint a longer roll lane.
- Brief: Heavy oak barrel with broad metal hoops; warm wood, cool shadow. Separate from hands, throw streak and slam crown; squash/recoil should feel heavy.
- Need: Roll/turn/contact poses if integration adds them, unknown count; currently one body serves all placements.
- Slot: png; placements/lift and incoming area are hard-coded.

### brasswake.bread — Recovery bread

- Role: pickup
- View: side/front icon; no mirror.
- Files: brasswake/bread.png
- States: bread=bread
- Timing: Static until collected, then disappears; no pickup-specific animation timer (Lane::step_at).
- Fit: 24×24 at (screen x−12,py(135)); centre convention (12,12).
- Geometry: Pickup when |knight x−bread x|<20 and |y−150|<18; restores 2 hp, up to existing 6. Body size must not imply a larger contact radius.
- Brief: Warm humble brewery loaf, broad crust and split; ochre against dark floor, no glowing white food. Detached from any collection bloom.
- Need: Collection squash/bloom, unknown frames/timing and unwired.
- Slot: png; single embedded icon; placement and collection hard-coded.

### brasswake.contact — Punch/guard contact

- Role: effect
- View: side, centred on reported contact.
- Files: brasswake/effects-contact-*.png
- States: contact=effects-contact-*
- Timing: Four frames over effect=12, 3 ticks each; local freeze=brass_stop=2 can hold the clock (Lane::contact/step_at, draw()).
- Fit: 32×32 at (screen effect_x−16,py(effect_y−16)); contact stores effect_y=contact y−20.
- Geometry: Reported sampled contact point; no extra hit area. Draw above relevant bodies.
- Brief: Broad connected warm contact petals with cool edge, comic punch weight; separate core/petals, no sparkle field.
- Need: Distinct guard versus fist variant if wired later; counts unknown.
- Slot: png; four-frame array and shared contact selector.

### brasswake.slam — Grounded slam crown

- Role: effect
- View: side, floor contact; no mirror.
- Files: brasswake/effects-slam-*.png
- States: slam=effects-slam-*
- Timing: Four frames across foe action=9 via floor(age×4/9); incoming cask burst=12 uses 3 ticks/frame (draw()).
- Fit: 48×32; foe top-left (danger screen centre−24,py(danger bottom−32)); cask uses (centre−24,py(danger y−8)).
- Geometry: Decorative splash stays related to the rule Danger rectangle; border remains exact and visible even if splash is taller.
- Brief: Low copper/ochre impact crown, hot restrained core, lobed mass and large connected debris. Separate from cask and dust.
- Need: Preserve both foe/cask registrations; additional smoke/recovery unknown.
- Slot: png; array shared between two timer schedules.

### brasswake.steam — Rush front

- Role: effect
- View: side; current sprite is not mirrored with rush direction.
- Files: brasswake/effects-steam-*.png
- States: steam=effects-steam-*
- Timing: Four frames across 9-tick rush action (draw()).
- Fit: 32×32 at (danger screen centre−16,py(danger bottom−32)). It decorates a 96×20 rule lane, not its entire area.
- Geometry: Foe::danger/draw exact rush borders own the 96×20 footprint and safe space; preserve direction arrows.
- Brief: Solid low cool steam wedge/front, broad two-tone curl, distinct from amber slam. Detached from vat and exact marks.
- Need: Direction-aware cutouts if integration adds mirroring; unknown added count. Do not confuse this solid contact front with ambient braille steam.
- Slot: png; fixed frame bank; directional placement/marks hard-coded.

### brasswake.room — Brewery aisle

- Role: backdrop
- View: side belt stage with shallow floor depth; horizontal camera.
- Files: none
- States: world=@1
- Timing: Static 1200×146 cached backdrop, cropped by lane camera; collision extent END=960 (world()/room()).
- Fit: Visible crop 254×146 at (12,34); top 36 px vat band, lower 110 px floor (world()).
- Geometry: Floor/actors y=126..174 in rule coordinates, wave gates at 240 px intervals; scenery cannot fake a blocked exit or conceal Danger rectangles.
- Brief: Existing copper vats, pipes/supports and slanted floor boards. Request far brewery recess, vat/piping band, quiet walkable floor and near posts/trim separately; room must feel occupied without consuming floor clearance.
- Need: Reusable vat, tap, pipe and floor kit with measured crops; new prop sizes unknown, test. Ambient loops unknown.
- Slot: hard-coded; world()/room() cached primitives, no room manifest.

### brasswake.chrome — Brawler HUD and podium

- Role: UI plate
- View: screen aligned.
- Files: none
- States: title-hud=@1; combo-plate=@1; podium=@1
- Timing: Combo while chain>0 and not down; final podium at completion/75×HZ; fresh E rematches (draw(), text_labels()).
- Fit: 384×224; header above y=34; stage ends y=180; result panel (16,16,352,202). Combo glyphs are dynamic, not a fixed source strip.
- Geometry: Live standings and podium share work/falls/finish order; local seat gutter, health pips and control footer must remain clear.
- Brief: Quiet navy/brass plates, copper trim, flat result fill; detach empty panel/edges. Keep all numbers and labels live.
- Need: Title/panel/podium skins; slice sizes unknown, test full-table/long names.
- Slot: hard-coded; panel primitives, combo glyph caches, metrics and layout.

## rafterwake — Rafterwake

Bellwright workshop loft: spring onto scaffolds, catch ladders, bump the hoist
keeper between chain-drop and floor-sweep commitments. Sources:
[rules](../crates/delve/src/rules/rafterwake.rs),
[renderer](../crates/delve/src/render/overworld/arena/rafterwake.rs),
[recipe](../crates/delve/assets/rafterwake/native.json),
[brief](../crates/delve/assets/rafterwake/BRIEF.md),
[GAME-5C report](/work/artifacts/boss-art-20261015/game-5c/REPORT.md).
Side camera, Q=256 fixed units/px. Visible stage 254×144 at (12,38); transform
(world x+12+shake, world y−camera+38). Floor=540, top=108.
Native recipe, rather than neighbouring PNGs, is the **runtime source** here.

### rafterwake.keeper — Hoist keeper

- Role: boss
- View: side/front three-quarter fixed source orientation; no runtime mirror.
- Files: rafterwake/keeper-*.png
- States: idle=keeper-idle; walk=keeper-walk-*; drop-anticipation=keeper-drop-tell-?; drop-action=keeper-drop; sweep-anticipation=keeper-sweep-tell-?; sweep-action=keeper-sweep; flinch=keeper-flinch; damage-variants=keeper-damage-*; defeat=keeper-defeat; recovery=keeper-recover; retained-base-tells=keeper-*-tell
- Timing: Twelve base poses plus four-stage tells per attack; 2 walk frames×7 ticks. Starts Walk=45; tell=33/29/25, action=10, rest=48/40/32 then Walk=30. Recovery pose when rest ticks<12. Flinch=12, player bump cooldown=24; hp stage 6–5/4–3/2–1 (Keeper::step/stage, Lane::step_at, draw()).
- Fit: 64×80; canvas foot (32,80), top-left (screen keeper x−32,py(TOP)−80). Nested damage_variants in recipe produce two variants per supported acting pose; PNG base-tell files are fallback/loading references, four-stage tells are selected while telling.
- Geometry: Sweep Danger=(16,TOP−19,216,19); drop=(clamp(player x−24,8,192),TOP−100,48,100). Danger::contains uses x left-inclusive/right-exclusive and y top-exclusive/bottom-inclusive. Bump requires rising knight y in [TOP−60,TOP−22], |x−keeper x|<40 and not Tell.
- Brief: Workshop hoist creature: flush rounded faceplate, protected curved amber slit, swept ribbed teal crest, pauldron/hoops, linked chain, open hook and pulley. Lift weight before drop, draw hook back before sweep, spring a hoop/dent helm when damaged. Detach chain, hook, pulley, arms and crest; cool leather/slate masses, restrained amber signature.
- Need: Breathing and more follow-through, unknown extra counts; keep four distinct anticipations, damage variants, named linear details and saved material underlays.
- Slot: recipe; native.json rows/damage_variants/linear_features are swappable data; state names, 64×80 detail plane, sockets, selectors and danger marks remain hard-coded.

### rafterwake.knight — Scaffold knight

- Role: player knight
- View: side, right source with cached left mirror; climb faces the ladder.
- Files: rafterwake/knight-*.png
- States: idle=knight-idle; run=knight-run-*; jump=knight-jump; fall=knight-fall; land=knight-land; climb=knight-climb-*; hit=knight-hit; victory=knight-victory
- Timing: Ten bodies; run 2×5 ticks, climb 2×6. Landing sets pose_ticks=4 (decremented later in same step); respawn land=6, down=18, protection=30, freeze=2. Jump/fall follow velocity; victory held. COYOTE=4, BUFFER=5; movement GRAVITY=112,JUMP=1536,RUN=640,LADDER=384 fixed units (Lane::step_at).
- Fit: 48×48; canvas foot (24,48), top-left screen feet−(24,48). Seat recolour maps I/r/o/t ramp; details remain independent of tint.
- Geometry: supports()/landing() use feet and ±4 px support lip; near_ladder/step_at own rung catch. Body image cannot add reach. Down uses hit pose; no separate knockdown image.
- Brief: Shared heavy-boot knight with huge gloves/helmet; spring off planks, reach with alternating hands and compress on landing. Detach hands/boots, helm/plume and cloth; seat colour stays obvious in overlaps.
- Need: Idle breathing, jump anticipation/release and distinct knockdown/rise, unknown extra frames/timing. Preserve responsive velocity-driven poses.
- Slot: recipe; rows and cached seat/mirror/contour variants exist; pose list, anchors and timing hard-coded.

### rafterwake.plank — Scaffold plank and lift skin

- Role: prop
- View: side, horizontally tiled; no mirror.
- Files: rafterwake/plank.png
- States: plank=plank
- Timing: One tile; lift triangular track uses phase=(elapsed/2+seed%80+deck×11)%80, 160-tick phase cycle (planks()).
- Fit: 24×10 tile, top-left at plank (x,y); repeated every 24 px. Rule widths 232 (bottom/top), 72 (lifts), 140 (fixed), 36 px deck spacing. Final tile currently can overhang rule width; do not treat it as support.
- Geometry: planks(), supports(), landing(); 13 supports including endpoints. Only selected interior decks lift. Surface stays at p.y irrespective of illustrated thickness.
- Brief: Broad timber deck with metal braces, warm muted wood over cool shadow; lift underside stripe remains code-owned. Separate deck, lift trim and rigging.
- Need: Endcap/collision-edge-safe tile integration, unknown new tile count; art alone cannot repair current overhang.
- Slot: recipe; tile rows swappable, tiling width and lift stripe hard-coded.

### rafterwake.ladder — Ladder segment

- Role: prop
- View: side/front along wall; no mirror.
- Files: rafterwake/ladder.png
- States: ladder=ladder
- Timing: Static, body climbs at LADDER=384 fixed units/tick; ladder pose clock is separate (ladder(), Lane::step_at).
- Fit: 20×36 segment top-left (screen ladder x−10,py(deck y)); ladder centre x=120 at interior deck indices 1,4,7,10.
- Geometry: ladder()/Lane::near_ladder require |feet x−ladder x|<=14 and feet y from upper deck−4 to lower deck+4; retain rung height and hand clearance rather than infer climbable pixels.
- Brief: Sturdy timber rungs, iron ties, broad readable rails in dark workshop; detach ladder from wall/rope shadows.
- Need: Optional joint/endcaps, count unknown; registered hand contact reference in delivery metadata.
- Slot: recipe; rows swappable; placement and catch geometry hard-coded.

### rafterwake.brick — Falling workshop brick

- Role: projectile
- View: side; downward fall, no rotation bank.
- Files: rafterwake/brick.png
- States: brick=brick
- Timing: Sparse event elapsed%150==90 when height>72 and <370; warning=30, burst=12. Draw descends 50 px during warning (Lane::step_at, draw()).
- Fit: 24×18; top-left (screen Danger.x+2,py(interpolated y)); pivot convention (12,18).
- Geometry: Danger=(clamp(player x−14,8,204),checkpoint deck y−70,28,70); contact checked once when tell reaches zero. Danger marks must retain the full footprint even though brick art is smaller.
- Brief: Heavy warm-grey/ochre workshop block, simple readable chipped plane; detach dust and impact pieces. Not a glowing projectile.
- Need: Contact/break-up coverage unknown; current burst holds brick near bottom, no distinct brick-impact sprite sequence.
- Slot: recipe; body swappable; interpolation, exact marks and event timing hard-coded.

### rafterwake.post — Loft support post

- Role: prop
- View: side/front background; no mirror.
- Files: rafterwake/post.png
- States: post=post
- Timing: Static tiles in backdrop().
- Fit: 14×36, at x=2/238 every 36 px down the 254×620 backdrop; top-left pivot (0,0).
- Geometry: Decorative only; never imply a new solid wall or hide ladder/plank edges.
- Brief: Dark braced workshop timber with cool metal ties, tactile broad planes. Separate from crossbeams and any ambient light.
- Need: Optional top/base joints unknown; keep repeated pattern quiet in cells.
- Slot: recipe; row tile swappable, repeated layout hard-coded.

### rafterwake.drop — Keeper drop impact

- Role: effect
- View: side, grounded at top deck.
- Files: rafterwake/fx-drop-*.png
- States: drop=fx-drop-*
- Timing: Four frames over 10 attack ticks via floor((10−ticks)×4/10), clamped (draw()).
- Fit: 48×32 at (danger screen centre−24,py(TOP)−32), bottom-centre (24,32).
- Geometry: Decorative drop, never replaces the 48×100 rule Danger or its strengthening marks.
- Brief: Hoist-weight contact lobes with muted ochre force and cool shadow; detach weight/chain from ground burst.
- Need: Preserve contact/release distinction; extra smoke/debris count unknown.
- Slot: recipe; frame rows swappable, sequence/anchor hard-coded.

### rafterwake.sweep — Keeper hook sweep

- Role: effect
- View: side, top-deck floor effect; source orientation fixed.
- Files: rafterwake/fx-sweep-*.png
- States: sweep=fx-sweep-*
- Timing: Four frames over the same 10-tick action schedule as drop (draw()).
- Fit: 48×32 at (danger screen centre−24,py(TOP)−32); effect does not span the entire 216 px danger lane.
- Geometry: 216×19 rule sweep lane and exact marks stay visible independently; source arc cannot imply safety in unpainted parts.
- Brief: Low hook-driven lateral force, cool teal/steel against warm floor marks; detach hook and broad sweep from smoke.
- Need: Direction/travel variants only if integrated; count/timing unknown.
- Slot: recipe; rows swappable; placement and area marks hard-coded.

### rafterwake.dust — Landing and contact dust

- Role: effect
- View: side, at feet behind both figures.
- Files: rafterwake/fx-dust-*.png
- States: dust=fx-dust-*
- Timing: Four frames, 3 ticks each over effect=12; freeze=2 can hold it (Lane::burst, draw()).
- Fit: 48×32 canvas at (screen effect x−24,py(effect y)−32); report requires occupied lobes <=16 px high. Do not fill the empty upper canvas.
- Geometry: Dust is not danger; source position also serves keeper bump/fail feedback. Keep it behind bodies and out of UI.
- Brief: Cool grey/ochre low lobed cloud, broad connected masses; separate from boots, impact and individual debris.
- Need: Distinguish ordinary landing versus heavy impact if wired later, unknown extra counts.
- Slot: recipe; rows swappable, shared event selector hard-coded.

### rafterwake.room — Bellwright loft enclosure

- Role: backdrop
- View: side, vertically scrolling.
- Files: none
- States: backdrop=@1
- Timing: Static 254×620 cached backdrop; camera follows feet (backdrop()/draw(), rules::camera).
- Fit: Crop 254×144 at (12,38), strict y=38..181 viewport; background posts/diagonal braces repeat every 36 px.
- Geometry: planks()/ladder() own traversable geometry, TOP=108 and FLOOR=540. Room decoration must never create a visually false deck or conceal moving lift endpoints.
- Brief: Existing posts and diagonal braces; request far loft recess/bell silhouettes, middle hoist rig and tools, near posts/trim separately. Believable bellwright workshop depth, dark slate/timber with warm work light, no busy black void.
- Need: Layered room kit, pulley/bell/tool dressing with unknown individual sizes; no new traversable geometry. Ambient loop counts unknown.
- Slot: mixed; post recipe exists, backdrop crossbeams/crop/layout are hard-coded.

### rafterwake.chrome — Climb HUD and podium

- Role: UI plate
- View: screen aligned.
- Files: none
- States: title-hud=@1; chain-plate=@1; podium=@1
- Timing: Chain and height follow Lane; podium at finish/75×HZ; fresh E rematches (text_labels()/draw()).
- Fit: 384×224; title/header y<38, gameplay y<182, sidebar begins beyond x=276. Result panel uses (16,16,352,202).
- Geometry: Ranking uses progress/falls/finish; keep longest names, result columns, E AGAIN and footer clear. No baked score glyphs.
- Brief: Dark navy/brass empty plates with workshop trim; separate borders/fill and preserve quiet flat podium.
- Need: Title/panel/podium skins, slice margins unknown, test at real cell sizes.
- Slot: hard-coded; panel geometry, glyph caches and numeric layout.

## pennon — Pennon Rush

Siege muster house: personal seeded rampart, three breaches, three sentry waves
and the pennant captain. Sources:
[rules](../crates/delve/src/rules/pennon.rs),
[renderer](../crates/delve/src/render/overworld/arena/pennon.rs),
[recipe](../crates/delve/assets/pennon/native.json),
[brief](../crates/delve/assets/pennon/BRIEF.md),
[GAME-6 report](/work/artifacts/boss-art-20261015/game-6/REPORT.md).
Side camera, Q=256; world 1200 px wide, FLOOR=112. Visible stage 254×144 at
(12,38), transform (world x−camera+12+shake,world y+38). Native recipe is the
runtime source; PNGs are matching review companions, not loaded textures.

### pennon.captain — Pennant captain

- Role: boss
- View: side/front three-quarter facing left; no runtime mirror.
- Files: pennon/captain-*.png
- States: idle=captain-idle; walk=captain-walk-?; drop-anticipation=captain-drop-tell-?; drop-action=captain-drop; hook-anticipation=captain-hook-tell-?; hook-action=captain-hook; flinch=captain-flinch; damage-variants=captain-damage-?; defeat=captain-defeat; recovery=captain-recover; stage-variants=captain-*-stage-?; retained-base-tells=captain-*-tell
- Timing: Twelve base poses, four-stage tells per attack and stage suffix variants (not temporal frames). Walk 2×7 ticks; wait=45; tell=33/29/25, action=10, rest=60/50/40; recover when ticks<12; flinch=4. Hp stages 18–13/12–7/6–1. Base tell families and four stage-of-damage files are retained but skipped by active key selection (Foe::step, foe_pose(), draw()).
- Fit: 64×80; occupied height 77 in report. Top-left (screen captain x−32,py(FLOOR)−80+2); rule foot maps to local (32,78), not the bottom canvas edge. Signature/linear detail registration in native.json.
- Geometry: Drop Area=(clamp(player x−26,8,WIDTH−60),FLOOR−100,52,100); hook=(max(captain x−156,0),FLOOR−23,156,23). Area::hits intersects player halfwidth 8,height 37 or duck 19. Shot hurtbox is 52×65 at (captain x−26,FLOOR−65), not the 64×80 canvas.
- Brief: Rounded slit face with amber curved light, copper fan crest, split swallowtail pennant on offset mast and open ladder-hook. Overcommitted siege guardian: hoist, pull hook, stagger, fold pennant in defeat. Detach banner/mast, hook/arm, crest and limbs; warm copper/ochre, cool shadow, keep exact marks visible.
- Need: Breathing and deeper follow-through, unknown extra frames; keep four-stage acting through both damaged stages and cell-readable banner/hook.
- Slot: recipe; 104-asset recipe includes captain variants; state-key assembly, sockets, detail masks and danger geometry remain hard-coded.

### pennon.sentry — Wind-vane sentry

- Role: enemy
- View: side/front three-quarter facing left; no runtime mirror.
- Files: pennon/sentry-*.png
- States: idle=sentry-idle; walk=sentry-walk-*; high-anticipation=sentry-drop-tell; high-action=sentry-drop; low-anticipation=sentry-hook-tell; low-action=sentry-hook; flinch=sentry-flinch; damage-variants=sentry-damage-*; defeat=sentry-defeat; recovery=sentry-recover
- Timing: Twelve bodies, walk 2×7; first wait=6, then 45. Tell=33/29/25 at hp 3/2/1, action=10, rest=60/50/40; final <12 rest ticks select recover. Flinch=4. Only captain has four body tell stages; sentry aim line strengthens through four stages (Foe::step, foe_pose()).
- Fit: 48×48; top-left (screen sentry x−24,py(FLOOR)−48+2); rule foot local (24,46).
- Geometry: Shot hurtbox 32×38 at (x−16,FLOOR−38). On action start emits one Shot::aimed from x−16 and FLOOR−26 (high) or FLOOR−16 (low) toward locked aim=(player x,player y−29). Sentry Area is not a damaging rectangle; the aimed bolt is the danger.
- Brief: Twin-prong wind vane, forked wooden bow and hanging seed-weight; comic planted sentry with a readable wind-up and embarrassed fold. Detach vane, bow, weight and arms; dark slate/wood, limited warm glint.
- Need: Four progressive body tell poses if implementation adds them; counts beyond current single tells unknown. Do not paint the captain's floor lane for this enemy.
- Slot: recipe; whole-body rows swappable; aimed-line geometry, bolt emission and scheduling hard-coded.

### pennon.knight — Running hook-bow knight

- Role: player knight
- View: side; right source with cached left mirror, eight-direction simulation aim.
- Files: pennon/knight-*.png
- States: idle=knight-idle; run=knight-run-*; jump=knight-jump; fall=knight-fall; land=knight-land; aim-up=knight-aim-up; aim-down=knight-aim-down; hit=knight-hit; victory=knight-victory; duck=knight-duck; shoot=knight-shoot
- Timing: Twelve bodies; run 2×5 ticks. Land pose_ticks=4 then same-step decrement; respawn land=6. Jump/fall driven by velocity; shoot/aim held by input (not cooldown). Shot cooldown=12; down=18, protection=30, freeze=2, COYOTE=4, BUFFER=5. GRAVITY=112,JUMP=1536,RUN=640 fixed units (Lane::step_at).
- Fit: 48×48, occupied height 45 in report; top-left screen feet−(24,46), foot local (24,46). Recipe e/f/g/h cloth is converted to I/r/o/t then seat-mapped; preserve mask/material distinction.
- Geometry: Enemy-shot hurtbox 16×37 (16×19 duck), centred on feet x; Area::hits uses same dimensions. supports()/landing() from Rafterwake own ±4 px lip. Muzzle x=sample x+dir×18, y=sample feet−10 duck/−29 up/−12 down/−18 level.
- Brief: Shared oversized-helmet knight with forked hook-bow, seat plume/tabard and heavy boots. Spring across breach, tuck low, keep bow attached through aim. Detach bow, gloves, head/plume and cloth; steel/wood excluded from seat mask.
- Need: Aim-direction coverage for diagonal/air/run/duck shots, muzzle flash and hit/knockdown/rise acting; counts/timing unknown. Current code uses jump/fall while airborne and only up/down/level grounded shooting bodies.
- Slot: recipe; cached mirrored/tinted bodies; aim-specific pose coverage, muzzle socket selection and hit/down selectors hard-coded.

### pennon.bolt — Player hook-bow bolt

- Role: projectile
- View: side; travels in eight aim directions, current picture is never rotated.
- Files: pennon/bolt.png
- States: bolt=bolt
- Timing: One source; life=30 ticks, axial speed=2048 fixed units/tick, diagonal component=1448; firing cooldown=12 (Lane::step_at).
- Fit: 16×8 centred on shot position, anchor (8,4); keep a solid terminal core.
- Geometry: segment_hits performs swept point-segment vs foe rectangles; no art-width collision expansion. Player muzzle coordinates in knight block.
- Brief: Readable warm shaft/core with dark contour, distinct from enemy seed bolt. Detached from muzzle flash and impact.
- Need: Directional/rotated bank and launch flash if integrated, count unknown; current single orientation is a gap, not eight existing images.
- Slot: recipe; row swappable; no rotation or per-direction asset keys yet.

### pennon.enemy-bolt — Sentry seed bolt

- Role: projectile
- View: side, arbitrary integer aim; current picture unrotated.
- Files: pennon/enemy-bolt.png
- States: enemy-bolt=enemy-bolt
- Timing: One source, life=180; Shot::aimed uses max-axis normalisation and max component 512 fixed units/tick. One bolt per sentry action, not a volley.
- Fit: 12×12 centred on shot position, anchor (6,6).
- Geometry: segment_hits vs 16×37 or duck 16×19 player box; tell line ends at locked sample aim, projectile continues on that vector. No radius from sprite bounds.
- Brief: Distinct cool/earth seed-weight projectile with restrained warm seam; no white core or tiny bullet rain. Detached trail/impact if supplied.
- Need: Travel-direction coverage or symmetric readable silhouette; extra frames/timing unknown.
- Slot: recipe; body swappable; emitter, aim and collision hard-coded.

### pennon.repair — Repair shield pickup

- Role: pickup
- View: side/front icon, fixed orientation.
- Files: pennon/repair.png
- States: repair=repair
- Timing: Two placements at world x=340/848, static until collected; grants ward=1, collection burst=12 (Lane::step_at).
- Fit: 24×24, bottom-centre anchor (12,24); at (screen x−12,py(FLOOR)−24).
- Geometry: Collect when |player x−pickup x|<16 and player feet y>FLOOR−28. Ward absorbs next fail, then hurt=30; no new shield hitbox.
- Brief: Repair shield, broad readable metal/cloth form in dark room; pickup warmth distinct from enemy bolt and floor warning. Detach collection bloom.
- Need: Ward activation/break visual and collection acting, unknown frames/timing; current pickup simply disappears and shares contact burst.
- Slot: recipe; icon swappable, ward visual support still implementation work.

### pennon.merlon — Slate parapet dressing

- Role: prop
- View: side/front masonry, repeated fixed orientation.
- Files: pennon/merlon.png
- States: merlon=merlon
- Timing: Static, placed every 64 px within each rampart section (backdrop()).
- Fit: 32×40 at (world section x+offset,FLOOR+4); baked to world 1200×144 and camera-cropped.
- Geometry: Decorative, not a separate solid obstacle. Placement must leave the ramparts() breach edges unambiguous.
- Brief: Rounded blue slate merlon, curved iron tie rods, broad masonry bands; cool quiet mass under the actors. Detach rods if required for cell readability.
- Need: End/transition variants only if integration needs them; unknown count.
- Slot: recipe; body swappable, bake placements/crop hard-coded.

### pennon.brick — Rampart masonry tile

- Role: prop
- View: side, tiled floor face; no mirror.
- Files: pennon/brick.png
- States: brick=brick
- Timing: Static repeating background in backdrop().
- Fit: 32×16; tiled across a 32 px tall floor face beginning at FLOOR, **clipped per pixel at each physical breach edge**.
- Geometry: ramparts(seed) owns four section spans and three gaps; supports()/landing() own feet. Do not bridge a breach with painted overhang.
- Brief: Rounded dark blue slate with iron ties, calm broad form steps; readable lip above darker wall face, no dense high-contrast mortar stripes.
- Need: Alternate endcaps if art requires them, unknown count; preserve code clipping and support plane.
- Slot: recipe; tile rows swappable, section geometry and per-pixel clipping hard-coded.

### pennon.pennant — Rampart signal pennant

- Role: prop
- View: side/front, fixed asymmetric split cloth; no mirror.
- Files: pennon/pennant.png
- States: pennant=pennant
- Timing: Static at world x=36/302/576/856/1172 (backdrop()).
- Fit: 32×48 at y=FLOOR−48; lower edge is the mount plane, anchor convention (0,48), exact mast socket unknown.
- Geometry: Decorative; no new cover or hazard. Keep breach lips and captain banner distinguishable.
- Brief: Split cloth signal and offset mast, muted warm cloth/cool fittings; same siege-house language as captain, quieter than the held boss pennant. Detach cloth/mast.
- Need: Small wind cycle and mounting reference, unknown frames/timing.
- Slot: recipe; rows swappable, static layout and lack of wind clock hard-coded.

### pennon.drop — Captain banner-drop impact

- Role: effect
- View: side, floor anchored.
- Files: pennon/fx-drop-*.png
- States: drop=fx-drop-*
- Timing: Four frames over 10 action ticks via floor((10−ticks)×4/10), clamped (draw()).
- Fit: 48×32, bottom-centre (24,32) at the rule danger centre and FLOOR.
- Geometry: 52×100 drop Area remains simulation-owned; burst art never replaces exact boundary marks.
- Brief: Broad grounded banner/weight impact mass, warm restrained core and cool contour, distinct vertical commitment. Detach banner/weight and dust from impact core.
- Need: Follow-through/release if wired later, unknown count; no painted larger impact radius.
- Slot: recipe; sequence, danger projection and solid terminal overlay hard-coded.

### pennon.hook — Captain low hook force

- Role: effect
- View: side, repeated low lane effect, fixed source direction.
- Files: pennon/fx-hook-*.png
- States: hook=fx-hook-*
- Timing: Four frames over 10 action ticks, same index as drop (draw()).
- Fit: 48×32 at each world x in danger lane in 40 px steps, bottom at FLOOR; repeated sprites can overlap by 8 px.
- Geometry: Rule hook Area=156×23; visual repetition is decorative and does not extend collision. Danger rectangle continues to show the exact safe boundary.
- Brief: Low lateral hook sweep/force, cool edge and muted brass, clearly different from vertical drop. Detach hook, force lobe and dust.
- Need: Terminal-clean repeated bank and optional travel phase, extra count unknown; do not add bullet density.
- Slot: recipe; frame bank swappable, repetition, bounds and masking hard-coded.

### pennon.impact — Shot and ward contact

- Role: effect
- View: side, contact centred.
- Files: pennon/fx-impact-*.png
- States: impact=fx-impact-*
- Timing: Four frames, 3 ticks each over effect=12; freeze=2 can hold. Used when effect_y<FLOOR (Lane::burst, draw()).
- Fit: 48×32, centre (24,16) at effect position; actor hits store contact at (foe x,FLOOR−30).
- Geometry: Decorative output of swept contacts; adds no splash damage. Solid effect stamps mask braille details behind them.
- Brief: Chunky warm hit star/lobes with dark contour, short readable force; separate from bow, bolt and smoke, no near-white core.
- Need: Distinct ward-break/pickup variants if integration separates shared events; counts unknown.
- Slot: recipe; rows swappable; event sharing, timer and terminal masking hard-coded.

### pennon.dust — Landing cloud

- Role: effect
- View: side, feet anchored, behind figures.
- Files: pennon/fx-dust-*.png
- States: dust=fx-dust-*
- Timing: Four frames, 3 ticks each over effect=12; chosen for effect_y>=FLOOR (draw()).
- Fit: 48×24, bottom-centre (24,24) at effect point. Keep low lobes and unused upper canvas rather than increasing cloud height.
- Geometry: No danger or support; preserve breach edge, feet and shots in foreground.
- Brief: Cool grey/ochre low connected dust, muted compared with contact stars; separate from boot, parapet and impact core.
- Need: Optional takeoff/land distinction, unknown extra frames/timing.
- Slot: recipe; frame bank swappable, shared contact classification hard-coded.

### pennon.room — Siege muster rampart

- Role: backdrop
- View: side, horizontally scrolling; personal seeded camera.
- Files: none
- States: seeded-backdrops=@3
- Timing: Three static seed%3 layouts cached; seed shifts section edges by 0/4/8 px (ramparts(), backdrop()).
- Fit: 1200×144 world, 254×144 crop at (12,38). Existing very dim distant recesses, masonry sections, merlons and signal pennants.
- Geometry: ramparts()/supports()/landing() govern gaps; foreground may not bridge empty sections or hide support lips. Camera clamp is inherited from Rafterwake.
- Brief: A siege muster house presenting its rampart exercise: far dark fortification depth, middle parapet/rods/signals, near trim and supports. Keep the black distance but add readable lit architectural masses, separate from traversable edge art.
- Need: Reusable recess/beam/stone/cloth depth kit, individual dimensions unknown, test; any parallax or moving cloth requires implementation.
- Slot: mixed; brick/merlon/pennant recipe slots exist, world layout/recesses/camera and depth composition hard-coded.

### pennon.chrome — Run-and-gun HUD and podium

- Role: UI plate
- View: screen aligned.
- Files: none
- States: title-hud=@1; combo-plate=@1; podium=@1
- Timing: Live chain and work, final result at finish/75×HZ; E rematch on next fresh press (text_labels()/draw()).
- Fit: 384×224; gameplay x=12..265,y=38..181; title above, sidebar beyond x=276, controls below. Result panel (16,16,352,202).
- Geometry: Live and podium ranking share work/falls/finish; keep local YOU gutter, ward indicator and control footer readable. No baked numbers or changed wording.
- Brief: Quiet navy/brass plates with muted siege-house trim; separate empty fill/border and title ornament. Preserve black surroundings and flat podium.
- Need: Title/panel/result skins with unknown slice margins; test longest names and full table in real cells.
- Slot: hard-coded; chrome, glyph caches, numeric layout and labels.

## Cross-cutting delivery needs

These are families to coordinate across the **67 request slots**, not extra
untracked assets. Counts are current pose/source counts, not a demand to create
new mechanics. The generator gives the exact filenames/canvases per slot.

| Knight destination | Existing coverage | Missing visual coverage to plan |
| --- | --- | --- |
| Cinder | Shared dynamic hero figure, no puzzle pose bank | Watch, turn/drop, clear reaction, cough flinch, victory; count/timing unknown |
| Latch | 2 guards + 3 counter bodies | Idle breath, miss collapse/recover, victory; count unknown |
| Mossbell | 4 rear lever combinations | Swing/release, launch, nudge, drain slump, jackpot cheer; count unknown |
| Brasswake | 12 bodies: idle, 2 walk, jab/cross/kick, grab/throw, hit/down, guard/rise | Breathing and victory; jump/climb/shoot not in game |
| Rafterwake | 10 bodies: idle, 2 run, jump/fall/land, 2 climb, hit/victory | Anticipation/release, separate knockdown/rise; count unknown |
| Pennon | 12 bodies: idle, 2 run, jump/fall/land, aim up/down, hit/victory, duck/shoot | Eight-direction weapon registration across run/air/duck, knockdown/rise, muzzle flash; count unknown |

Keep one shared knight design/seat mask convention and separate game pose banks;
do not enlarge all bodies to Latch's presentation size. Existing body poses and
their canvas margins are the registration target. The art team's knight design
is authoritative; older source/converter prompts are identity evidence, not the
new style target.

| Effect family | Current source count and scheduling | Gaps / delivery caution |
| --- | --- | --- |
| Cinder clear/slag/impact | Clear 3 (8 ticks), slag 1, impact 4×3 ticks, smoke 1 + terminal counterpart | 8 sampled pot flights and CHAIN glyph cache are procedural; no separate explosion/dust bank |
| Latch parry/clash/strike | 4 each over 18-tick recover plus freeze; clash last frame also on miss | Three overlapping contact layers, no projectile/explosion/smoke bank |
| Mossbell contact/jackpot | Impact 4 stored, ordinary contact uses last 3; jackpot 4×4 ticks | Bumper burst 4 stored but unused; 3 procedural target shapes×4 flash frames; steam procedural; no explosion bank |
| Mossbell danger | Root 4, crown 4, tell 36/30 then action 12 | Marks must retain exact gate/band; ordinary VFX must not masquerade as danger |
| Brasswake hit/slam/steam | 4 each; hit 12 ticks, foe action 9, cask burst 12 | No separate muzzle, explosion or ambient-smoke bank; cask is a targeted landing, not a travelling bullet |
| Rafterwake drop/sweep/dust | 4 each; action 10, dust 12 plus freeze | No separate general hit/explosion bank; dust currently handles bump/fail/landing feedback |
| Pennon drop/hook/impact/dust | 4 each; action 10, contact/dust 12 plus freeze | No muzzle-flash, explosion or smoke source bank; pickup/ward shares impact; bolts each 1 static orientation |

The named games need distinct tells, contact cores and payoff acting first.
Explosion, smoke, dust and muzzle-flash requests with no current event/slot
remain **unwired with unknown counts/timing**. Deliver separate optional source
layers only after their event/registration contract is agreed; no decorative
effect should imply splash damage or obscure safe gaps. CHAIN/COMBO, jackpot
status, live standings, title labels and result numbers remain rendered text.

Environment delivery should contain far enclosure, middle identity props and
near trim as independently placeable crops. The per-game room blocks describe
what exists and the empty areas to improve. Ground surfaces and playable
silhouettes must remain quiet enough for exact tells and players. Individual
new prop dimensions, slice margins, layer scroll rates and ambient animation
counts are unknown until layout trials. UI requests are empty skins for the
existing title/panel/podium geometry, not new copy or decorative baked scores.
Pickups are Brasswake bread/casks and Pennon repair shields; Cinder pots and
Mossbell target lights are rule objects, not gold pickups. Gold/crowns are
results; none of these games has an independently animated gold-drop field.

## 3D requests for GLM in Blender / TRELLIS

The repository has a shared triangle rasterizer and three true 3D race courses
([race3d rules](../crates/delve/src/rules/race3d.rs),
[race renderer](../crates/delve/src/render/overworld/arena/race3d.rs),
[world3d mesh](../crates/delve/src/render/world3d/mesh.rs)). These six mini-game
renderers are 2D `Img` compositions and have **no 3D gameplay scene or per-game
3D room asset loader**. The world3d code's old `docs/plans/world3d-spec.md`
reference points to a file absent on this branch; do not use it as a delivered
room contract. Any 3D work below is a deferred source-kit request, not an
implemented room mode or a reason to block the sprite deliveries.

| Building / game | Current 3D presence | Useful deferred kit | Measured size / units |
| --- | --- | --- | --- |
| Alchemy Lab / Cinder | None for this game | Kiln, pot racks, workbench, masonry bay | Actual 3D dimensions unknown; 2D board cells 24×20 px are not world metres |
| Dented Anvil / Latch | None for this game | Practice alcove, beams, anvil, tool rack, bellows fixtures | Actual 3D dimensions unknown; 100 px Clasp presentation is not a 3D scale |
| Bathhouse conservatory / Mossbell | None for this game | Table cabinet, plunger/levers, copper glazing, bath/plant fittings | Actual 3D dimensions unknown; 160×140 physics table is in 2D pixels |
| Brewery / Brasswake | None for this game | Vat row, taps/pipes, barrel, timber aisle kit | Actual 3D dimensions unknown; 960 px rule aisle is not world metres |
| Bellwright workshop / Rafterwake | None for this game | Modular scaffolds/ladders, hoist/chain/pulley, bell supports | Actual 3D dimensions unknown; deck spacing 36 native px, not a 3D unit contract |
| Siege muster house / Pennon | None for this game | Rampart sections/endcaps, merlons, mast/pennants, braced recess | Actual 3D dimensions unknown; 1200 px stage is not a 3D room dimension |

Existing `race3d.rs` states one world unit per map tile; `Mesh::Tri` has vertex
positions, UVs, semantic material id and normal, with z-up rotation in
`Mesh::rotated_z`. No automatic conversion from these mini-game pixels exists.
Poly/triangle budget, texture budget, authoring-to-runtime import format, room
dimensions and pivots are **unknown**; an integrator must measure/agree them
before production. Keep a scale marker, named object pivots, separate moving
parts, semantic material assignments and a collision-free decorative version
in any proposed kit. Blender source/project plus a proposed mesh export can be
offered, but no glTF/FBX loader is promised by this code. Author neutral material
roles for the existing dark shader, not baked white lighting or an emissive
full-room fill. Cosmetic meshes never replace simulation collision.

## Delivery priority

1. **Knights and their immediate contacts:** Pennon run/air/aim/duck/shoot,
   Rafterwake jump/climb/land, Brasswake combo/grab/throw/guard, then Latch
   guard/counter and Mossbell's operator. These show every input and determine
   whether the shared knight identity survives the actual small cell grain.
   Match existing source counts first, provide registered weapon/hand sockets.
2. **Boss anticipation → action → recovery kits:** captain and hoist keeper
   with four-stage tells and damage coverage; then vat keeper/porters, Clasp,
   Keeper and Crock. Their distinct committed actions carry pressure without
   more bullets. Deliver exact-geometry-compatible VFX beside the actors.
3. **Hit, landing and payoff effects plus critical props:** existing contact,
   slam, drop/hook/sweep, clear/jackpot strips; readable bolts, repair shield,
   cask/bread, table targets and paddles. Fill the missing muzzle/ward/recovery
   coverage after implementation agrees event slots. Preserve all boundaries.
4. **Room depth kits and empty UI skins:** brewery vats, loft rigging and siege
   parapet depth first (largest scrolling stage areas), then lab/smithy/
   conservatory dressing. Quiet layered occupancy improves feel without masking
   play. UI skins follow verified live text geometry.
5. **Can wait:** unused Mossbell bumper strip, retained legacy impact and
   unselected recovery files, optional atmospheric loops, additional facings
   beyond current cameras, and all deferred 3D room kits. Confirm wiring first.

## Slot-readiness audit and implementation queue

Every block's `Slot` is the asset-level audit. `png` means an embedded file can
be replaced at the existing size/name and recompiled; it does **not** mean a
manifest-driven scheduler. `recipe` means native rows/metadata are consumed;
it does **not** mean arbitrary states, sizes, sockets or durations are wired.
`mixed` splits these responsibilities. `hard-coded` needs an implementation
slot before a mechanical art swap. No loader or renderer changes are made here.

| Game | Data-backed now | Hard-coded / missing integration work |
| --- | --- | --- |
| Cinder | Crock/action/damage/seam PNGs, miniature, smoke/tell, slag RGB, clear, impact | Knight-specific pose bank; pot/ghost/slag-cell recipe; room/depth slots; exact danger marks; glyph/UI skin slots; body/face/damage sockets and terminal overrides; unused impact.png |
| Latch | Clasp, 5 knight composites, 3 four-frame effect banks | Pose/eye/damage sockets, cached 80→100 resize, terminal face registration; room/depth and empty UI skins; missing idle/miss/recovery acting |
| Mossbell | Keeper PNGs plus signature metadata, 8 vine stages, 3 effect banks | Knight, targets/flash rims, ball/trail, paddles, steam, room and UI slots; crop/x-projection/floor-layer schema; unused bumper bank; contact frame-0 policy |
| Brasswake | 3 character PNG sets, face-anchor JSON, cask/bread, 3 effect strips | Body/weapon/foot sockets; pose scheduler/unused foe recovery; room/depth, exact marks and UI skins; shared-event effect variants |
| Rafterwake | All actor/prop/effect rows in native.json, nested damage variants and tagged detail metadata | Fixed keys/pose arrays, 64×80 details plane, sockets, selectors; backdrop/depth/UI slots; collision-safe plank endcaps; new knockdown/rise coverage |
| Pennon | All actor/prop/shot/effect rows and captain variants in native.json | Fixed keys/pose arrays, sockets; 8-direction weapon/bolt/muzzle coverage, ward visuals; backdrop/depth/UI slots; exact area/aim-line generation and cell masks |

Before final art integration, give the listed procedural families explicit
asset recipes/manifests with registered pivots and optional layers, and keep
danger drawing bound to simulation geometry. Extend animation selection only
within existing rule durations, baking tint/mirror/material/cell variants in
OnceLock. After integration, inspect native and actual cells and measure smoke
frame cost; historical report measurements do not certify a new art swap.

Open decisions for operator/art team: final source banks for these new rivals
are not identified by this branch; use the existing knight art direction and
retain documented game identities while the art team selects designs. Larger
canvases, extra animation frame counts, per-part sockets and 3D budgets remain
unknown. The older ART.md dimensions/prompts sometimes predate polish (notably
Cinder, Latch and Brasswake); measured files and active code above take priority.

## Damage integration follow-up (2026-10-15)

The reviewed supplemental sources are registered under `assets/art-slots/*-damaged`.
Retain each delivered identity and camera; these requests repair gaps in those
banks. Deliver isolated RGBA PNG cells, measured crop JSON, source foot pivots
and a labeled reading-order manifest. Use the established dark material ramps,
warm contours and original equipment. No effects may hide the exact rule marks.

| Asset id / game | Missing states and frame count | Timing / native registration / geometry |
| --- | --- | --- |
| `latch.clasp-damaged` / Latch | Severe low windup, low swipe and settled recovery: 1 frame each | Existing 36-tick tell; attack/recovery share existing 18-tick Miss, Rest uses existing `latch_rest`; 80x80 at (40,80), displayed on the same floor. Copper hook rival faces the knight at left; high/low contact lanes remain 25x2 at y120/144. |
| `mossbell.keeper-damaged` / Mossbell | Complete moderate and severe straight punches: 1 frame each; severe crown action: 1 | Root/Crown action remains 12 ticks, tell 36/30 by HP. 48x48 at (24,48). Keep the entire fist and leaf silhouette inside the source crop; current two punches have cut-off fingers. Jade leaf/pod guardian, amber eye and sap. Body is decorative; root y72..82 and crown x60..100,y20..43 remain authoritative. |
| `brasswake.keeper-damaged` / Brasswake | Severe rush windup, rush, recovery after slam and recovery after rush: 1 each | Existing tell 23 ticks, action 9, Rest 28 with final 12 ticks for recovery. 80x80 at (40,80), same 3/2 display fit. Copper vat, blue lens, cup hands and hanging cloth; preserve exact rush 96x20/slam 48x28 danger rectangles and lane clipping. |
| `rafterwake.keeper-damaged` / Rafterwake | Severe lowered-weight impact, sweep windup, sweep and two recoveries: 1 each | Existing tell 25, action 10, Rest 32 with final <12 ticks for recovery. 64x80 at (32,80); keep body feet fixed while the bucket/hook moves. Slate hoist armor, teal crest, apron, chain and weight; danger remains full-width sweep/drop from the rules. Existing severe raised weight is anticipation, not an impact. |
| `mossbell.flippers` / Mossbell | Four complete registered pairs: both down, left up, right up, both up; 1 each | 114x26 source contract; floor pivots at rule (37,119)/(123,119), tips (70,130)/(90,130) down and (70,111)/(90,111) up. Independent five-tick presses; display x-projection is handled by integration. Every pair must retain both outer pivots. Current raised pair moves pivots inward and cannot register. |
| `mossbell.root` / Mossbell | Four rightward and four mirrored leftward advance fronts: 1 each | 179x11 horizontal bands, bottom center anchor. Existing tell advances four stages across 36/30 ticks and action lasts 12; front must span x7..150 in projection, y72..82. Teal roots and amber leading bud; no tall curls requiring the whole strip to shrink horizontally. |
| `mossbell.crown` / Mossbell | Open, half, near-shut, shut horizontal gate: 1 each | 51x24 source contract, center anchor; existing tell 36/30 ticks and action 12. Upper/lower leaves close vertically across the whole x60..100,y20..43 landing rectangle. Current square clusters shrink to a narrow column at aspect-preserving fit; preserve a wide horizontal gate. |

Crock has no independent punch rule; its unused delivered punch drawings remain
proposals. Pennon's existing standing damage and defeat remain active, with the
supplemental walk/limp/punch/recovery poses wired inside existing counters.

# Art requests

## Bosses

These slots request the library artist's designs. Existing effects stay active until delivered and reviewed. This section is independent of mini-game requests. Camera: oblique overhead arena, orthographic screen sprites; projectile master faces east, with four cached quarter turns. No overhead cutout rotated into a side camera. All deliveries: lossless RGBA PNG, binary alpha at native grain, shared dark inks, no baked floor/background/text. Include source sheet, crop rectangles, SHA-256, centre socket, opaque bounds and material/identity regions. Supply v1 masters at the sizes below; v2 currently stamps the same simulation-space effect geometry at twice the sampling density. Higher density must not expand a danger footprint.

The runtime reads `scripts/boss-effect-slots.json`: per boss, 2 projectile / 4 tell / 5 impact filenames. Import and validate replacements, change those slots, rebuild. The build embeds files and tracks every manifest/file input; decode, turns, warning geometry and variants are cached. Sprite changes cannot change release rules, counts, damage, collision or warning duration.

| Asset id | Game / role | Native v1 size, anchor and footprint | States / rule timing | Identity brief |
| --- | --- | --- | --- | --- |
| iron-tusk-chief/projectile | Delve / travelling shot | 24×24 transparent canvas; centre anchor; bone/ember shot collision unchanged (point shot expansion 0.16 units); visual stroke must stay compact around centre | 2 looping flight frames, 4 ticks each; real aimed 3-shot volley every 65 ticks at speed7; lead gap-wall every 98, 2 volleys; no invented aimed-volley tell | Heavy capped-iron/ore language matching tusks, spear and slab buckler; replace thin slashes with the artist's chunky readable projectile |
| iron-tusk-chief/tell | Delve / outgoing rage directions | 24×24 author canvas, centre; cached 15×15 rotated motifs on the existing 72-direction table; outgoing marks at existing reach38px, stone clipped; safe lane untouched | 4 stages indexed by existing 36-tick ring progress (9 ticks/stage); existing desperate arms window66..90 =24 ticks; gap-wall wait 24 has mark=false, so no fabricated landing mark | Ore/steel anticipation motif that leaves the exact real lane open; include separate source anticipation decoration if needed, never imply an area larger than the rays |
| iron-tusk-chief/impact | Delve / hit burst | 28×28; contact-centre; cosmetic, no collision | 5 frames over the existing 6-tick impact:2,1,1,1,1 ticks; no added hold | Weighty chipped iron/ore impact matching the buckler and spear |
| cinderjaw/projectile | Delve / travelling shot | 9×9, centre; unchanged projectile centre/collision and trajectory | 2 flight frames×4 ticks; fan 5 arc 55 speed 5 every 95, spiral 2 speed 4 every 20; lead bounce/mines every 91, 2 volleys | Tactile furnace slag/ember with a substantial outlined body, matching oak/iron and the belly fire grate |
| cinderjaw/tell | Delve / mine landing and rage rays | 24×24 author canvas; centre; runtime point-mark clips/resamples to 11×11 radius5px =ceil((HERO_RADIUS+0.16)×8); mine location is the actual discrete braking endpoint and stone reach | 4 warning stages; mines keep full 51-tick fuse, stages by floor(age×4/51); ring 36 ticks; arms 24 ticks; no ordinary-volley windup | Furnace pressure/iron jaw anticipation matching the delivered body; danger boundary remains the exact point-hit envelope |
| cinderjaw/impact | Delve / hit burst | 28×28, contact-centre, cosmetic only | 5 frames over 6 ticks:2,1,1,1,1 | Chunky hot slag and outlined smoke rather than sparse sparks; restrained peak light, dark contours |
| mother-of-spores/projectile | Delve / travelling shot | 9×9, centre; unchanged projectile/collision core | 2 flight frames×4 ticks; fan 7 arc 70 speed 7 every 70, aimed 2 speed 9 every 42; split/mines/crossfire every 78, 2 volleys | Match russet/cream cap, orange gills, moss and roots; replace mismatched purple-green body language while preserving threat readability |
| mother-of-spores/tell | Delve / mine landing and rage rays | 24×24 author canvas; centre; same clipped 11×11 point-hit envelope/radius5px; marks read real held-shot position or braking endpoint, never body envelope | 4 stages; mine 51-tick fuse; held-shot warning 24 ticks (stage mapping wait 23..0); ring 36 ticks; arms 24 ticks; no invented aimed/fan advance window | The artist's cap/gill/root anticipation motif, with the danger boundary unobscured and distinct from the body |
| mother-of-spores/impact | Delve / hit burst | 28×28, contact-centre, cosmetic only | 5 frames over 6 ticks:2,1,1,1,1 | Russet/cream/orange organic burst with outlined dust/spore lobes matching the library matron |
| the-index/projectile | Delve / travelling shot | 9×9; centre; existing collision and quarter turns | 2 flight frames×4 ticks; spiral 3 speed5 every12; aimed 3 speed8 every55; curve/weave/seeker every65, 2 volleys | Match blue book bindings, brass corners and cream folded pages; clearly separate travelling shots from decorative bookmarks |
| the-index/tell | Delve / held-shot lane and beam | 24×11 tile, centre; renderer repeats the tile along the authoritative stone-clipped lane, with its existing half-width; never paint a wider corridor | 4 progress stages; held-shot wait24; beam warn21, burn18; rage36 and arms24 retain their existing rules; no new ordinary-volley windup | Book/page/brass anticipation; preserve visible baseline, advancing fill, exact beam edge and safe lane. Source pose is decoration, not danger geometry |
| the-index/impact | Delve / hit burst | 28×28; contact-centre, cosmetic | 5 frames over6 ticks:2,1,1,1,1 | Outlined folded-page/ink burst matching the delivered tome; keep hostile ink distinct from the body |
| waxen-warden/projectile | Delve / travelling shot | 9×9; centre; unchanged point collision | 2 flight frames×4 ticks; aimed1 speed6 every65; gap_ring/reaim/flower every78, 2 volleys | Candle/wax language matching ivory body, blue mantle and brass; substantial outlined silhouette |
| waxen-warden/tell | Delve / outgoing rage rays | 11×11 master, centre; cached ring glyphs on authoritative stone-clipped outgoing rays; never a fabricated mine or landing circle | 4 stages over existing36-tick rage ring (9 each); arms24; reaim selects its target on release | Wax/candle anticipation; clear safe gap and outgoing directions; do not imply the hovering body envelope is a hit area |
| waxen-warden/impact | Delve / hit burst | 28×28; contact-centre, cosmetic | 5 frames over6 ticks:2,1,1,1,1 | Chunky wax lobes and outlined candle smoke matching the weary keeper |
| the-bone-choir/projectile | Delve / travelling shot | 9×9; centre; unchanged point collision | 2 flight frames×4 ticks; ring12 speed4 every140; aimed3 speed6 every70; gap_ring/burst/flower every72, 2 volleys | Bone/music language matching three skulls, ribcage and red robe; no extra shot silhouettes that suggest more collisions |
| the-bone-choir/tell | Delve / outgoing rage rays | 11×11 master, centre; exact simulation rays, stone clipping and safe gap | 4 stages over36-tick rage ring; arms24; existing volleys retain their own release cadence | Conductor/bone anticipation with readable directional gaps; decorative notes must not look like travelling damage |
| the-bone-choir/impact | Delve / hit burst | 28×28; contact-centre, cosmetic | 5 frames over6 ticks:2,1,1,1,1 | Outlined bone-note burst and dark red dust, matching the choir's delivered materials |
| crypt-tithe-marshal/projectile | Delve / travelling shot | 9×9; centre; unchanged point collision | 2 flight frames×4 ticks; aimed3 speed6 every75; gap_ring/burst every98, 2 volleys | Bone/brass seal language matching square helm, staff and red tabard; keep projectile distinct from keys |
| crypt-tithe-marshal/tell | Delve / outgoing rage rays | 11×11 master, centre; exact outgoing ray reach, stone clipping and safe gap | 4 stages over36-tick rage ring; arms24; no new aimed-volley windup | Staff/seal anticipation with clear outgoing directions; staff art does not grant melee reach |
| crypt-tithe-marshal/impact | Delve / hit burst | 28×28; contact-centre, cosmetic | 5 frames over6 ticks:2,1,1,1,1 | Chunky brass/bone seal impact with dark contours and restrained steel glints |

Delivery BOSSFX-1: all 21 slots above are integrated through `fx-v1-*` filenames (and native `fx-v2-projectile-*` flight banks). The source crops, full-sheet/crop hashes, cell mappings, raw/prepared fidelity and old/new active-file hashes are retained in `crates/delve/assets/boss-effect-src/` and each boss's `effect-import.json`. Normal builds include Waxen/Choir effects independently of their inactive library bodies. Flight2×4 ticks, tell4 stages and impact2/1/1/1/1 are unchanged. Heavy Tusk flight cells6/8 replace the unreadable compact pair; no extra trails or shot count. Other variants stay unwired.

Next effect requests follow the body rollout order below. Each row requests three separate assets `bossfx.<id>.{projectile,tell,impact}`: east-facing2×4-tick centre-registered flight; four warning stages only on the listed real patterns (36-tick rage,24-tick held shot,51-tick mine,21-tick beam as applicable); five contact-centred impact frames over6 ticks (2/1/1/1/1). Deliver rich transparent source + measured crop JSON/SHA, native v1/v2 masters, anchors and material regions. Projectile size is per-boss review data; point collision stays9×9/0.16. Tells must skin exact rays/stone clipping/safe gaps,11×11 point marks or24×11 lane tiles; impacts28×28 are cosmetic. Existing designs, `.boss` definitions and signatures own identity and timing; no new windup or danger circle.

| Next asset id prefix | Existing attacks / real warning geometry | Library identity to retain |
| --- | --- | --- |
| crypt-tax-keeper | ring6 every95; burst/reaim91; outgoing rage rays | Reliquary tax lord, closed ward/seal materials |
| crypt-relic-liege | ring6/85, aimed2/60; gap_ring/flower/reaim78; outgoing rays | Crown/reliquary bone and brass |
| red-pick-chief | ring6/95; bounce/mines98; actual braking endpoints | Powder captain, pick/ore and iron |
| the-foreman | fan5/85, aimed2/50; gap_wall/bounce/lead78; no mark for gap_wall | Harness, lamp, whip and quota equipment |
| late-fee-leviathan | Ravage5/210, aimed2/45,fan5/95; weave/curve/laser72; exact Ravage bands50° gap and laser lane | Library debt collector; no invented collection radius |
| fungal-rootwarden | fan5/95; split/mines104; actual mine endpoints | Library roots and fungal material |
| fungal-spore-crown | ring6/85,aimed2/60; mines/split/crossfire/surge78; exact endpoints/held lanes | Library brood apex crown |
| hellforge-ash-castellan | ring8/95; twin_spiral/sweep91; outgoing rays/beam edges | Library living iron and ash |
| hellforge-chain-keeper | aimed3/80; lead/crossfire91; held-shot marks | Library chain/forge iron |
| hellforge-free-captain | aimed2/60; surge/laser91; exact beam half-width | Library slag captain |
| hellforge-fuel-reeve | fan4/80; rain/reaim91; real held points | Library fuel officer and slag |
| hellforge-slag-sovereign | ring6/85,aimed2/60; twin_spiral/sweep/rain/laser72 | Library forge throne materials |
| archive-bellkeeper | ring8/100; gap_ring/weave98; outgoing rays | Library drowned bell/brass |
| shoggoth-heart | ring10/100,aimed2/80; gap_ring/sweep/surge/laser78 | Library threshold heart; explicit heart anchor |
| grinwick | lead/bounce36 plus signature inhale/tell/dash/recovery; use signature tick constants | Library train/whistle/ore; no expanded track footprint |
| the-big-ripper | flower/weave38 plus signature inhale/tell/dash/daze; use signature tick constants | Library comic fungal anatomy; no new gas radius |
| salvage-claimant | fan4/80; bounce/split104; existing rays only | Library drowned stack-chief/brass |
| unknown-memory-warden | spiral3/85; laser/gap_ring91; exact beam/rays | Library first-seal memory materials |
| unknown-silence-warden | fan7/100; sweep/seeker/curve91; exact beam/rays | Library second-seal silence materials |

At 30 Hz, 4 ticks =0.133s, 6 ticks =0.200s, 24 ticks =0.800s, 36 ticks =1.200s, 51 ticks =1.700s. The ordinary attacks above already have their own cadences; these requests grant no new charge-up, projectile density, hitbox or damage. Side-on versions remain a separate camera-registration delivery with measured anchors before use in a hall.

## Remaining library delivery integration (BOSS-ROLL-2)

The nineteen rows above now have registered native effect banks. Fifty-five of
the 57 sheets are usable; the two tell replacements below remain requests.
Flight is centre-registered, two frames ×4 ticks, with exposure 1.10 and cached
source-colour core/rim inks in both kits. The imported warning pigments use
existing alpha masks; authored alternatives, trails and extra impacts remain
unwired. Grinwick's accepted wave17 impact is the retained source, not its
rejected earlier delivery. Rules, radii, ray/lane clipping and timing stay exact.

- `bossfx.hellforge-free-captain.tell.alpha-2` and
  `bossfx.hellforge-slag-sovereign.tell.alpha-2`: Delve overhead, compact
  centre warning; four urgency stages across the existing 36-tick rage ring,
  24-tick held points and 21-tick beam warning. Preserve existing outgoing rays,
  safe gaps, 11×11 point clip and actual beam width. Native master 24×24;
  simulation-owned point collision is unchanged. Match the delivered captain's
  slag/ember and sovereign's molten iron identity respectively. Both delivered
  sheets have 100% opaque alpha, one border-touching merged component and a
  painted checkerboard. Deliver genuine transparent RGBA with four independently
  measured source crops and SHA-256, stable centre sockets and no backdrop;
  keep hot values saturated below near-white. The shared old warning masks
  remain byte-exact until a valid source arrives; projectile/impact art is active.
- `bossbody.grinwick.floor-2`: Delve overhead staged engine body; v1 57×54,
  v2 102×96; existing bottom-centre feet at centre+simulation radius, preserving
  track/charge footprint, porcelain faceplate, boiler, jointed legs and funnel.
  On the actual graded cavern floor including its existing contact glow, v1
  rest presence is 63.82% (1000/1567 pixels), short 16.18 percentage points and 254
  opaque samples of the 80% floor+15 L* requirement. Both median/edge tests and
  the other kit/path/pose checks pass. Raise interior/leg material separation
  while preserving alpha, contours, hues and the unchanged 75 inks; do not
  enlarge or alter the contact glow. Keep existing signature stage ticks and
  the two source keyposes; supply transparent source+measured JSON/SHA with
  registered feet. The old engine body and staged tells remain active.
- `bossbody.hellforge-slag-sovereign.palette-2`: Delve overhead guardian body,
  rest and anticipation keyposes; v1 45×54, v2 80×96; bottom-centre feet,
  unchanged `.boss` radius and footprint. Preserve the delivered five-prong crown,
  molten belly, gauntlets and mantle silhouette. Anticipation colour agreement
  under the unchanged 75-ink bank is 0.8921 (v1) and 0.8814 (v2), below 0.90 by
  0.0079/0.0186. Supply source orange/fire ramps that fit the existing palette
  within RGB distance 60 on at least 90% of visible samples, with edge agreement
  ≥85% and alpha IoU≥98%; do not paint extra silhouette or lower thresholds.
  Rest and windup must also retain local median floor contrast ≥15 L*, rest
  presence ≥80% and edge coverage ≥50% at 10 L*. Deliver transparent lossless RGBA,
  measured crop JSON/SHA and the same feet; timings remain existing 18-tick breath,
  36-tick rage and24-tick arms warning. The old body remains active pending this
  correction. See retained `library-import.json` and BOSS-ROLL-2 candidate metrics.

## Bone Choir body value correction (VALUE-1)

- Asset id: `bossbody.the-bone-choir.value-1`; Delve guardian body, oblique overhead arena camera. Revise the existing library design; retain its three skulls, ribcage, red conductor robe and dark contours.
- Native masters: v1 49×54, v2 87×96; bottom-centre foot anchor. Preserve opaque alpha, footprint and the authoritative `.boss` radius1.4. Existing source/crop registration and palette/fidelity gates remain binding.
- States: one rest and one anticipation master. Existing cached idle breath advances in18-tick steps; anticipation follows the existing36-tick rage ring and24-tick arms warning. Existing hurt/heat grades stay derived; no additional attack frames, release delay, hitbox or gameplay change.
- Measured failure: in its generated Crypt guardian Stairs room (depth2, seed17), four seats, rest native v1 has1107/1395 samples at local floor+15 L* (79.35%); v2 has3272/4397 (74.41%). The retained threshold is80%: deficits9/246 samples, or0.65/5.59 percentage points. Native medians already pass at27.70 L* above the floor; decoded cells pass99.50%/95.00%.
- Required value change: raise interior robe/bone-shadow values so at least9 additional v1 opaque samples and246 additional v2 samples reach **their own graded floor L*+15**, while retaining the outline, red hue family, source alpha and existing passing samples. These are measured native-sample requirements after import and the current grade, not permission to paint extra pixels. Retest the shared palette/fidelity and both poses. Keep the three native skulls distinct; their light cell mass currently merges, so separate readable value groups where the source geometry permits it.
- Delivery: transparent lossless RGBA source sheet plus measured crop JSON, SHA-256, foot/socket and opaque bounds, and labelled skull/robe/contour material regions; native v1/v2 validation renders. No baked floor, labels or bars. Source library remains read-only to integration.
- Evidence: `/work/artifacts/delve-int-20261015/value-1/review/the-bone-choir-rest-rooms.png`, `the-bone-choir-windup-rooms.png`, `the-bone-choir-identity.png`; complete measurements in `candidate-metrics.json` beside `review/`.

### Merged guardian-floor value correction (INT-2)

- Asset ids: `bossbody.{iron-tusk-chief,cinderjaw,mother-of-spores,the-index,waxen-warden,the-bone-choir,crypt-tithe-marshal}.int-2`; Delve guardian/chief bodies, existing oblique overhead arena camera. Revise the delivered library designs, preserving their identity props, hues and contours.
- Preserve each `scripts/boss-library-registry.json` v1/v2 native size, bottom-centre anchor, alpha/footprint and authoritative `.boss` radius. Rest and anticipation masters retain existing 18-tick breathing, 36-tick rage ring and 24-tick arms warning; no timing or gameplay change.
- The merged dungeon floor invalidates VALUE-1's black-floor measurements above. All seven candidates fail the unchanged local graded-floor rule. Native rest presence v1/v2: Tusk 46.17/50.09%, Cinder 47.56/51.66%, Mother 52.70/60.80%, Index 27.28/36.27%, Waxen 57.95/64.17%, Choir 26.74/34.00%, Marshal 34.09/43.14%. Full 56 measurements, per-kit pixel deficits and edge/median failures: `/work/artifacts/delve-int-20261015/int-2/candidate-metrics.json`.
- Required: source-derived value grouping that passes local signed median ΔL* ≥15 in both poses, ≥80% rest opaque samples at their own floor+15 L*, and ≥50% outer-two-logical-pixel samples at absolute floor contrast ≥10 L*, native RGB and actual cells in both kits. Preserve existing source fidelity/palette limits; do not paint extra alpha or alter the floor to pass. All seven earlier production bodies remain selected pending acceptance.
- Delivery: transparent lossless RGBA source sheets plus measured crop JSON, SHA-256, foot/socket and opaque bounds, labelled identity/material regions and native v1/v2 validation renders. No baked floor, labels or bars. Read-only source library is unchanged. Review sheets: `/work/artifacts/delve-int-20261015/int-2/contrast-review/`.

## Dungeon integration follow-ups, 15 October evening

| Asset | Role and geometry | States and timing | Delivery / remaining need |
|---|---|---|---|
| `dgn.floor.fungal-quiet` | Top-down Fungal background; 16x16 v1 / 32x32 v2, centre anchor (8,8)/(16,16), no collision or footprint changes. | Plain earth, worn earth, sparse roots: one held tile each, no animation. | Source PNG + SHA-bound measured crop JSON, binary alpha, existing dark palette. Broad low-contrast material areas that join across tile edges, without a root lattice or framed bevel on every tile. The current source still repeats visible roots after reduced variation, dressing and contrast. Design belongs to the art team. |
| `dgn.odoor.<pack>.side` | Orthogonal west/east vertical strips, 16x32 / 32x64; bottom-centre pivot (8,32)/(16,64). Existing two-cell door collision stays authoritative. | West barred/open and east barred/open, one held frame each. Opening timing awaits a rules-owned age. | Separate complete measured objects with matched pivots, no angled pedestal; open must show a clear passage and barred must show bars at native size. Mines is angled; Keep and Fungal silhouettes remain ambiguous at native size. All six packs retain dim pack-coloured placeholders for those rejected states. |

The delivered `dgn.hud.chrome` ready/cooldown/disabled/pressed, health, stamina,
boss, seat, floor, minimap, status and toast plates are registered. Pressed is
staged until an authoritative pressed-state signal exists; no new input clock
or animation age was invented. Existing labels and numbers draw over the skins.

`dgn.odoor.mines.south`: south barred/open top-down threshold, one held frame
each; 32x16 / 64x32, bottom-centre pivot (16,16)/(32,32), existing two-cell door
collision. Deliver two separated measured PNG crops with matched pivots; the
open state needs a clearly empty passage instead of the current solid timber
crosspiece. Timing and collision stay unchanged.

### BOSS-READ-1 value-request disposition

The VALUE-1 and INT-2 body-value requests above are superseded for the seven
registered production envelopes by the accepted boss-hall floor shade and
source-colour silhouette rim. All seven pass the unchanged contrast rule in
both kits, both poses and both output paths. No source redraw is required for
this integration. Source design, alpha, anchors, sizes and gameplay stay fixed.
See `docs/BOSS-LIBRARY-IMPORT.md` (BOSS-READ-1) and
`/work/artifacts/delve-int-20261015/boss-read-1/after-metrics.json` for evidence.
The terminal still merges Choir's skulls and Waxen's small candle tips; passing
floor contrast does not claim those fine identity details survive reduction.

### INT-4 ART-18 entrance and side-door disposition

Accepted entrance masters for Cinder, Latch, Mossbell, Brasswake, Rafterwake
and Pennon are integrated through SHA-bound supplemental `.entrance` slots,
with exact 384x224 / 768x448 registration and collision-aware masking. Extra
master props never acquire collision. Source-only acceptance remains distinct
from the native/cell review recorded in int-4/REPORT.md.
Crypt, Mines, Keep, Fungal and Unknown vertical side-door sheets replace their
placeholders at 16x32 / 32x64, west/east closed/open/locked. Existing rule
closure states select the drawings; worn proposals remain staged. Archive's
baked-checkerboard sheet remains rejected and its placeholder is retained.
No new art was designed; the ART-18 sources remain read-only.

## Torchlight Touge (DRIFT-1, 16 October 2026)

First-person chariot drift racing, all four seats at once. Implementation uses
plain cached race3d road/barrier meshes, existing cart tubs, box horses and
status plates. These are temporary placeholders; no new source art was drawn.
Slot contract: [`drift/slots.json`](../crates/delve/assets/drift/slots.json).
Original dark 1990s arcade style and knight anatomy from the read-only library
apply. Keep the road ahead clear; art never changes collisions or timing.

| Asset id | Role and camera | States, frames and ticks at 30 Hz | Native size, anchor, placement and geometry |
|---|---|---|---|
| `touge-cockpit` | First-person cockpit rim, gloves and reins; art team designs the vehicle | Idle 1; drive 4×4 ticks; drift left/right 4×4 each; spin recovery 6×4 (24 total) | 160×64, bottom centre (80,64) in 384×224; chariot half-width 0.65 world units; do not obscure road centre or HUD |
| `touge-horses-rear` | Pair of horses from behind, first-person and rival chariots; flat rear and rear three-quarter banks | Idle 1; gallop 6×3; slide left/right 4×4 each; gait is visual and follows speed | Pair 64×48, feet (32,48); centres 2.8 units ahead and ±0.55 across; height 1.55 units; no separate horse hitboxes |
| `touge-torch` | Torch and support, road sides and tunnel; screen-facing billboard or material masks | Burn 4×4 looping | 12×32, foot (6,32); road sides ±4.65, every16 along course; top 2.3 units; no collision |
| `touge-road` | Grimy mountain road, edge, tunnel road and lap stripe; perspective material tiles | Four static material states, 1 frame each | 32×32 seamless source tiles and material masks; 10-unit road; centre stripe must not suggest an obstacle; barriers authoritative |
| `touge-mountains` | Night mountain backdrop; first-person horizon behind near geometry | Night 1 frame | 384×112, horizon anchor (192,112); dark silhouette with cool layered forms; no collision |
| `touge-boost` | Blue, orange and purple wheel/boost signals; rear/three-quarter rivals and restrained cockpit-edge cue | Each stage 4×3 looping; charge at 18/42/72 eligible slide ticks (0.6/1.4/2.4s); release lasts 18/30/45 ticks (0.6/1/1.5s) | 32×24 each, centre (16,12); rear sockets ±0.55 across, z0.2; visual only, no impact or damage; stage colour is also stated numerically |

Deliver transparent RGBA sheets plus measured crop JSON, source SHA-256,
state/frame/facing, frame duration, common anchors, native previews, palette and
material masks, and separate reins/horse/socket/occlusion metadata. No near-white
cores, noise or dithering. Preserve the broad view needed to steer through the
hairpins. Track, spin-out, boost and scoring rules remain authoritative.
