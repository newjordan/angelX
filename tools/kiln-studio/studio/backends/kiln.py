"""kiln, the realm's own renderer: a recipe (`<name>.kiln.json`) in, every grain out.

The `kiln` command (crates/kiln, feature `cli`; `KILN_BIN`, else `kiln` on
PATH, else crates/target/fast/kiln) takes both kinds of recipe,
told apart by their version field:

    {"kiln": 0, ...}  a figure (docs/KILN-RECIPE.md): lit volumes on a rig, clips
    {"fx": 0, ...}    an effect (docs/KILN-FX.md): shapes and emitters timed in beats

`kiln render` writes a PNG a frame a grain; the backend puts each grain's
frames side by side. A figure is shown in its `idle` clip (else its first,
else at rest). kiln draws each grain itself, so a render carries real grain
1 and 2 strips beside grain 4, and the studio's ladder shows them instead of
reductions. Without the command an effect still renders through kiln-fx
(tools/kiln-studio/fx-render), and a figure job falls back to pxl.
"""
import json
import re
import shutil
from pathlib import Path

from .. import paths
from .base import Backend, BackendError, Render, run

WROTE = re.compile(r"^wrote (.+\.png) \(\d+x\d+\)$")
FRAME = re.compile(r"-g(\d)-(\d+)\.png$")
STILL = re.compile(r"-g(\d)\.png$")


def kind_of(recipe):
    return "vfx" if "fx" in recipe else "figure"


def clip_of(recipe):
    """The clip a figure is judged in: idle, else its first, else none (at rest)."""
    clips = recipe.get("clips") or {}
    return "idle" if "idle" in clips else next(iter(clips), None)


def strips(files, out):
    """Each grain's frames, side by side: {grain: (strip path, frames, (w, h))}."""
    from PIL import Image
    by = {}
    for f in files:
        m = FRAME.search(f)
        if m:
            by.setdefault(int(m.group(1)), {})[int(m.group(2))] = f
    if not by:  # a figure at rest: one frame a grain
        for f in files:
            m = STILL.search(f)
            if m:
                by.setdefault(int(m.group(1)), {})[0] = f
    found = {}
    for g, frames in by.items():
        ims = [Image.open(frames[i]).convert("RGBA") for i in sorted(frames)]
        w, h = ims[0].size
        strip = Image.new("RGBA", (w * len(ims), h), (0, 0, 0, 0))
        for i, im in enumerate(ims):
            strip.paste(im, (i * w, 0))
        path = Path(out) / f"strip-g{g}.png"
        strip.save(path)
        found[g] = (str(path), len(ims), (w, h))
    return found


class KilnBackend(Backend):
    name = "kiln"
    ext = ".kiln.json"
    kinds = ("figure", "prop", "vfx")

    def available(self, kind=None):
        cli, fx = paths.kiln_bin(), paths.kiln_fx_bin()
        if cli:
            return True, f"kiln at {cli}"
        if kind == "vfx" and fx:
            return True, f"kiln-fx at {fx} (effects only; the kiln command is not found)"
        if kind is None and fx:
            return True, f"kiln-fx at {fx}: effects only, figures wait for the kiln command (KILN_BIN)"
        return False, "the kiln command is not found (KILN_BIN, PATH, crates/target/fast/kiln)"

    def _recipe(self, source):
        try:
            return json.loads(Path(source).read_text())
        except ValueError as e:
            raise BackendError(f"{Path(source).name}: not JSON: {e}") from None

    def names(self, source):
        return [Path(source).name[: -len(self.ext)]]

    def render(self, source, name, out_dir):
        recipe = self._recipe(source)
        out = Path(out_dir) / f"kiln-{name}"
        shutil.rmtree(out, ignore_errors=True)
        out.mkdir(parents=True)
        cli = paths.kiln_bin()
        fx = kind_of(recipe) == "vfx"
        clip = None if fx else clip_of(recipe)
        if not cli:
            if fx and paths.kiln_fx_bin():
                return self._render_fx_fallback(source, out)
            raise BackendError("the kiln command is not found (KILN_BIN, PATH, crates/target/fast/kiln)")
        cmd = [cli, "render", str(source), "--grain", "1,2,4", "--out", str(out)] + (["--clip", clip] if clip else [])
        code, text, err = run(cmd, timeout=60)
        if code:
            raise BackendError((err or text).strip()[:3000])
        files = [m.group(1) for m in (WROTE.match(l.strip()) for l in text.splitlines()) if m]
        got = strips(files, out)
        if 4 not in got:
            raise BackendError(f"kiln wrote no grain-4 frames:\n{text.strip()[:800]}")
        png, n, frame = got[4]
        fps = recipe.get("fps", 12) if fx else ((recipe.get("clips") or {}).get(clip) or {}).get("fps", 0)
        return Render(png=png, frame=frame, frames=n, grain=4, fps=fps, files=[g[0] for g in got.values()],
                      notes=text.strip().splitlines()[-1][:300] if text.strip() else "",
                      meta={"grains": {str(g): v[0] for g, v in got.items() if g != 4}, "clip": clip,
                            "clips": list((recipe.get("clips") or {}).keys())})

    def _render_fx_fallback(self, source, out):
        code, text, err = run([paths.kiln_fx_bin(), "render", str(source), "--out", str(out), "--grains", "1,2,4"], timeout=60)
        try:
            info = json.loads(text.strip().splitlines()[-1]) if text.strip() else {}
        except ValueError:
            info = {}
        if code or info.get("error"):
            raise BackendError(info.get("error") or (err or text).strip()[:3000])
        s = info.get("strips") or {}
        w, h = info["canvas"]
        return Render(png=s["4"], frame=(w * 4, h * 4), frames=int(info["frames"]), grain=4, fps=info.get("fps", 0),
                      files=list(s.values()), meta={"grains": {g: p for g, p in s.items() if g != "4"}, "via": "kiln-fx"})

    def check(self, source, name):
        """kiln's own lints, for a figure (an effect has none beyond reading)."""
        cli = paths.kiln_bin()
        if not cli or kind_of(self._recipe(source)) == "vfx":
            return []
        code, text, err = run([cli, "check", str(source), "--json"], timeout=60)
        try:
            report = json.loads(text)
        except ValueError:
            return [{"rule": "kiln", "level": "error", "msg": (err or text).strip()[:600]}] if code else []
        level = {"error": "error", "warn": "warn", "warning": "warn"}
        return [{"rule": "kiln:" + l.get("code", "?"), "level": level.get(l.get("level"), "look"),
                 "msg": l.get("message", "") + (f" (grain {l['grain']})" if l.get("grain") else "")}
                for l in report.get("lints", [])][:40]

    def describe(self, source, name):
        r = self._recipe(source)
        cli = paths.kiln_bin()
        if cli and kind_of(r) == "figure":
            code, text, err = run([cli, "describe", str(source)], timeout=60)
            return (text or err).strip()[:6000]
        lines = [f"{r.get('name', name)}: canvas {r.get('canvas')} game px, pivot {r.get('pivot', 'the centre')}, "
                 f"fps {r.get('fps', 12)}, frames {r.get('frames', 'from its beats')}, beats {r.get('beats')}, "
                 f"{'a loop' if r.get('loop') else 'one shot'}, outline {r.get('outline', False)}"]
        for n, layer in enumerate(r.get("layers") or []):
            rest = ", ".join(f"{k} {v}" for k, v in layer.items() if k not in ("type", "ramp", "inks"))
            lines.append(f"  layer {n}: {layer.get('type')} in {layer.get('ramp') or layer.get('inks')} {rest}")
        return "\n".join(lines)

    def compare(self, source, name, ref):
        cli = paths.kiln_bin()
        if not cli or kind_of(self._recipe(source)) == "vfx":
            raise BackendError("kiln compares figures; the studio compares the pixels")
        code, text, err = run([cli, "compare", str(source), "--ref", str(ref)], timeout=60)
        if code:
            raise BackendError((err or text).strip()[:2000])
        return text.strip()[:6000]

    def template(self, name, kind):
        if kind == "vfx":
            return json.dumps({"fx": 0, "name": name, "canvas": [16, 16], "layers": [{"type": "ring", "ramp": "frost"}]}, indent=1)
        return json.dumps({"kiln": 0, "name": name, "canvas": [16, 16], "pivot": [8, 15.5], "parts": []}, indent=1)
