"""The Bend pixel-VFX cartridge (~/.angelX/cartridges/vfx), read-only.

The source is the cartridge's own v1 recipe (`<name>.vfx.json`: size, frames,
fps, loop, seed, outline, dither, layers; its README has the fields), written
by hand. Its prompt mode is not used: it turns most words into a flat disc.

The cartridge knows nothing of the realm, so the studio holds its colours to
the realm's inks here. A layer's `ramp` is one of:

    "$fire"        a realm ramp the style bible names (bible.json `vfx_ramps`)
    "ink:a456w"    realm ink keys, dark to light
    "$own"         a ramp the recipe defines in `ramps`, in realm colours only

The cartridge's own ramp names (`fire`, `ice`, ...), `palette` and `hue` are
refused: they paint outside the realm. Its `outline` is drawn in a fixed
#201819 (engine.bend `finish`); the studio redraws exactly that colour in the
bible's outline ink on its own copy of the sheet.
"""
import json
import os
import re
import shutil
import time
from pathlib import Path

from .. import paths
from ..realm import realm
from .base import Backend, BackendError, Render, run


CARTRIDGE_OUTLINE = (0x20, 0x18, 0x19)


def outline_in_realm(sheet, out):
    """The sheet with the cartridge's outline colour in the bible's outline ink; (path, pixels changed)."""
    from PIL import Image
    try:
        key = json.loads(paths.BIBLE_JSON.read_text())["outline"]["ink"]
    except (OSError, ValueError, KeyError):
        key = "k"
    ink = realm().rgb[key]
    im = Image.open(sheet).convert("RGBA")
    px = im.load()
    n = 0
    for y in range(im.size[1]):
        for x in range(im.size[0]):
            r, g, b, a = px[x, y]
            if a and (r, g, b) == CARTRIDGE_OUTLINE:
                px[x, y] = ink + (a,)
                n += 1
    im.save(out)
    return str(out), n


def bible_ramps():
    try:
        return json.loads(paths.BIBLE_JSON.read_text()).get("vfx_ramps", {})
    except (OSError, ValueError):
        return {}


def realmed(recipe):
    """The recipe with every ramp resolved to realm colours, or BackendError
    naming the first path that leaves the realm."""
    if not isinstance(recipe, dict):
        raise BackendError("a vfx recipe is a JSON object")
    pal = realm()
    named = bible_ramps()
    r = json.loads(json.dumps(recipe))
    for field in ("palette", "hue"):
        if r.get(field) not in (None, 0):
            raise BackendError(f"{field}: refused, it paints outside the realm's inks; use realm ramps")
    own = r.get("ramps") or {}
    for key, colours in own.items():
        for i, c in enumerate(colours):
            c = c.lstrip("#").lower()
            if tuple(int(c[j:j + 2], 16) for j in (0, 2, 4)) not in pal.by_rgb:
                raise BackendError(f"ramps.{key}[{i}]: #{c} is not a realm ink; use ink keys (\"ink:a456\") or a bible ramp")
    used = {}
    for n, layer in enumerate(r.get("layers") or []):
        ramp = layer.get("ramp")
        if ramp is None:
            raise BackendError(f"layers[{n}].ramp: every layer names a realm ramp (one of {', '.join('$' + k for k in named)})")
        if isinstance(ramp, list):
            raise BackendError(f"layers[{n}].ramp: a colour list; use \"ink:<keys>\" so it stays in the realm")
        if ramp.startswith("ink:"):
            keys = ramp[4:]
            try:
                pal.ramp(keys)
            except ValueError as e:
                raise BackendError(f"layers[{n}].ramp: {e}") from None
            if not 2 <= len(keys) <= 16:
                raise BackendError(f"layers[{n}].ramp: 2 to 16 inks, not {len(keys)}")
            slot = "ink_" + re.sub(r"[^A-Za-z0-9]", lambda m: "%02x" % ord(m.group()), keys)
            used[slot] = [pal.hex(k) for k in keys]
            layer["ramp"] = "$" + slot
        elif ramp.startswith("$"):
            key = ramp[1:]
            if key in own:
                continue
            if key not in named:
                raise BackendError(f"layers[{n}].ramp: no ramp {ramp!r}; the bible's are {', '.join('$' + k for k in named)}")
            used[key] = [pal.hex(k) for k in pal.ramp(named[key])]
        else:
            raise BackendError(f"layers[{n}].ramp: {ramp!r} is the cartridge's own colours; use a realm ramp "
                               f"({', '.join('$' + k for k in named)}) or \"ink:<keys>\"")
    if used:
        r["ramps"] = {**own, **used}
    return r


class VfxBackend(Backend):
    name = "vfx"
    ext = ".vfx.json"
    kinds = ("vfx",)

    def _bin(self, tool):
        return str(Path(paths.VFX) / tool)

    def available(self):
        if not Path(self._bin("vfx")).exists():
            return False, f"no vfx cartridge at {paths.VFX}"
        code, out, err = run([paths.NODE, self._bin("vfx"), "catalog"], env=paths.node_env(), timeout=30)
        if code:
            return False, (err or out).strip()[:300]
        try:
            return True, "vfx " + json.loads(out).get("version", "?")
        except ValueError:
            return True, "vfx"

    def _recipe(self, source):
        try:
            return json.loads(Path(source).read_text())
        except ValueError as e:
            raise BackendError(f"{source}: not JSON: {e}") from None

    def names(self, source):
        return [Path(source).name[: -len(self.ext)]]

    def render(self, source, name, out_dir):
        recipe = realmed(self._recipe(source))
        work = Path(out_dir).resolve()
        work.mkdir(parents=True, exist_ok=True)
        folder = f"vfx-{name}-{time.time_ns()}"
        env = dict(paths.node_env(), ANGEL_WORKSPACE=str(work))
        body = json.dumps({"recipe": recipe, "out": folder, "frames": False})
        code, out, err = run([paths.NODE, self._bin("vfx-render")], cwd=work, env=env, stdin=body, timeout=90)
        if code:
            raise BackendError(f"vfx render failed:\n{(err or out).strip()[:3000]}")
        try:
            res = json.loads(out)
        except ValueError:
            raise BackendError(f"vfx render said: {out.strip()[:1500]}") from None
        if res.get("error"):
            raise BackendError(f"vfx: {res['error']}")
        files = res.get("files", [])
        sheet = next((f for f in files if f.endswith(".png") and "/frames/" not in f and "@" not in Path(f).name), None)
        if not sheet:
            raise BackendError(f"vfx made no sheet: {files}")
        for old in work.glob(f"vfx-{name}-*"):
            if old.name != folder:
                shutil.rmtree(old, ignore_errors=True)
        atlas = next((f for f in files if f.endswith(".json") and not f.endswith(".recipe.json")), None)
        meta = {k: res.get(k) for k in ("loop", "seed", "impactFrame", "elapsedMs")}
        if recipe.get("outline", "none") != "none":
            sheet, meta["outline_pixels_inked"] = outline_in_realm(sheet, Path(sheet).with_suffix(".realm.png"))
        if atlas:
            data = json.loads(Path(atlas).read_text())
            meta["pivot"] = data.get("meta", {}).get("pivot") or data.get("pivot")
        return Render(png=sheet, frame=(res["w"], res["h"]), frames=res["frames"], grain=self.grain,
                      fps=res.get("fps", 0), files=files, meta=meta)

    def describe(self, source, name):
        r = self._recipe(source)
        lines = [f"{r.get('name', name)}: {r.get('w', 64)}x{r.get('h', 64)}, {r.get('frames', 16)} frames at "
                 f"{r.get('fps', 15)} fps, {'a loop' if r.get('loop') else 'one shot'}, seed {r.get('seed', 1)}, "
                 f"outline {r.get('outline', 'none')}, dither {r.get('dither', 0)}"]
        for n, layer in enumerate(r.get("layers") or []):
            rest = ", ".join(f"{k} {v}" for k, v in layer.items() if k not in ("type", "ramp"))
            lines.append(f"  layer {n}: {layer.get('type')} in {layer.get('ramp')} (t {layer.get('t0', 0)}-{layer.get('t1', 1)}) {rest}")
        return "\n".join(lines)

    def template(self, name, kind):
        return json.dumps({"name": name, "w": 64, "h": 64, "frames": 16, "fps": 15, "loop": False, "seed": 1,
                           "outline": "outer", "layers": [{"type": "burst", "ramp": "$fire"}]}, indent=2) + "\n"
