"""pxl (~/tools/claude-pxl, or KILN_PXL): a `.px.js` file in, PNGs out.

The source keeps to the realm's inks with `use("realm.hex")`; the studio puts
a copy of realm.hex beside every source it hands an agent.
"""
import json
import os
import re
import shutil
from pathlib import Path

from .. import paths
from .base import Backend, BackendError, Render, run

LIST_LINE = re.compile(r"^\s+\d+(?:-\d+)?\s+(sprite|anim|scene|rig|sheet|tilemap|group)\w*:\s+(\S+)")


class PxlBackend(Backend):
    name = "pxl"
    ext = ".px.js"
    kinds = ("figure", "prop")

    def _pxl(self, *args, cwd=None, timeout=120):
        return run([paths.NODE, paths.PXL, *args], cwd=cwd, env=paths.node_env(), timeout=timeout)

    def available(self):
        if not Path(paths.PXL).exists():
            return False, f"no pxl at {paths.PXL}"
        code, out, err = self._pxl("help", "use", timeout=30)
        return (True, "pxl at " + paths.PXL) if code == 0 else (False, (err or out).strip()[:300])

    def names(self, source):
        """The assets, the piece first: the one named after the file, then
        anims, then the rest, in file order."""
        code, out, err = self._pxl("list", str(source))
        if code:
            raise BackendError((err or out).strip()[:2000])
        found = []
        for line in out.splitlines():
            m = LIST_LINE.match(line)
            if m and not m.group(2).startswith("_"):
                found.append((m.group(1), m.group(2)))
        stem = Path(source).name[: -len(self.ext)]
        found.sort(key=lambda t: (t[1] != stem, t[0] != "anim"))
        return [n for _, n in found]

    def render(self, source, name, out_dir):
        source = Path(source)
        out = Path(out_dir) / f"pxl-{name}"
        shutil.rmtree(out, ignore_errors=True)
        code, text, err = self._pxl("build", str(source), "--only", name, "--out", str(out), "--no-preview", cwd=source.parent)
        if code:
            raise BackendError(f"pxl build failed:\n{(err or text).strip()[:3000]}")
        for sub in ("anims", "sprites", "scenes", ""):
            png = out / sub / f"{name}.png"
            if png.exists():
                break
        else:
            raise BackendError(f"pxl build made no {name}.png:\n{text.strip()[:1500]}")
        from PIL import Image
        w, h = Image.open(png).size
        meta = png.with_suffix(".json")
        frames, fps, frame = 1, 0, (w, h)
        if meta.exists():
            data = json.loads(meta.read_text())
            rects = list(data.get("frames", {}).values())
            if rects:
                frames = len(rects)
                f0 = rects[0]["frame"]
                frame = (f0["w"], f0["h"])
                ms = rects[0].get("duration") or 0
                fps = round(1000 / ms, 2) if ms else 0
        files = [str(p) for p in sorted(out.rglob("*")) if p.is_file()]
        return Render(png=str(png), frame=frame, frames=frames, grain=self.grain, fps=fps, files=files,
                      notes=(err.strip() + "\n" + text.strip()).strip()[:1500])

    def check(self, source, name):
        code, out, err = self._pxl("check", str(source), name, cwd=Path(source).parent)
        found = []
        for line in (out + "\n" + err).splitlines():
            line = line.strip()
            if not line or line.startswith(("no warnings", "ran:", "checked")):
                continue
            level = "look" if "LOOK" in line else ("error" if code and "rror" in line else "warn")
            found.append({"rule": "pxl", "level": level, "msg": line[:600]})
        return found[:40]

    def describe(self, source, name):
        code, out, err = self._pxl("describe", str(source), name, "--brief", cwd=Path(source).parent)
        if code:
            code, out, err = self._pxl("describe", str(source), name, cwd=Path(source).parent)
        text = (out or err).strip()
        # The ink map is the eyes' job; keep pxl's structure and painting steps short.
        return text[:6000]

    def compare(self, source, name, ref):
        code, out, err = self._pxl("compare", str(source), name, str(ref), cwd=Path(source).parent)
        if code:
            raise BackendError((err or out).strip()[:2000])
        return out.strip()[:6000]

    def shot_text(self, source, name):
        """pxl's own shot: what changed since the last one. Its PNG is the
        pxl view (16x); the studio's ladder is the one to judge by."""
        code, out, err = self._pxl("shot", str(source), name, "--frames", "all", cwd=Path(source).parent)
        return (out + err).strip()[:2000]

    def template(self, name, kind):
        return f"""// {name}: drawn at grain 4 (64 px a tile) in the realm's inks.
use("realm.hex")

sprite("{name}", {{ w: 64, h: 64, outline: "k", pivot: "bottom" }},
  rect(28, 40, 8, 20, "U"))
"""


def prepare(dirpath):
    """Put realm.hex beside a pxl source so `use("realm.hex")` finds it."""
    dst = Path(dirpath) / "realm.hex"
    if not dst.exists():
        shutil.copyfile(paths.REALM_HEX, dst)
    return str(dst)
