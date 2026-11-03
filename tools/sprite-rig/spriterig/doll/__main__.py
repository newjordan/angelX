"""Creative procedural dolls, NOT native game-pixel parity. Use spriterig.native for game gear.

Paper-doll animator CLI.

  python -m spriterig.doll export  [-l knight,mage|path.json|all] [-c slash,dab|attacks|emotes|all]
                                   [--views side,front,3q,back|8dir] [--mixamo DIR] [--in-place]
                                   [--out out/doll] [--gif] [--scale 3]
  python -m spriterig.doll check   [-l ...] [-c ...] [--out out/doll/check]      # validator + magenta preview
  python -m spriterig.doll list                                                  # kit contents
  python -m spriterig.doll bench

One command renders character x loadout x clip x view -> transparent sprite sheet PNG +
JSON (frame size, fps, loop, hit/smear events) and optional preview GIF.
"""
import argparse, glob, json, os, sys, time
import numpy as np
from . import dsl, Doll, Loadout, load_kit
from . import draw as Dr
from .anim import frames as animate
from .out import save_sheet, save_gif, tile, bbox
from ..skel.clips import Clip

LIB = os.path.join(os.path.dirname(__file__), "library")
VIEWS = {"side": (0, 20, 90), "front": (0, 20, 15), "3q": (0, 25, 50), "back": (0, 20, 200),
         "left": (0, 20, 270)}
for i, h in enumerate(range(0, 360, 45)):
    VIEWS[f"d{h}"] = (0, 22, h)


def loadouts(spec):
    names = sorted(os.path.basename(p)[:-5] for p in glob.glob(os.path.join(LIB, "loadouts", "*.json")))
    if spec in (None, "all"):
        spec = ",".join(names)
    out = []
    for s in spec.split(","):
        if s.endswith(".json"):
            out.append(Loadout(s))
        elif s in names:
            out.append(Loadout(os.path.join(LIB, "loadouts", s + ".json")))
        else:
            out.append(Loadout({"name": s, "body": s}))   # bare body
    return out


def resample(c, fps=12):
    if c.fps == fps:
        return c
    n = max(int(round(len(c) * fps / c.fps)), 1)
    rot = np.array([c.sample(i / fps)[0] for i in range(n)])
    hips = np.array([c.sample(i / fps)[1] for i in range(n)])
    out = Clip(c.name, rot, hips, fps, c.loop, c.source)
    out.events, out.smear, out.tags, out.meta = {}, set(), ["mixamo"], {}
    return out


def clips_for(spec, lo, lib, mixamo):
    """attacks -> the ones suiting this loadout's weapon; emotes; all; or explicit names."""
    wt = lo.weapon_type()
    pool = {**lib, **mixamo}
    if spec in (None, "all"):
        spec = "attacks,emotes"
    names = []
    for s in spec.split(","):
        if s == "attacks":
            names += [n for n, c in lib.items() if "attack" in c.tags and wt in c.meta.get("weapon", [])]
        elif s == "emotes":
            names += [n for n, c in lib.items() if "emote" in c.tags]
        elif s == "mixamo":
            names += list(mixamo)
        elif s in pool:
            names.append(s)
        else:
            sys.exit(f"unknown clip '{s}' (have: {', '.join(pool)})")
    return [(n, pool[n]) for n in dict.fromkeys(names)]


def load_mixamo(folder, in_place):
    if not folder:
        return {}
    from ..skel.clips import load_folder
    more, skipped = load_folder(os.path.expanduser(folder), in_place)
    return {"mx_" + n.lower().replace(" ", "_"): resample(c) for n, c in more.items()}


def render_set(lo, clip, view, size=None):
    d = Doll(lo)
    yaw, pitch, facing = VIEWS[view]
    return animate(d, clip, yaw=yaw, pitch=pitch, facing=facing, size=size), d


def cmd_export(a):
    lib = dsl.library()
    mx = load_mixamo(a.mixamo, a.in_place)
    views = list(VIEWS)[5:] if a.views == "8dir" else a.views.split(",")
    os.makedirs(a.out, exist_ok=True)
    index, n, t0 = [], 0, time.perf_counter()
    for lo in loadouts(a.loadouts):
        for cname, clip in clips_for(a.clips, lo, lib, mx):
            for v in views:
                fr, d = render_set(lo, clip, v)
                y0, y1, x0, x1 = bbox(fr, 2)
                fr_c = [f[y0:y1, x0:x1] for f in fr]
                base = os.path.join(a.out, f"{lo.name}__{cname}__{v}")
                meta = {"loadout": lo.name, "body": lo.spec.get("body", "hero"), "clip": cname, "view": v,
                        "fps": clip.fps, "loop": bool(clip.loop),
                        "events": {str(k): e for k, e in getattr(clip, "events", {}).items()},
                        "origin": [float(fr[0].shape[1] / 2 - x0), float(fr[0].shape[0] - (d.h * 0.55) * 0.55 - y0)]}
                index.append(save_sheet(fr_c, base + ".png", meta))
                if a.gif:
                    save_gif([tile([f], a.scale) for f in fr_c], base + ".gif", clip.fps)
                n += len(fr)
    dt = time.perf_counter() - t0
    json.dump(index, open(os.path.join(a.out, "index.json"), "w"), indent=1)
    print(f"exported {len(index)} sheets / {n} frames to {a.out} in {dt:.1f} s ({dt / max(n, 1) * 1000:.2f} ms/frame incl. IO)")


def check_frames(fr, clip=None):
    """Per-frame problems: [(kind, frame, mask)]. Rendering already ran the cleanup pass,
    so this flags what would still look broken: off-canvas clipping, empty frames,
    orphans/pinholes (should be 0), and popping (silhouette area jumps). Pops are not
    flagged on smear frames (they are meant to) or in clips tagged `glitch`/`snap`."""
    issues = []
    T = len(clip) if clip is not None else 0
    sm = getattr(clip, "smear", set()) if clip is not None else set()
    nopop = bool({"glitch", "snap"} & set(getattr(clip, "tags", []) or []))
    areas = [int((f > 0).sum()) for f in fr]
    for i, f in enumerate(fr):
        a = f > 0
        if not a.any():
            issues.append(("empty", i, a)); continue
        edge = np.zeros_like(a); edge[0] = edge[-1] = True; edge[:, 0] = edge[:, -1] = True
        if (a & edge).any():
            issues.append(("off-canvas", i, a & edge))
        g = f.copy()
        holes, orphans = Dr.cleanup(g)
        if holes or orphans:
            issues.append((f"stray({holes} holes,{orphans} orphans)", i, g != f))
        j = i - 1
        smeary = T and ((i % T) in sm or (j % T) in sm)
        if j >= 0 and not nopop and not smeary and areas[j] and abs(areas[i] - areas[j]) / areas[j] > 0.45:
            issues.append((f"pop({areas[j]}->{areas[i]}px)", i, a ^ (fr[j] > 0)))
    return issues


def cmd_check(a):
    from PIL import Image
    from .out import rgba
    lib = dsl.library()
    mx = load_mixamo(a.mixamo, a.in_place)
    os.makedirs(a.out, exist_ok=True)
    bad = 0
    for lo in loadouts(a.loadouts):
        for cname, clip in clips_for(a.clips, lo, lib, mx):
            for v in a.views.split(","):
                fr, _ = render_set(lo, clip, v)
                iss = check_frames(fr, clip)
                if not iss:
                    continue
                bad += 1
                print(f"{lo.name:16s} {cname:16s} {v:6s} " + "; ".join(f"f{i}:{k}" for k, i, _ in iss[:6]))
                # preview: offending frames, problem pixels in magenta
                ims = []
                for k, i, m in iss[:8]:
                    im = rgba(fr[i]).copy(); im[m] = (255, 0, 255, 255)
                    ims.append(Image.fromarray(im).resize((im.shape[1] * 4, im.shape[0] * 4), Image.NEAREST))
                W = sum(i.width for i in ims)
                sheet = Image.new("RGBA", (W, ims[0].height), (34, 32, 44, 255))
                x = 0
                for im in ims:
                    sheet.paste(im, (x, 0), im); x += im.width
                sheet.save(os.path.join(a.out, f"{lo.name}__{cname}__{v}.png"))
    print(f"check: {bad} problem set(s)" + (f", previews in {a.out}" if bad else " - all clean"))
    return 1 if bad and a.strict else 0


def cmd_list(a):
    mats, items, bodies = load_kit()
    lib = dsl.library()
    print("bodies:   ", ", ".join(bodies))
    print("loadouts: ", ", ".join(l.name for l in loadouts("all")))
    for k in ("weapon", "shield", "helm", "headsprite", "plume", "pauldron", "chest", "limb", "cape", "robe"):
        print(f"{k + ':':10s}", ", ".join(n for n, i in items.items() if i["kind"] == k))
    print("attacks:  ", ", ".join(f"{n}[{'/'.join(c.meta.get('weapon', []))}]" for n, c in lib.items() if "attack" in c.tags))
    print("emotes:   ", ", ".join(n for n, c in lib.items() if "emote" in c.tags))
    print("other:    ", ", ".join(n for n, c in lib.items() if not {"attack", "emote"} & set(c.tags)))
    print("materials:", ", ".join(f"{k}={''.join(v)}" for k, v in mats.items()))


def cmd_bench(a):
    lib = dsl.library()
    rows = []
    for name in ("peasant", "knight", "mage", "dwarf_berserker"):
        lo = loadouts(name)[0]
        d = Doll(lo)
        n, t = 0, time.perf_counter()
        for cn, c in lib.items():
            for f in range(len(c)):
                d.frame(c.rot[f], c.hips[f], yaw=(f * 37) % 360, pitch=20, facing=90, smear=f in c.smear)
                n += 1
        ms = (time.perf_counter() - t) / n * 1000
        rows.append((name, n, ms, d.h))
        print(f"{name:16s} {n:4d} frames  {ms:.2f} ms/frame  ({d.h}px)")
    cmd_bench.rows = rows


def main(argv=None):
    ap = argparse.ArgumentParser(prog="python -m spriterig.doll", description="Creative procedural dolls (non-game parity); use spriterig.native for game assets.")
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("export", "check"):
        p = sub.add_parser(name)
        p.add_argument("-l", "--loadouts", default="all")
        p.add_argument("-c", "--clips", default="all")
        p.add_argument("--views", default="side" if name == "export" else "side,front,3q,back")
        p.add_argument("--mixamo", default=None, help="folder of Mixamo FBX clips (never committed)")
        p.add_argument("--in-place", action="store_true")
        p.add_argument("--out", default=f"out/doll/{name}")
        if name == "export":
            p.add_argument("--gif", action="store_true")
            p.add_argument("--scale", type=int, default=3)
        else:
            p.add_argument("--strict", action="store_true", help="exit 1 on any problem (CI)")
    sub.add_parser("list"); sub.add_parser("bench")
    a = ap.parse_args(argv)
    return {"export": cmd_export, "check": cmd_check, "list": cmd_list, "bench": cmd_bench}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main() or 0)
