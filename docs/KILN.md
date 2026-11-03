# Kiln: the Delve's own pixel studio

2026-10-28, from the owner: "lets build a real, robust pixel dev tool ... models are only getting
smarter and faster", "im totally fine with this tool using my models to drive the actual tool, it
doesnt have to be an offline system... it should be agentic to the max". The focus: rapid VFX and
animation, and pixel character design, at 64 px a tile (four times the game's grain) and up.

Kiln is three things, kept apart so each can grow on its own:

| Layer | What it is | Who does it |
|---|---|---|
| **Hands** | `crates/kiln`: a deterministic renderer. A recipe in, the realm's pixels out, at any grain. | code, no model |
| **Eyes** | Kiln's look tools: shots, text descriptions, lints, comparisons, scores. Everything a model needs to see what it made. | code, plus a vision critic |
| **Brain** | `tools/kiln-studio`: jobs run by the owner's models (Claude Code, Codex, the treebeard router, ComfyUI on the Spark), each driving the hands through an MCP server and the CLI. | models |
| **Taste** | A review page where the owner and the group vote and leave notes. Approved pieces join a library and the style bible, and teach the next job. | people |

The models get better every month. The hands and eyes are what make a better model's work land in
the game exactly, so that is where the engineering goes. The brain is kept thin.

## Principles

1. **The recipe is the asset.** One source renders at every grain: 1 (the game's 16 px a tile:
   terminal, wire, goldens) and 2, 4 (HD packs, 64 px a tile) and 8. The look a figure shows at 16 px
   and at 64 px come from the same recipe, so they never drift. A drawing is named by its recipe, its
   pose, its frame and its seat, never by a hash of its pixels.
2. **Hard constraints do the drawing.** The realm's inks only (`assets/realm.hex`, the game's own 74).
   One light. One outline rule. Proportions and grain held by the renderer, not asked of the model.
3. **Construct, don't type.** Figures are lit volumes (ellipsoids, capsules, boxes, slabs, strokes)
   and flat shapes, each in a material (an ink ramp and a finish), sorted by depth per pixel. What a
   volume can't say (an eye, an emblem, a rivet) is a decal drawn for one grain.
4. **Motion from rigs and emitters.** A rig is bones; parts hang on bones; a pose turns bones; a clip
   keys poses in time with easing. Parts are volumes, so a turned arm is re-lit and re-rasterized,
   never a rotated bitmap. Effects are emitters, smears, rings, bolts, flames and smoke, driven by
   eased curves.
5. **Deterministic and fast.** The same recipe gives the same pixels on every machine. A 64 px frame
   renders in milliseconds, so a model can look dozens of times a minute and the game can render
   recipes live (wishes) on the host and in the browser (wasm).
6. **Built for models to drive.** Recipes are JSON with a versioned schema, unknown fields refused,
   errors that name the path. Every look tool has a text form as well as a picture.

## The hands: `crates/kiln`

- `palette` (shared): `Palette::realm()`, inks by key, `Ramp` (dark to light: `at(v)`, `step(ink, n)`).
- `grain` (shared): `Grain(n)`, pixels per game pixel. `Grain::GAME` is 1, `Grain::HD` is 4.
- `raster` (shared): `Raster`, an indexed buffer with depth and part ids per pixel. Ink 0 is clear.
  `to_rgba`, `ink_rows` (the game's text rows, `.` clear).
- **Model space** is in game pixels (f32), x right, y down, z toward the viewer, origin the canvas's
  top-left. At grain g, pixel (px, py) samples model point ((px + 0.5) / g, (py + 0.5) / g).
- `recipe` (lane kiln-core): the schema below, `Recipe::parse`, checked bounds.
- `shape`, `light`, `line` (kiln-core): volumes and flat shapes into the raster with depth; the light
  model and finishes (metal, cloth, skin, matte, glow); the silhouette outline with selout, inner
  lines where depth or part breaks, cleanup (lone pixels, L-corners, jaggies) per grain.
- `rig` (kiln-core, second stage): bones, parts on bones, poses, clips, easing, sockets (the
  quick-rig contract: grip, off_hand, head, back, feet, tip).
- `fx` (lane kiln-fx): emitters, particles with life ramps, smears and arcs with taper, rings and
  shockwaves, bolts, flames, smoke, glyphs; eased timing (anticipation, overshoot, settle). Bridges the
  cards' `vfx` recipe (`crates/delve/src/rules/vfx.rs`) so every wish's effect renders at 64 px.
- `look` (kiln-core): `shot` (a grain ladder 1/2/4 at true screen size on a room floor, a zoom,
  frame strips, onion skin), `describe` (text: parts, boxes, inks used, silhouette, motion), `check`
  (lints: palette, lone pixels, jaggies, silhouette legible at grain 1, floor contact, socket bounds,
  frame-to-frame jitter), `compare` (against a reference PNG or another version).
- `export`: ink rows at grain 1; an HD pack entry at grain 4 (`hd_pack.rs`'s `pack.json`), named; a
  quick-rig JSON (`assets/quick-rig/knight.json`'s contract).
- `bin/kiln` (feature `cli`): `kiln render|shot|check|describe|compare|export|watch`.

### Recipe v0 (kiln-core owns the final form)

```json
{ "kiln": 0, "name": "knight", "canvas": [24, 24], "pivot": [12, 23],
  "light": [-0.55, -0.9, 0.7], "outline": { "ink": "k", "selout": 2 },
  "materials": { "steel": { "ramp": "sxSuUvVW", "finish": "metal" },
                 "red": { "ramp": "n87", "finish": "cloth" } },
  "parts": [
    { "id": "helm", "shape": "ellipsoid", "at": [12, 6, 0], "r": [4, 3.6, 3.6], "mat": "steel", "bone": "head" },
    { "id": "plume", "shape": "stroke", "points": [[13, 2, -1], [10, 1, -1], [7, 3, -1]], "width": [1.4, 0.6], "mat": "red", "bone": "head" } ],
  "decals": { "4": [ { "at": [26, 15], "rows": ["kkk", ".3."] } ] },
  "rig": { "bones": [ { "id": "root", "at": [12, 16] }, { "id": "head", "parent": "root", "at": [12, 10] } ] },
  "clips": { "idle": { "fps": 8, "loop": true, "keys": [ { "t": 0, "pose": {} }, { "t": 4, "pose": { "head": { "dy": 0.25 } } } ] } } }
```

## The brain: `tools/kiln-studio`

- **Model adapters**, all the owner's: `claude -p` (Claude Code, headless, its tools limited to
  kiln's), `codex exec` (gpt-6.1-sol), the treebeard router at `127.0.0.1:8011` (OpenAI-compatible,
  quick text), ComfyUI on the Spark (`ssh dgx-spark`, tunnelled to `127.0.0.1:18188`; see
  `~/.angelX/wish-paint/painter.py` for how it's started and called), the local RTX 3090.
- **The kiln MCP server**: the hands and eyes as tools (`render`, `shot`, `check`, `describe`,
  `compare`, `export`, `library_search`, `critique`, `concept`), so any agent (Claude Code, Codex)
  drives kiln directly. A skill document teaches them the recipe and the style bible.
- **Jobs**: a brief in (from a person, or from the game: a wish) and an asset out. A job is
  directed (the brief expanded against the style bible and the library), conceived (concept images
  when they help), built (N variants in parallel by agent workers), criticised (a vision pass against
  the brief and a rubric), then put up for review. Budgets per job; everything logged under
  `~/.angelX/kiln/jobs/<id>/`.
- **Backends**: kiln, and pxl (a local pixel-art tool) for sketches until kiln covers a need.
- **Review**: a page with every job's variants at true size and zoomed, votes and notes. Approved
  pieces go to the library (recipes, renders, notes) and lessons into the style bible.

## Rules for every lane

Own worktree and branch off `rnd/kiln`; builds through `systemd-run --user` with `MemoryMax` and the
lane's own `CARGO_TARGET_DIR` under `<build-cache>/`; commits as Jordan Newman
(the repo's configured author), with no AI attribution of any kind anywhere; no pushes; no rented servers;
never touch `~/.codex/worktrees`; every picture looked at before it is called done.
