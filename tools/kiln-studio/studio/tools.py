"""The hands and eyes as tools: what the MCP server serves and the job
runner calls. One Studio is rooted in a workspace (a job's variant folder, or
any folder a person points it at); it reads and writes inside that root only,
and reads the library.

Every render, shot, check and describe of a changed source keeps a version:
`versions/vNN<ext>` with its ladder shot `versions/vNN.png`, listed in
`versions/versions.json`, so a job keeps each recipe it tried.
"""
import hashlib
import json
import shutil
import time
from pathlib import Path

from . import backends, library, paths, realm
from .backends import BackendError
from .backends.pxl import prepare as pxl_prepare


class ToolError(Exception):
    """A tool's input was refused; the message says what to fix."""


class Studio:
    def __init__(self, root, kind=None, log=None, brief=None):
        self.root = Path(root).resolve()
        self.root.mkdir(parents=True, exist_ok=True)
        self.kind = kind
        self.log_path = Path(log) if log else self.root / "mcp.log"
        self.brief = brief
        self.bible = load_bible()

    # --- paths, inside the root ---------------------------------------------

    def path(self, p, must_exist=True, readable_outside=()):
        q = Path(p)
        q = (q if q.is_absolute() else self.root / q).resolve()
        inside = q == self.root or self.root in q.parents
        if not inside and not any(base in q.parents for base in readable_outside):
            raise ToolError(f"{p}: outside the workspace {self.root}")
        if must_exist and not q.exists():
            raise ToolError(f"{p}: no such file in {self.root}")
        return q

    def source(self, p):
        q = self.path(p)
        b = backends.for_source(q)
        if b.name == "pxl":
            pxl_prepare(q.parent)
        return q, b

    def _name(self, src, b, name):
        if name:
            return name
        names = b.names(src)
        if not names:
            raise ToolError(f"{src.name} defines no asset (a name not starting with _)")
        return names[0]

    def _reduce_outline(self, r):
        """The ink a reduction keeps on a figure's edge; an effect reduces by plain vote."""
        return None if r.meta.get("kind") == "vfx" else self.bible.get("outline", {}).get("ink", "k")

    def _kind(self, b, src=None):
        if self.kind:
            return self.kind
        if b.name == "vfx":
            return "vfx"
        if b.name == "kiln" and src is not None:
            try:
                return "vfx" if "fx" in json.loads(Path(src).read_text()) else "figure"
            except (OSError, ValueError):
                pass
        return "figure"

    def _lower(self, r):
        """The backend's own frames at grains 1 and 2, when it drew them."""
        lower = {}
        for g, png in ((r.meta or {}).get("grains") or {}).items():
            g = int(g)
            grid = realm.read_grid(png)
            fs = realm.frames_of(grid, r.frame[0] * g // r.grain, r.frame[1] * g // r.grain, r.frames)
            for f in fs:
                f.drawn = True
            lower[g] = fs
        return lower

    def log(self, tool, args, result=None, error=None, t0=None):
        entry = {"t": round(time.time(), 3), "tool": tool, "args": args,
                 "secs": round(time.time() - t0, 3) if t0 else None}
        if error:
            entry["error"] = str(error)[:500]
        try:
            with open(self.log_path, "a") as f:
                f.write(json.dumps(entry) + "\n")
        except OSError:
            pass

    # --- versions -----------------------------------------------------------

    def snapshot(self, src, b, shot=None):
        """Keep the source as a new version if it changed; returns its number."""
        vdir = self.root / "versions"
        vdir.mkdir(exist_ok=True)
        index = vdir / "versions.json"
        listed = json.loads(index.read_text()) if index.exists() else []
        digest = hashlib.sha256(src.read_bytes()).hexdigest()[:16]
        mine = [v for v in listed if v["source"] == src.name]
        if mine and mine[-1]["sha"] == digest:
            v = mine[-1]
        else:
            n = len(listed) + 1
            v = {"n": n, "source": src.name, "sha": digest, "t": round(time.time(), 1),
                 "file": f"v{n:02}{b.ext}"}
            shutil.copyfile(src, vdir / v["file"])
            listed.append(v)
        if shot and not v.get("shot"):
            v["shot"] = f"v{v['n']:02}.png"
            shutil.copyfile(shot, vdir / v["shot"])
        index.write_text(json.dumps(listed, indent=1))
        return v["n"]

    # --- the tools ----------------------------------------------------------

    def _render(self, source, name):
        src, b = self.source(source)
        name = self._name(src, b, name)
        r = b.render(src, name, self.root / "renders")
        r.meta = dict(r.meta or {}, kind=self._kind(b, src), backend=b.name)
        grid = realm.read_grid(r.png)
        frames = realm.frames_of(grid, r.frame[0], r.frame[1], r.frames)
        r.lower = self._lower(r)
        return src, b, name, r, grid, frames

    def render(self, source, name=None):
        src, b, name, r, grid, frames = self._render(source, name)
        v = self.snapshot(src, b)
        return {"name": name, "backend": b.name, "version": v, **r.as_dict(),
                "facts": realm.facts(frames[0], r.grain, small=(r.lower.get(1) or [None])[0])}

    def shot(self, source, name=None):
        src, b, name, r, grid, frames = self._render(source, name)
        out = self.root / "shots" / f"{name}.png"
        outline = self._reduce_outline(r)
        floor = tuple(self.bible.get("floor", {}).get("inks", ["K", "Z"]))
        # An effect is judged at its busiest frame, a figure at its first.
        key = max(range(len(frames)), key=lambda i: sum(len(row) - row.count(".") for row in frames[i].rows)) \
            if r.meta.get("kind") == "vfx" else 0
        realm.ladder(frames, out, grain=r.grain, outline=outline, floor_inks=floor, lower=r.lower, key=key,
                     title=f"{name}  {b.name}  {r.frame[0]}x{r.frame[1]} at grain {r.grain}, {r.frames} frame(s)")
        v = self.snapshot(src, b, shot=out)
        findings = self._lint(r, grid, frames, r.grain)
        res = {"name": name, "backend": b.name, "version": v, "shot": str(out), "render": r.png,
               "frames": r.frames, "frame": list(r.frame), "grain": r.grain,
               "findings": findings, "facts": realm.facts(frames[0], r.grain, outline, small=(r.lower.get(1) or [None])[0])}
        if b.name == "pxl":
            res["pxl"] = b.shot_text(src, name)
        return res

    def _lint(self, r, grid, frames, grain):
        """The eyes' findings: the inks and alpha on the whole strip, the rest per frame.
        An effect is not held to a figure's floor, outline and silhouette, and a
        one-shot may end on a clear frame."""
        kind = r.meta.get("kind", "figure")
        outline = self.bible.get("outline", {}).get("ink", "k")
        rules = dict(self.bible.get("lint", {}))
        found = [f for f in realm.lint(grid, 1, outline, rules) if f["rule"] in ("palette", "alpha")]
        smalls = r.lower.get(1) or [None] * len(frames)
        skip = {"palette", "alpha"} | ({"floor", "outline", "silhouette", "lone", "empty"} if kind == "vfx" else set())
        if kind == "vfx" and all(realm.bbox(f) is None for f in frames):
            found.append({"rule": "empty", "level": "error", "msg": "nothing is drawn in any frame"})
        for i, f in enumerate(frames):
            for item in realm.lint(f, grain, outline, rules, small=smalls[i]):
                if item["rule"] in skip:
                    continue
                if item["rule"] == "lone" and r.meta.get("backend") == "kiln":
                    # kiln's line work cleans lone pixels per grain; a lit volume's tone steps are its own.
                    item = dict(item, level="look")
                item = dict(item, frame=i)
                if not any(x["rule"] == item["rule"] and x["msg"] == item["msg"] for x in found):
                    found.append(item)
        if kind != "vfx" and len(frames) > 1:
            found += motion_lint(frames, grain)
        # Many frames with the same rule: keep the first few.
        kept, seen = [], {}
        for item in found:
            seen[item["rule"]] = seen.get(item["rule"], 0) + 1
            if seen[item["rule"]] <= 3:
                kept.append(item)
        return kept

    def check(self, source, name=None):
        src, b, name, r, grid, frames = self._render(source, name)
        self.snapshot(src, b)
        found = self._lint(r, grid, frames, r.grain) + b.check(src, name)
        errors = [f for f in found if f.get("level") == "error"]
        return {"name": name, "ok": not errors, "errors": len(errors),
                "warnings": sum(1 for f in found if f.get("level") == "warn"), "findings": found}

    def describe(self, source, name=None):
        src, b, name, r, grid, frames = self._render(source, name)
        self.snapshot(src, b)
        outline = self._reduce_outline(r)
        small = (r.lower.get(1) or [None])[0] or realm.reduce(frames[0], r.grain, outline)
        motion = []
        for i in range(1, len(frames)):
            a, c = realm.bbox(frames[i - 1]), realm.bbox(frames[i])
            changed = sum(1 for y in range(frames[i].h) for x in range(frames[i].w)
                          if frames[i].rows[y][x] != frames[i - 1].rows[y][x])
            motion.append({"frame": i, "box": c, "moved": [c[0] - a[0], c[1] - a[1]] if a and c else None,
                           "pixels_changed": changed})
        return {"name": name, "backend": b.name, "facts": realm.facts(frames[0], r.grain, outline, small=small),
                "frames": r.frames, "fps": r.fps, "motion": motion,
                "at_grain_1": small.text(), "backend_text": b.describe(src, name)}

    def compare(self, source, name=None, ref=None, other=None):
        src, b = self.source(source)
        name = self._name(src, b, name)
        if ref:
            refp = self.path(ref, readable_outside=(paths.LIBRARY.resolve(),))
            try:
                return {"name": name, "text": b.compare(src, name, refp)}
            except BackendError:
                pass
            return self._pixel_compare(src, b, name, refp)
        if other:
            o = self.path(other)
            _, _, _, ro, go, fo = self._render(o, None)
            return self._pixel_compare(src, b, name, ro.png)
        raise ToolError("compare wants `ref` (a PNG) or `other` (another source)")

    def _pixel_compare(self, src, b, name, ref_png):
        r = b.render(src, name, self.root / "renders")
        a = realm.read_grid(r.png)
        c = realm.read_grid(ref_png)
        w, h = min(a.w, c.w), min(a.h, c.h)
        diff = sum(1 for y in range(h) for x in range(w) if a.rows[y][x] != c.rows[y][x])
        sil = sum(1 for y in range(h) for x in range(w) if (a.rows[y][x] == ".") != (c.rows[y][x] == "."))
        return {"name": name, "size": [a.w, a.h], "ref_size": [c.w, c.h], "pixels_differ": diff,
                "silhouette_differs": sil, "of": w * h}

    def export(self, source, name=None):
        src, b, name, r, grid, frames = self._render(source, name)
        outline = self._reduce_outline(r)
        out = self.root / "export" / name
        shutil.rmtree(out, ignore_errors=True)
        out.mkdir(parents=True)
        entries, rows = [], []
        for i, f in enumerate(frames):
            png = out / f"{name}-{i}.png"
            realm.to_image(f).save(png)
            small = r.lower[1][i] if r.lower.get(1) else realm.reduce(f, r.grain, outline)
            rows.append(f"# {name} frame {i}, grain 1 ({small.w}x{small.h})\n{small.text()}")
            # The look key is the game's; it is filled when the piece is wired to a look.
            entries.append({"look": "", "name": f"{name}/{i}", "file": png.name, "w": small.w, "h": small.h})
        (out / "rows.txt").write_text("\n\n".join(rows) + "\n")
        (out / "pack.json").write_text(json.dumps({"scale": r.grain, "art": entries}, indent=1) + "\n")
        shutil.copyfile(src, out / src.name)
        shutil.copyfile(r.png, out / f"{name}-strip.png")
        self.snapshot(src, b)
        return {"name": name, "dir": str(out), "files": sorted(p.name for p in out.iterdir()),
                "fps": r.fps, "frames": r.frames}

    def library_search(self, query="", limit=8):
        return {"hits": library.search(query, limit)}

    def library_add(self, name, source, notes="", tags=(), shot=None):
        src, b = self.source(source)
        shot_p = self.path(shot) if shot else None
        return library.add(name, src, notes=notes, tags=list(tags), shot=shot_p, by="mcp")

    def critique(self, shot, brief=None, critic=None):
        from . import critic as critic_mod
        p = self.path(shot)
        brief = brief or self.brief or ""
        if not brief:
            raise ToolError("critique wants the brief it judges against")
        return critic_mod.critique(p, brief, self.root / "critic", panel=[critic] if critic else None)

    def concept(self, prompt, n=1, seed=None):
        from .models import comfy
        out = self.root / "concepts"
        return comfy.Comfy().concepts(prompt, out, n=max(1, min(4, int(n))), seed=seed, kind=self.kind or "figure")


def motion_lint(frames, grain):
    """A figure's loop: its feet stay put, and no frame jumps more than a game pixel."""
    found = []
    first = frames[0]
    box = realm.bbox(first)
    if not box:
        return found
    foot = range(max(0, box[1] + box[3] - 2 * grain), box[1] + box[3])
    for i, f in enumerate(frames[1:], 1):
        moved = sum(1 for y in foot for x in range(min(f.w, first.w)) if f.rows[y][x] != first.rows[y][x])
        if moved > grain * 2:
            found.append({"rule": "feet", "level": "warn", "frame": i,
                          "msg": f"frame {i}: {moved} pixels of the feet (the bottom {2 * grain} rows) differ from frame 0; an idle keeps its feet still"})
    for i in range(1, len(frames)):
        a, b = realm.bbox(frames[i - 1]), realm.bbox(frames[i])
        if a and b and max(abs(a[0] - b[0]), abs(a[1] - b[1]), abs(a[2] - b[2]), abs(a[3] - b[3])) > grain:
            found.append({"rule": "jitter", "level": "warn", "frame": i,
                          "msg": f"frame {i}: the drawn box moves or grows by more than {grain} px (one game pixel) from frame {i - 1}"})
    return found


def load_bible():
    try:
        return json.loads(paths.BIBLE_JSON.read_text())
    except (OSError, ValueError):
        return {}
