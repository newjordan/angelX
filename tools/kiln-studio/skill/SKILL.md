---
name: kiln
description: Draw a piece of the Delve's pixel art (a figure, a prop, an effect) with kiln's MCP tools: write a source, render it, look at the shot, check it, revise, and stop when it reads. Used by kiln-studio's job agents and by anyone driving kiln from Claude Code or Codex.
---

# kiln: drawing for the Delve

You are one variant of a kiln job. Other agents draw the same brief beside you; a critic and then
people judge the results at true size. You draw by writing a **source file** and looking at what
kiln makes of it. The renderer holds the inks, the grain and the light; your job is the shapes,
the values and the read.

## Your folder

Everything you need is in your working folder, and you write only there:

| file | what it is |
|---|---|
| `brief.md` | what to make, as asked |
| `director.md` | the brief worked out: name, canvas, ramps, pose, must-haves, the source file to write |
| `BIBLE.md`, `bible.json` | the style bible: grain, light, line, ramps, proportions, what to avoid, the rubric |
| `lessons.md` | what reviewers said about earlier pieces (when there is any): follow it |
| `crib.md` | the source format's ops you need (pxl or the vfx recipe), with their arguments |
| `realm.hex` | the realm's 74 inks; a pxl source loads it with `use("realm.hex")` |
| `refs/` | concept images and library pieces to look at, when the job has them |

Never touch git, never commit, never write outside this folder. Don't install anything.

## The tools (MCP server `kiln`)

- `shot {source}`: renders and writes `shots/<name>.png`, the **grain ladder**: grain 1 at the game's
  own size, then grains 1, 2 and 4 side by side at 64 px a tile on the Crypt floor, a 4x zoom, and
  every frame. It comes back as an image; if it doesn't, open `shots/<name>.png` (Read it, or view it).
- `check {source}`: the lints. `ok: false` means errors (inks outside the realm, half-transparent
  pixels, nothing drawn): fix them. Warnings (lone pixels, outline, floor, grain, the silhouette
  falling apart at grain 1, too many inks): fix each, or know why it is meant.
- `describe {source}`: the facts (box, inks, families), how frames move, and the first frame as ink
  rows **at grain 1**: read them, that is the game's 16 px figure.
- `render`, `compare {ref | other}`, `export`, `library_search {query}`: as named.
- Every shot, check and describe of a changed source keeps a version in `versions/`: you can
  always go back to one.

## The loop

1. **Read** `brief.md`, `director.md`, `lessons.md` if present, and `crib.md`. Skim `BIBLE.md`
   (grain, light, line, the ramp table, figures, avoid). Look at `refs/` if there is anything.
2. **Plan** in a few lines before drawing: the canvas, the three to five big masses and where they
   sit, the ramp each one uses, where the light falls.
3. **Write the source** the director named. Build big to small: silhouette masses first, then the
   light and shadow on each mass, then the line, then details (eyes, rivets, cracks) last.
4. **shot**, and **look** at the picture. Say to yourself what you see at each grain: does grain 1
   read as the thing the brief asked for, facing right? Is the light from the upper left on every
   part? Are there lone pixels, banding, pillow shading, a mass that is mush?
5. **Fix everything that one look showed in one edit**, then shot again. Don't shoot after every
   small change: each look costs time.
6. **check** when it looks right; fix what it says. **describe** and read the grain-1 rows.
7. Animation (an idle, a swing): get the first frame right before drawing the others, then shot
   again and look at the frame rows: feet still, the motion small and even, no pixel jitter.

## Stop rules

- **A first good shot is a start, not the finish.** After the piece first reads, make at least three
  more look-and-fix passes, each one aimed at the worst thing the last look showed: the grain-1 read,
  then the light and the clusters, then the line and the lints.
- **Lints are fixed, not explained.** Lone pixels over the limit, an outline that isn't the outline,
  a silhouette in pieces at grain 1: fix them. Only an eye or a glint is a meant lone pixel.
- **Done** when `check` has no errors and no warnings you could fix, the grain-1 silhouette reads,
  it fits the brief and the director's must-haves, and your last look found nothing worth a pass.
- **At most 14 shots.** If it isn't working by shot 8, simplify (fewer parts, bigger shapes).
- **The clock**: every tool result says how much of your budget is used and left. Use it: stop when
  the piece is done or with two minutes left, whichever is first. The job stops you when it runs
  out, and what is on disk then is your piece.
- Always leave the source **working**: never end on an edit you haven't shot.

## Finish

Write `NOTES.md` in your folder: what you made, its canvas and frames, the ramps, what the last
check said, and the one thing you would change next. Then reply with two or three sentences
saying the same. No other files are needed: the job takes your source and its shots from here.

## Craft notes that matter most at 64 px a tile

- The silhouette carries the read at 16 px: give the head, the weapon and the stance a clear
  outer shape. Anything thinner than 4 px at grain 4 is gone at grain 1.
- Two to four tones per mass, from its ramp; the light on the upper-left planes, one step down on
  the lower right, the darkest step in the folds and under overhangs.
- No black inside a mass except real gaps (eye sockets, between ribs, a mouth). The `k` line is
  for the outside and where parts overlap.
- The outline with selout: in pxl, `outline: "k", selout: 2` with `ramps(...)` declared, so the line
  softens beside lit colours.
- Clusters, not pixels: each tone sits in a shape of several pixels. A lone pixel is noise unless it
  is an eye or a glint.

## Sources by backend

- **pxl** (`<name>.px.js`): JavaScript calls that paint. Start with `use("realm.hex")` and
  `ramps(...)`. **With `outline`, pxl grows the canvas 1 px on every side**: for a 64 x 64 result give
  the sprite `w: 62, h: 62`. An idle is an `anim()` whose `frame()`s patch a shared body. `crib.md`
  has the ops.
- **vfx** (`<name>.vfx.json`): the vfx cartridge's layered recipe, written by hand (its prompt mode
  is not used). Every layer's `ramp` is a realm ramp: `"$fire"`, `"$ice"`, ... from `bible.json`
  `vfx_ramps`, or `"ink:<keys>"` (realm keys, dark to light). The cartridge's own ramp names,
  `palette` and `hue` are refused. `crib.md` has the fields and layer types.
- **kiln** (`<name>.kiln.json`): kiln's own recipe, JSON, drawn by kiln at every grain in the realm's
  inks; the renderer holds the light, the line and the cleanup. A figure is `{"kiln": 0, ...}`: lit
  volumes (ellipsoids, capsules, boxes, slabs, strokes) in materials, hung on a rig's bones, posed by
  clips; the reference is docs/KILN-RECIPE.md, and `crib.md` holds it with the worked knight
  (crates/kiln/recipes/knight.json). An effect is `{"fx": 0, ...}`: layers of shapes and emitters
  timed in beats (anticipation, action, follow, settle), each in a named ramp; the reference is
  docs/KILN-FX.md, and `crib.md` holds it with examples. Model space is game pixels (16 a tile).
  Errors name their path (`layers[2].arc: a ring takes no arc`): read them and fix that field. The
  shot's grains 1 and 2 are kiln's own drawing, not a reduction: judge the grain-1 read from them,
  and fix it in the recipe (a part's `grains`, a decal for one grain), not by hand.
