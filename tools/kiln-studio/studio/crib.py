"""crib.md: the ops of a source format an agent needs, put in its folder.

A Claude job agent has no shell, so it can't run `pxl help`; the crib is
made from pxl's own help at job time (so it follows pxl as it changes), and
from the vfx cartridge's README for vfx recipes.
"""
import json
import re
from pathlib import Path

from . import paths
from .backends.base import run

PXL_OPS = ["use", "pal", "ramps", "outline", "selout", "pivot", "mx", "rect", "box", "line", "curve", "circ", "ellipse",
           "arc", "capsule", "poly", "px", "grid", "fill", "shade", "light", "stamp", "flip", "anim", "frame",
           "sub", "layer", "clip"]

PXL_HEAD = """# crib: pxl for a 64 px figure

The calls you need, from pxl's reference (`pxl help <op>`). Coordinates are pixels from the top left.
Colours are realm ink keys (one character); `.` is skip, `_` erases.

A figure, the shape of the file:

```js
use("realm.hex")                       // the realm's 74 inks by their keys
ramps("SviH", "sxSuUvVW", "nbIBpr")    // dark -> light: drives shade()/light() and selout

sprite("_skull", { w: 20, h: 18 },     // a part: "_" names are not exported
  ellipse(10, 9, 9, 8, "v"), ellipse(8, 7, 6, 5, "i"), px(6, 5, "H"))

sprite("hero", { w: 62, h: 62, outline: "k", selout: 2, pivot: "bottom" },   // 62 + 1 px outline a side = 64
  capsule(30, 40, 4, 30, 58, 3, "SviH"),   // a limb, lit from the upper left along its ramp
  stamp("_skull", 20, 4))

anim("hero_idle", { w: 62, h: 62, fps: 6, loop: true, outline: "k", selout: 2, pivot: "bottom" },
  stamp("hero", -1, -1),                     // shared by every frame (hero is 64 with its line: -1 lands it)
  frame(), frame(rect(20, 20, 2, 2, "H")))   // each frame() patches the shared items
```

Put the parts in drawing order: what is behind first. Check each op's arguments below.

"""


def pxl_crib():
    out = [PXL_HEAD]
    for op in PXL_OPS:
        code, text, err = run([paths.NODE, paths.PXL, "help", op], env=paths.node_env(), timeout=30)
        if code or not text.strip():
            continue
        # The reference's own lines, not the list of other places it is mentioned.
        body = text.split("[reference.md: also mentioned in]")[0].split("[examples:")[0].strip()
        body = re.sub(r"^\[reference\.md: [^\]]*\]\n?", "", body, flags=re.M).strip()
        if body:
            out.append(f"## {op}\n\n{body[:2500]}\n")
    return "\n".join(out)


def vfx_crib():
    readme = Path(paths.VFX).parent / "README.md"
    text = readme.read_text() if readme.exists() else ""
    m = re.search(r"## Recipes\n(.*?)(?=\n## |\Z)", text, re.S)
    recipes = m.group(1).strip() if m else "(the cartridge's README was not found)"
    try:
        ramps = json.loads(paths.BIBLE_JSON.read_text()).get("vfx_ramps", {})
    except (OSError, ValueError):
        ramps = {}
    example = Path(paths.VFX).parent / "examples" / "frost-nova.recipe.json"
    ex = json.loads(example.read_text()) if example.exists() else {}
    for layer in ex.get("layers", []):
        layer["ramp"] = "$ice"
    ramp_rows = "\n".join(f"| `${k}` | `{v}` |" for k, v in ramps.items())
    return f"""# crib: the vfx cartridge's recipe, in the realm's inks

Write `<name>.vfx.json` by hand; the `shot` tool renders it. The fields and layers are the
cartridge's own (from its README, below). The studio holds its colours to the realm:

- every layer names a `ramp`: a bible ramp (`"$fire"`) or realm keys (`"ink:0123w"`, dark to light);
- the cartridge's own ramp names (`fire`, `ice`, ...), `palette` and `hue` are refused.

| ramp | realm keys |
|---|---|
{ramp_rows}

An example (the cartridge's frost nova, in `$ice`):

```json
{json.dumps(ex, indent=1)}
```

## The cartridge's recipe fields

{recipes}
"""


def kiln_crib(kind="figure"):
    """kiln's own reference: docs/KILN-FX.md and two of its example recipes for an
    effect, docs/KILN-RECIPE.md (when the core lane has written it) for a figure."""
    if kind == "vfx":
        doc = paths.REPO / "docs" / "KILN-FX.md"
        text = doc.read_text() if doc.exists() else "(docs/KILN-FX.md is not in this checkout)"
        examples = []
        for name in ("frost-nova", "fireball", "hit-2-medium"):
            p = paths.REPO / "crates" / "kiln" / "examples" / "fx" / f"{name}.json"
            if p.exists():
                examples.append(f"### {name}\n\n```json\n{p.read_text().strip()}\n```")
        return ("# crib: a kiln effect recipe\n\nWrite `<name>.kiln.json` with `\"fx\": 0`. kiln draws it at every grain in the "
                "realm's inks; the shot shows grains 1, 2 and 4 as kiln drew them, the busiest frame on top.\n\n"
                + text + "\n\n## Examples from crates/kiln/examples/fx\n\n" + "\n\n".join(examples) + "\n")
    doc = paths.REPO / "docs" / "KILN-RECIPE.md"
    knight = paths.REPO / "crates" / "kiln" / "recipes" / "knight.json"
    if not doc.exists():
        return "# crib: kiln\n\nThe figure recipe's doc (docs/KILN-RECIPE.md) is not in this checkout; KILN.md's Recipe v0 is the shape.\n"
    return ("# crib: a kiln figure recipe\n\nWrite `<name>.kiln.json` with `\"kiln\": 0`. kiln lights, lines and rasterizes it at "
            "every grain; the shot shows grains 1, 2 and 4 as kiln drew them, in the recipe's `idle` clip (else its "
            "first clip, else at rest). `check` adds kiln's own lints.\n\n" + doc.read_text() +
            ("\n\n## The worked example: crates/kiln/recipes/knight.json\n\n```json\n" + knight.read_text().strip() + "\n```\n"
             if knight.exists() else ""))


def write(backend, dest, kind="figure"):
    if backend == "kiln":
        Path(dest).write_text(kiln_crib(kind))
        return str(dest)
    text = {"pxl": pxl_crib, "vfx": vfx_crib}[backend]()
    Path(dest).write_text(text)
    return str(dest)
