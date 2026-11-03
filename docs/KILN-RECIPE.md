# Kiln recipe v0

A recipe is a figure as JSON: lit volumes and flat marks in materials, on a rig, with clips. One recipe
draws at every grain: 1 (the game's 16 px a tile), 2, 4 (HD, 64 px a tile) and 8. This is the reference
to write one from; docs/KILN.md is the why, docs/KILN-FX.md the effects. The knight
(`crates/kiln/recipes/knight.json`) is a full worked recipe.

## Rules of the file

- **Units are game pixels**, as f32. x right, y down, z toward the viewer; the origin is the canvas's
  top left. At grain g, pixel (px, py) samples the model point ((px + 0.5) / g, (py + 0.5) / g).
- **Unknown fields are refused.** Every fault comes back with its path, all at once:
  `parts[3].r[1]: 0 is not above 0 and at most 64`, `parts[5].points: an ellipsoid takes no points`.
- Names (`name`, part and bone ids, materials, sockets, clips) are 1 to 32 of `a-z 0-9 _ -`.
- Inks are the realm's 74 keys (`crates/kiln/assets/realm.hex`); `.` is clear.

## The whole

```json
{ "kiln": 0, "name": "pip", "canvas": [12, 12], "pivot": [6, 11.5],
  "materials": { "steel": { "ramp": "sxSuUvVW", "finish": "metal" } },
  "parts": [ { "id": "ball", "shape": "ellipsoid", "at": [6, 7, 0], "r": [4, 4, 3.5], "mat": "steel" } ] }
```

| field | needed | what |
|---|---|---|
| `kiln` | yes | 0 |
| `name` | yes | the recipe's name |
| `canvas` | yes | `[w, h]` in game px, each 1..=128 |
| `pivot` | yes | `[x, y]`: the floor contact under the feet. The row it is on is the floor (`check` holds the feet to it) |
| `light` | | toward the one light, `[x, y, z]`; default `[-0.55, -0.9, 0.7]` (upper left, a little toward you) |
| `outline` | | `{ "ink": "k", "selout": 2, "inner": 0.75 }`, see Lines |
| `lift` | | 0..1, default 0.35: at grain 1 lit tones are lifted toward the light end of their ramp |
| `materials` | yes | name to material |
| `parts` | yes | 1..=256 parts, drawn in order (a later part wins a depth tie) |
| `decals` | | grain (`"1"`, `"2"`, `"4"`, `"8"`) to marks drawn for that grain only |
| `rig` | | bones and sockets |
| `clips` | | name to clip |
| `notes` | | words for people |

## Materials

`{ "ramp": "sxSuUvVW", "finish": "metal", "shade": 0 }`: a ramp of ink keys dark to light (1..=16), a
finish, and a shade (-1..1) moving every tone along the ramp. The realm's ramps: steel `sxSuUvVW`, red
`n87`, gold `a456`, leather `nbIBpr`, gem `q1z3`.

| finish | how light falls |
|---|---|
| `metal` | hard bands, a dark core where it turns away, the lightest ink only in the glint, a step of the floor's light on the underside |
| `cloth` | three tones: lit, mid, shadow |
| `skin` | four soft tones, a lifted shadow |
| `matte` | the whole ramp by how it faces the light |
| `glow` | unshaded, brightest facing you |

## Parts

Every part has an `id`, a `shape` and a `mat` (a flat may take an `ink` instead). These fields go on any part:

| field | what |
|---|---|
| `bone` | the bone it hangs on (none: the canvas) |
| `socket` | the socket it is held in; its points are then about the socket, pointing up (-y) |
| `on` | a part before it: a **surface mark**, drawn only where that part shows, at its depth (no inner line) |
| `shade` | -1..1, its tones along the ramp |
| `bias` | added to its depth, to settle ties |
| `keep` | kept at grain 1 when thin (see Grains) |
| `grains` | the grains it is drawn at, e.g. `[2, 4, 8]`; all when absent |
| `hidden` | drawn only in a clip that `show`s it |
| `line` | `false`: it draws no inner lines where it covers another part |
| `cut` | `{ "x": [lo, hi], "y": [lo, hi] }` in its own space: outside is not drawn |
| `turn` | degrees about z, clockwise on screen (ellipsoid, box) |

Each shape takes its own fields and refuses the others.

**ellipsoid**: `at` its centre, `r` `[rx, ry, rz]`, `turn`. A helm, a pauldron, a knee, a fist.

```json
{ "id": "helm", "shape": "ellipsoid", "mat": "steel", "at": [14, 8.7, 0], "r": [3.25, 3.15, 2.7], "cut": { "y": [0, 10.95] } }
```

**capsule**: a sphere swept from `from` to `to` (3D points), `r` `[r]` or `[r_from, r_to]` tapering. An arm,
a leg, a horn. Tilt it in z and its near end stands proud.

```json
{ "id": "arm", "shape": "capsule", "mat": "steel", "from": [17.25, 13, 1], "to": [17.75, 15.5, 1], "r": [0.75, 0.65] }
```

**box**: `at` its centre, `size` `[w, h, d]`, `round` the edge radius (at most half the smallest side),
`turn`. A boot with a flat sole, a buckle, a crate.

```json
{ "id": "boot", "shape": "box", "mat": "leather", "at": [15.75, 18.95, 0], "size": [3, 1.1, 1.6], "round": 0.5 }
```

**slab**: a polygon `points` `[[x, y], ...]` (3..=64) with a face at depth `z` (default 0), `thick`
(default 0.5) and a quarter-round `bevel` (default 0.3) at its edge; `lean` `[x, y]` tips its face,
`curve` `[x, y]` turns the face across its width and height (a tabard over a chest). Cloth panels, blades,
plates.

```json
{ "id": "tabard", "shape": "slab", "mat": "red", "z": 1.9, "thick": 0.3, "bevel": 0.2, "curve": [0.5, 0],
  "points": [[12.25, 12.25], [16, 12.25], [16.25, 16.5], [14.5, 16.75], [12.5, 16.5]] }
```

**stroke**: a Catmull-Rom tube through `points` `[[x, y, z], ...]` (2..=64), `width` `[w]` or
`[w_first, w_last]` tapering along its length. Plumes, tails, hair, ropes.

```json
{ "id": "plume", "shape": "stroke", "mat": "red", "keep": true, "width": [1.7, 0.8],
  "points": [[14.5, 5.6, 0.4], [13, 4.7, 0.1], [11.4, 4.8, -0.4], [10.1, 5.7, -0.9], [9, 8.2, -1.4]] }
```

**flat**: an unlit polygon `points` at depth `z`, in one `ink`, or in a `mat` at `tone` (0 darkest, 1
lightest). Visor slits, stripes, folds; mostly as surface marks.

```json
{ "id": "visor", "shape": "flat", "ink": "k", "on": "helm", "grains": [2, 4, 8],
  "points": [[12.5, 8], [17.5, 8], [17.5, 8.5], [12.5, 8.5]] }
```

## Decals

What a volume can't say (an eye glint, an emblem, rivets) is drawn for one grain, in that grain's pixels:

```json
{ "4": [ { "at": [54, 51], "bone": "torso", "on": "tabard", "rows": ["..5..", ".555.", "55655", ".555.", ".5.5."] } ] }
```

`at` is the top-left in the grain's pixels at rest; `bone` moves it with the bone (never turns it); `on`
draws it only where that part's fill shows. Decals go on after the line work.

## Grains: how each is drawn

- **Grains 4 and 8** sample each pixel's centre.
- **Grains 1 and 2** are drawn at grain 4 and voted down: a pixel takes what most of its samples saw (the
  middle four count twice). A `keep` part wins its pixel with a quarter of the vote. A kept tube (capsule
  or stroke) thinner than 1.5 px at that grain is drawn instead as a one-pixel line along its axis.
  On the floor row a fifth of the vote inks a pixel, so a foot within a quarter pixel of the floor stays
  on it.
- **At grain 1** lit tones are lifted by `lift`, one-pixel gaps between parts close as seams, and the line
  is the figure's own edge (see Lines).
- A pose's shifts (`dx`, `dy`) land on whole pixels of the grain drawn, so a 16 px figure bobs by a pixel
  or not at all. Turns are not snapped.
- When a part reads differently at 16 px and 64 px, give each its own part with `grains` (the knight's
  visor is `k` at `[2, 4, 8]` and a shorter `s` at `[1]`) or a decal.

## Lines

- **Outline**, grain 2 and up: one pixel outside the figure. Facing the light it is selout, its
  neighbour's ink `selout` steps down that neighbour's ramp (the outline ink below the ramp's darkest);
  elsewhere the outline ink.
- **Grain 1**: the figure's own edge, away from the light only, in its ramp's second-darkest tone; the lit
  edge keeps its shading.
- **Inner lines**: where a part covers another across a depth break over `outline.inner` game px (twice
  that at grain 1), the covered pixel along the seam takes the nearer part's darkest tone.
- **Tidying**: specks inside a surface take their neighbours' ink (grain 2 and up); a line pixel that only
  doubles an L-corner of a line drawn on the figure is given back its fill.

## Rig

```json
{ "bones": [ { "id": "root", "at": [14, 19.5] },
             { "id": "torso", "parent": "root", "at": [14, 15] },
             { "id": "arm_near", "parent": "torso", "at": [17.2, 12.6] },
             { "id": "hand_near", "parent": "arm_near", "at": [17.7, 15.6] } ],
  "sockets": { "grip": { "bone": "hand_near", "at": [17.6, 16.3], "turn": 180 },
               "tip": { "bone": "hand_near", "at": [17.6, 18.3] } } }
```

- A bone's `at` is its joint in model space at rest; parents come before children. A part on a bone moves
  with it and is lit and drawn afresh in every pose: a turned arm is never a turned bitmap.
- Sockets are named points on bones. The game's quick-rig reads six every frame: `grip`, `off_hand`,
  `head` (where a hat sits), `back`, `feet`, `tip` (the held weapon's direction from `grip`). A socket's
  `turn` is which way what it holds points, in degrees from up, clockwise.
- A part with `socket` is written about the socket, pointing up: a sword's blade runs from `[0, -0.7]` to
  `[0, -6.3]`. Socketed parts are gear: the quick-rig export leaves them out, as the game fits its own.

## Clips

```json
{ "walk": { "frames": 8, "fps": 10, "loop": true, "keys": [
    { "t": 0, "ease": "both", "pose": { "leg_near": { "turn": -16 }, "foot_near": { "turn": 16 } } },
    { "t": 2, "ease": "both", "pose": { "torso": { "dy": -0.3 }, "leg_far": { "dy": -0.45 } } },
    { "t": 4, "ease": "both", "pose": { "leg_near": { "turn": 15 }, "foot_near": { "turn": -15 } } } ] } }
```

- `frames` 1..=128, `fps` 1..=60, `loop` (default true): a looping clip's last key runs back into its first.
- A key's `t` is its frame (fractions allowed, ascending); its `pose` gives each bone's `turn` (degrees,
  clockwise), `dx`, `dy`, `dz` from rest. A bone not named is at rest in that key.
- `ease` is how motion runs from this key to the next: `linear`, `in`, `out`, `both`, `back` (overshoot),
  `spring`.
- `show` and `hide` list parts; hidden parts (a sword) appear only in clips that show them.
- Swing a leg and turn its foot back by as much, and the foot stays flat on the floor.

## The kiln command

```text
kiln render   <recipe> [--grain 1,2,4] [--clip NAME|all] [--frames all|0,3] [--scale N] [--floor PNG] [--out DIR]
                       [--fx FX [--fx-at SOCKET|X,Y] [--fx-from N]]
kiln shot     <recipe> [--clip NAME [--frame N | --strip [--onion]]] [--ref PNG[@G]] [--floor PNG] [--zoom N] [--sockets]
kiln check    <recipe> [--json]
kiln describe <recipe>
kiln compare  <recipe> (--ref PNG[@G] | --against RECIPE) [--grain G] [--clip NAME --frame N] [--out FILE]
kiln export   <recipe> (--ink-rows [--grain G] | --hd-pack DIR | --quick-rig [FILE])
kiln watch    <recipe> [--out DIR] [--ref PNG[@G]]
```

`cargo build --manifest-path crates/Cargo.toml -p angel-kiln --features cli` builds it. An fx recipe (root
`"fx": 0`) given to `render` or `shot` draws as an effect.

## The loop

Write, `kiln shot` and look at the sheet (the grain ladder at true screen size, grains 1 and 4 zoomed, a
reference beside them), `kiln check` (each lint says its grain, clip, frame and pixels), `kiln describe`
when a part seems lost, mend, again. The lints:

| lint | |
|---|---|
| `palette` | an ink not the realm's (error) |
| `lone` | specks, lone pixels inside a surface (warning) |
| `jaggy` | one-pixel nubs on the silhouette, doubled corners (warning) |
| `silhouette` | grain 1 too small, or straying from grain 4's; a kept part lost (error), a big part lost (warning) |
| `floor` | the feet off the floor row (error) |
| `sockets` | a socket off the canvas (error), a quick-rig socket missing (warning) |
| `jitter` | a part that moved less than a quarter pixel yet changed pixels: shimmer (warning) |
