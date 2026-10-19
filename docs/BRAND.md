# The brand guide: the look, in numbers

The operator, 2026-10-18:

> "lets establish a brand guide with rules on object brightness etc so we build a really solid game experience and not jsut more eye chaos ok? terrain can only be X bright, etc we need to literally put clamps on the art assets and have a skeleton that easily allows agentic rapid buildouts but integrates objects intot he world accrodingly and it will look good and fit in"

**The look.** Simple primitives with good lighting. The inspiration is *Superbrothers: Sword & Sworcery*, and the dungeons should feel simple and beautifully brutalist:
- few, great masses;
- deep quiet dark;
- one warm light;
- small figures in solemn spaces.

**The references:**
- the lantern corridor (`/work/artifacts/delve-handoff-20261018/look-reference.png`);
- the monolith's castle and river (`/work/artifacts/monolith/review/`, docs/REALM-MONOLITH.md).

Every number below was measured from that approved work.

**The one rule that makes rapid building safe: the builder says what a thing is, and the engine decides how it looks.**
- A set file names each surface's **kind** (stone, floor, door, fire …).
- The kind belongs to a **class**, and the class fixes its brightness and its colour limits.
- The place's ramp gives it its hue, and the look lights it.
- Nobody picks a colour or a brightness for a world surface, so anything anyone builds lands on the same ladder and fits.

The clamps live in code (`crates/stage/src/brand.rs`), and the shader holds them.

## 1. The value ladder

Every place's ramp has eight steps, dark to light, at these brightnesses. Brightness is luma: Rec. 709 weights on sRGB-encoded channels, 0 to 1. Only the hue is the place's own. A step may stray an eighth of its value (at least 0.012).

| step | dungeon | open | what lives there |
|---|---|---|---|
| 0 | 0.060 | 0.066 | the void: past the room, a doorway's depth, deep shadow |
| 1 | 0.090 | 0.112 | faces turned from the light, shadow on masses |
| 2 | 0.125 | 0.162 | openings at their brightest; ground in cast shadow |
| 3 | 0.170 | 0.218 | masses in shadow light; ground in shade |
| 4 | 0.215 | 0.287 | masses lit from above |
| 5 | 0.280 | 0.368 | **the most a mass or a prop is ever lit to** |
| 6 | 0.365 | 0.468 | **the most the ground or a road is ever lit to** |
| 7 | 0.500 | 0.604 | kept for attention: water's shimmer, what a knight can use |

- **Dungeon** is the ladder for dungeons, castles' masses, interiors and night. The castle's slate ramp sits on it exactly.
- **Open** is for country under the sky: a step lighter at the top, the same dark. The meadow and river ramps sit on it.
- There is **no pure black**: step 0 is 0.06.
- **Nothing in the environment** goes past step 7. Only lights and figures do, and a frame has little of either (§6).

## 2. Classes

| class | what | tone (before light) | ceiling step | set kinds |
|---|---|---|---|---|
| void | the dark itself | 0.00–0.06 | 0 | — |
| opening | doorways, slots, a gate's mouth: dark, found by the light set at them | 0.04–0.14 | 2 | door |
| ground | what is walked on: the brightest stone in a room | 0.12–0.40 | 6 | ground grass sand mud floor rug crop |
| route | the way on, a brighter ribbon | 0.36–0.50 | 6 | path road |
| water | runs in bands, glints | 0.40–0.55 | 7 | water |
| mass | architecture and land: walls, pillars, plinths, roofs, rock, woods | 0.10–0.34 | 5 | stone brick plaster roof thatch slate rock foliage trunk hedge |
| prop | dressing nobody uses | 0.12–0.34 | 5 | wood metal cloth bone |
| interactable | what a knight can use: chests, levers, pickups, the stairs down | 0.30–0.45 | 7 | (game objects) |
| light | lamps, fire, lit windows, glints: the only warm colour | 0.80–1.00 | off the ladder | window fire |
| figure | knights, monsters, folk: sprites in their own colours | — | off the ladder | (sprites) |
| effect | shots, sparks, spells: brief and small | — | off the ladder | (game) |

**Reading the table:**
- **Masses never outshine the ground.** The floor is where the play is, so it is the brightest stone: mass tone ≤ 0.34 < ground tone ≤ 0.40, and mass ceiling 5 < ground ceiling 6.
- **Props never outshine masses.**
- **The top step is earned.** Only water and interactables reach it, so the eye goes there.
- **Openings are dark.** A door is a deep slot with a lamp beside it, never a painted rectangle.

**How the clamps hold:**
- The tone is held when a set is built (`Class::clamp_tone`).
- The ceiling is held in the shader, however strong the sun or a lamp is (`Material::ceiling`, mesh.wgsl `lit_look`).
- A set's ground tones are held within ground-to-route, so a "100" written for a road is 0.50, never white.

## 3. Light

Light is the event, and **light = activity**: where a lamp burns, something is happening.

- **Sun or moon:**
  - one per look;
  - ambient 0.18–0.32;
  - strength 0.80–1.25;
  - **shadows on**: the long hard shadow is the look.
- **Lamps:**
  - each at most 1.0 strength (a flicker may lift it a tenth);
  - at most 8 in a look;
  - **at most 3 in a set** (a room).
  
  The dungeon reference has one.
- **Warm light** (the warm ramp, lamplight) warms a surface and lifts it **one step at most**.
- **Accents:**
  - at most 2;
  - each a light, at least 0.55 bright: a glint on water, a lit window.

## 4. Colour

- **One hue per place.** A ramp keeps one hue from its third step up. It may lean warmer toward its light, as the meadow's sage does, but no more than 90 degrees, and never through the rainbow.
- **Chroma ceilings:**
  - environment ramps: chroma ≤ 0.20;
  - the warm ramp: chroma ≤ 0.55.
- **Warm colour comes only from light.** No surface is painted warm.
- **Figures are the only saturated things that walk.** They keep their own colours under the world's light.
- **A knight is the brightest figure in the room:**
  - knights' mean brightness ≤ 0.58, with at most 70% of their pixels over 0.505;
  - any other figure (monsters, folk) ≤ 0.50, and at most 50% of its pixels.
  
  A monster never outshines a knight.

## 5. Shape: simple, beautiful brutalism

**Per set (a room, a place, a chunk):**
- **at most 60 shapes**;
- **at most 5 surface kinds** (lights don't count);
- **nothing finer than 4 stage pixels** (a quarter tile) but a light;
- **at most 3 lamps**.

The old blockouts ran to a median of 107 shapes, 8 kinds and 22 fine details. That is the eye chaos.

**The vocabulary:**
- thick slab walls, with stepped plinths and batters at their feet;
- square pillars in rhythm;
- deep recessed doorways and slots under heavy lintels and cornices;
- stairs and ramps cut as solid blocks;
- a shaft of light or one lamp breaking the dark;
- woods as massed spires, never tree by tree;
- rock as great slabs.

**Repetition and proportion do the work, ornament never.** Work on the 16 px tile: masses span whole tiles, heights step in 4 px.

**The silhouette test:** a room shrunk to a thumbnail still reads as its few great masses.

## 6. The frame

What a finished frame may be. All twelve approved castle stills and the corridor pass.

| measure | rule | approved work |
|---|---|---|
| 1st percentile | ≥ 0.05: deep dark, never crushed | 0.063–0.067 |
| median | 0.12–0.26 | 0.16–0.22 |
| 95th percentile | ≤ 0.42 | 0.26–0.38 |
| share brighter than 0.505 | ≤ 1% (lights and figures) | 0.04–0.66% |
| share with chroma over 0.30 | ≤ 1% | 0.04–0.65% |

## 7. Building with it (the skeleton for agents)

1. **Write a set file** (docs/WORLD-3D.md "Set files", with the monolith's `ground` and `lamp` statements). Name kinds only: never a colour, never a tone but a ground's. Build from the primitives (`crates/stage/src/primitives.rs`) in the vocabulary of §5.
2. **Place lamps** only where something happens: a door, a stair, a fire, a shrine. One is often enough.
3. **Audit the set:**
   `cargo run -q -p angel-glass --example brand_audit -- --set path/to/room.set`
   Fix every breach. The rules are in §2–§5.
4. **Render it** in the lit look (`lit_world_shot`, `monolith_shots`, or the game), then **audit the frames:**
   `cargo run -q -p angel-glass --example brand_audit -- shot-1.png shot-2.png`
5. **Audit new sprites:** `--knight` for knights, `--sprite` for every other figure.
6. **Look at it yourself**, beside the references, then show the operator a contact sheet. The audit catches the numbers; only eyes catch the rest.

**A new kind of thing** (a new set kind, a game object) gets a class first, and the class gives it everything else. A new class, or a changed number, is the operator's call: change it here and in `brand.rs` together, and re-run the audit over the approved work.

## 8. Where the clamps live

- `crates/stage/src/brand.rs`:
  - the classes, bands, ladders and budgets;
  - `class_of(Kind)`;
  - the audits: `audit_ramp`, `audit_warm`, `audit_look`, `audit_set`, `audit_frame`, `audit_sprite`, with tests that the approved ramps pass and garish ones fail.
- `crates/stage/src/set.rs`:
  - every kind's tone sits in its class band (a test holds the table to it);
  - lit set surfaces carry their class's tone and ceiling;
  - a ground's tones are held;
  - lamps are held to strength 1.0.
- `crates/stage/src/scene.rs`: `Material::ceiling`.
- `crates/gpu/src/scene.rs` and `mesh.wgsl`:
  - the ceiling rides in the draw's `flags.z`;
  - `lit_look` caps the brightness at that step of the ramp where the pixel is, zones included.
  - `crates/glass/tests/look_lit_gpu.rs` proves it on the GPU: a white floor in full sun stops at the ground's step.
- `crates/glass/examples/brand_audit.rs`: the audit over PNGs, sprites, set files and the monolith's look.

## 9. What does not pass yet

This is the catalogue to redo:
- **The world3d blockout sets** (`crates/delve/assets/world3d/**/*.set`, 294 files): too many shapes, kinds and fine details.
- **The painted plates** (projection painting): crushed blacks, and too bright or too colourful.
- **Sprites brighter than any knight:** ghost (0.65), skeleton (0.64), gas core (0.60), slime (0.56), grey horse (0.58).
- **The old colourful floor-pickup art** (the item plates and models).
