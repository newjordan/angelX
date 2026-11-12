#!/work/artifacts/kwpixel/pipeline/.venv/bin/python
"""Turn a creature's native frames into a boss-library baked set for the Creator's preview feed.

    scripts/creator/export-baked-set.py --spec scripts/creator/frameset/knight.json
    scripts/creator/export-baked-set.py --spec /work/artifacts/kwpixel/3d/render/<id>/frameset.json

One command rebuilds a set, so a better creature drops in with no Rust beyond one registry row
(`REGISTRY` in `crates/delve/src/render/creation/baked.rs`). Nothing is redrawn, resampled,
recoloured or invented: every pixel written is a pixel the source rendered. Any source works that
can name its frames: a T-pose rig, a Blender render pixelised to the grid, a hand-drawn sheet.

The frame-set spec (JSON; relative paths resolve against `root`, default the spec's own folder):

    {
      "id": "knight",                       set id and output folder name
      "name": "KNIGHT",                     what the stage and the plate call it ("STAND-IN: KNIGHT")
      "role": "hero",                       the Creator role it is meant to show (registry text only)
      "root": "/abs/folder",                optional
      "fps": 12,                            authored clip rate (default 12)
      "cell": [64, 54],                     optional, the library's v1 envelope is the default
      "anchor": [32, 53],                   optional: the pixel edge the soles stand on; default is
                                            read from the rest pose (its last opaque row + 1). The
                                            library's convention is soles on row 52, edge 53.
      "props": ["..."],                     optional identity props for the registry text
      "grade": [320, 320],                  optional library exposure (native, cell)
      "dye": {"hues": [205, 250], "min_sat": 0.1, "min_val": 0.16},
                                            optional: which pixels the main pigment turns (hue window
                                            in degrees); derived from the art when absent
      "clips": {                            every clip is optional
        "idle":       {"frames": "frames/idle"},
        "walk-east":  {"frames": "frames/walk-side"},     the side walk, facing right
        "walk-south": {"frames": "frames/walk", "anchor_y": 51},   toward the camera
        "attack":     {"frames": "frames/attack",
                       "events": {"wind_up": 10, "swing_arc": 16, "impact": 17, "hit_pause": 18}}
      },
      "poses": {"rest": "idle[0]", "windup": "attack[15]", "strike": "attack[17]"}   see below
    }

`frames` is a folder of PNGs (sorted), a list of PNG paths, or one strip PNG of equal cells. A clip
may say `anchor_y` (its own sole edge, when its frames are authored higher or lower in the cell),
`stride_px` ([x, y], the clip's authored root travel per cycle, informational). The attack's
`events` use the motion library's names (`wind_up`, `swing_arc`, `impact`, `hit_pause`); only
`impact` is required. A pose is `clip[index]` or a PNG file. Defaults: rest = idle[0], windup = the
frame before the swing, strike = the impact frame, so a set with no idle or attack clip names its
three poses itself, as the boss library does. A clip a set lacks is simply absent: no idle loop
means the library's breathing rule, no walk strip means the idle bob at a stride, no attack clip
means the windup and strike poses are the blow.

Shortcut for a folder already in the boss library's layout (`library-v1-rest.png`, `-windup.png`,
`-strike.png`, and any of `library-v1-{walk-east,walk-south,idle,attack}-strip.png`; an
`animation.json` beside it supplies the attack's keys as the Bailiff's does):

    scripts/creator/export-baked-set.py --library-dir /work/artifacts/kwpixel/3d/render/<id> --id <id> --name ORC

It writes the Bailiff's set (`crates/delve/assets/creator/baked/marsh-bailiff/`) and more:
library-v1-{rest,windup,strike}.png, walk east and south strips, an idle strip and the attack as a
strip (a clip the set lacks gets one blank cell, and no state in the record), `library-import.json`
in the Bailiff's schema, a registry entry, and `animation.json` (kwpixel.overhead-animation/1, with a
`creator` block carrying the name and the dye window).

Shortcut for the pipeline's eight-direction frame set (`manifest.json` + `frames/<clip>/<dir>/NNNN.png`,
or the wish pipeline's `<dir>/<clip>/NNNN.png`; dirs s sw w nw n ne e se; 64x54 RGBA; SpriteKiln's
atlas.json shape plus a creator block, see night-20261107/import/NOTES.md):

    scripts/creator/export-baked-set.py --frameset /work/artifacts/kwpixel/3d/sk-a6-final/game64x54/manifest.json \
        --id a6-crusader --name "CRUSADER A6"
    scripts/creator/export-baked-set.py --spec scripts/creator/frameset/a6-crusader.json

(a spec may carry `"frameset": "<manifest>"` beside the id, name, role and props). The clips idle, walk,
run, attack (also `slash`), hit (`hurt`), death (`die`) and cast are read, each in all eight directions,
with their fps, hold ticks (a held frame is repeated once per 12 fps tick) and events. The set is written
as an eight-way set: one atlas `library-v1-ways.png` (a block of eight direction rows per clip, a clip's
frames running along each row) and a `ways` block in `animation.json` carrying every clip's frame count,
loop flag, events and per-direction sole edge. The legacy files are written as well, taken from the
south (idle, attack, walk-south) and east (walk-east) directions, so a reader that knows only East and
South still loads the set and plays those two ways. A set exported without a frame set (every older one)
has no `ways` block and behaves exactly as before.

It refuses a set that is not the library's shape: frames of the cell size, binary alpha (soft edges
are thresholded and said), RGB zero under transparent, no frame above the 54-row ceiling, the rest
pose equal to the idle clip's first frame when it is taken from it. Art-quality findings (more than
32 colours, a frame in two pieces, a frame touching a side of its cell) are printed as warnings, or
refused with --strict.
"""
import argparse
import colorsys
import hashlib
import json
import re
import sys
from pathlib import Path

import numpy as np
from PIL import Image
from scipy import ndimage

DEFAULT_CELL = (64, 54)          # the boss library's v1 envelope, bottom-centre registered
MAX_ROWS = 54                    # documented policy: the common 54 px v1 height ceiling
LIBRARY_EDGE = 53                # the pixel edge library figures stand on: soles on row 52
CLIPS = ("idle", "walk-east", "walk-south", "attack")


def die(msg):
    sys.exit(f"export-baked-set: {msg}")


def sha(data):
    h = hashlib.sha256()
    h.update(data if isinstance(data, bytes) else Path(data).read_bytes())
    return h.hexdigest()


def clean(a):
    """Binary alpha, zero RGB under transparent (the library's delivery rule)."""
    a = a.copy()
    opaque = a[..., 3] > 127
    a[..., 3] = np.where(opaque, 255, 0)
    a[~opaque, :3] = 0
    return a


def colours(a):
    m = a[..., 3] > 0
    return {tuple(int(v) for v in p) for p in a[m][:, :3]}


def read_png(path):
    return np.array(Image.open(path).convert("RGBA"))


def frames_of(spec_frames, root, cell, label):
    """The frames a clip names: a folder, a list of files, or a strip of equal cells."""
    w, h = cell
    if isinstance(spec_frames, list):
        files = [root / f for f in spec_frames]
    else:
        p = root / spec_frames
        if p.is_dir():
            files = sorted(p.glob("*.png"))
        elif p.is_file():
            strip = read_png(p)
            if strip.shape[0] != h or strip.shape[1] % w:
                die(f"{label}: strip {p} is {strip.shape[1]}x{strip.shape[0]}, cells are {w}x{h}")
            return [strip[:, i * w:(i + 1) * w].copy() for i in range(strip.shape[1] // w)], [p]
        else:
            die(f"{label}: {p} is neither a folder nor a PNG")
    if not files:
        die(f"{label}: no frames in {spec_frames}")
    missing = [f for f in files if not f.is_file()]
    if missing:
        die(f"{label}: missing {missing[0]}")
    return [read_png(f) for f in files], files


def pose(ref, clips, root, cell, label):
    """A pose: `clip[index]`, or a PNG file (relative to the root) of one cell."""
    m = re.fullmatch(r"([\w-]+)\[(\d+)\]", ref)
    if m:
        clip, i = m.group(1), int(m.group(2))
        if clip not in clips or i >= len(clips[clip]):
            die(f"{label}: pose `{ref}` names a frame that is not there")
        return clips[clip][i]
    p = root / ref
    if not p.is_file():
        die(f"{label}: pose `{ref}` is neither `clip[index]` nor a PNG under {root}")
    return clean(read_png(p))


def check_frame(a, label, cell, errors, warnings):
    w, h = cell
    if a.shape[:2] != (h, w):
        errors.append(f"{label}: {a.shape[1]}x{a.shape[0]}, the library cell is {w}x{h}")
        return
    al = a[..., 3]
    if set(np.unique(al).tolist()) - {0, 255}:
        errors.append(f"{label}: alpha is not binary")
    if (a[al == 0][:, :3] > 0).any():
        errors.append(f"{label}: RGB under transparent pixels")
    opaque = al > 0
    if not opaque.any():
        errors.append(f"{label}: empty frame")
        return
    ys, xs = np.where(opaque)
    if ys.max() - ys.min() + 1 > MAX_ROWS:
        errors.append(f"{label}: taller than the {MAX_ROWS}-row ceiling")
    pieces = ndimage.label(opaque, structure=np.ones((3, 3)))[1]
    if pieces != 1:
        warnings.append(f"{label}: {pieces} pieces")
    if xs.min() < 1 or xs.max() > w - 2:
        warnings.append(f"{label}: touches a side of its cell (x {xs.min()}..{xs.max()})")


def derive_dye(frames):
    """The main material's hue window: the heaviest hue band of the rest pose's saturated, lit pixels."""
    hist = np.zeros(360)
    for a in frames:
        m = a[..., 3] > 0
        for r, g, b in a[m][:, :3]:
            h, s, v = colorsys.rgb_to_hsv(r / 255, g / 255, b / 255)
            if s >= 0.15 and v >= 0.16:
                hist[int(h * 360) % 360] += 1
    if hist.sum() == 0:
        return None
    k = np.ones(15) / 15
    smooth = np.convolve(np.concatenate([hist[-7:], hist, hist[:7]]), k, mode="valid")
    peak = int(smooth.argmax())
    floor = smooth[peak] * 0.2
    lo = hi = peak
    while smooth[(lo - 1) % 360] >= floor and peak - lo < 60:
        lo -= 1
    while smooth[(hi + 1) % 360] >= floor and hi - peak < 60:
        hi += 1
    if lo < 0 or hi >= 360:
        return None  # a window that wraps through red: say it in the spec instead
    return {"hues": [float(lo), float(hi)], "min_sat": 0.1, "min_val": 0.16}


def strip(frames, cell):
    w, h = cell
    out = np.zeros((h, w * len(frames), 4), np.uint8)
    for i, f in enumerate(frames):
        out[:, i * w:(i + 1) * w] = f
    return out


def save_png(a, path):
    Image.fromarray(a, "RGBA").save(path, optimize=False, compress_level=9)


def spec_from_library_dir(folder, args):
    """A frame-set spec for a folder already in the boss library's layout."""
    d = Path(folder).resolve()
    poses = {"rest": "library-v1-rest.png", "windup": "library-v1-windup.png", "strike": "library-v1-strike.png"}
    for f in poses.values():
        if not (d / f).is_file():
            die(f"--library-dir: no {f} in {d}")
    spec = {"id": args.id, "name": args.name or args.id.upper(), "role": args.role, "root": str(d), "poses": poses, "clips": {}}
    for clip, f in {"idle": "library-v1-idle-strip.png", "walk-east": "library-v1-walk-east-strip.png",
                    "walk-south": "library-v1-walk-south-strip.png", "attack": "library-v1-attack-strip.png"}.items():
        if (d / f).is_file():
            spec["clips"][clip] = {"frames": f}
    if "attack" in spec["clips"]:
        record = d / "animation.json"
        keys = json.loads(record.read_text()).get("attack_events") if record.is_file() else None
        if not keys:
            die("--library-dir with an attack strip needs an animation.json beside it with `attack_events` (windup_key, swing, strike)")
        spec["clips"]["attack"]["events"] = {"wind_up": keys["windup_key"], "swing_arc": keys["swing"], "impact": keys["strike"]}
    return spec


WAY_NAMES = ("s", "sw", "w", "nw", "n", "ne", "e", "se")   # the pipeline's order, clockwise from the camera
WAY_CLIPS = ("idle", "walk", "run", "attack", "hit", "death", "cast")
WAY_ALIASES = {"slash": "attack", "hurt": "hit", "die": "death"}
LOOPING = ("idle", "walk", "run")
EVENT_NAMES = {"windup": "wind_up", "wind_up": "wind_up", "windup_key": "wind_up", "swing": "swing_arc",
               "swing_arc": "swing_arc", "hit": "impact", "impact": "impact", "strike": "impact", "hit_pause": "hit_pause"}


def load_frameset(manifest_path, warnings):
    """Read a pipeline frame-set manifest into one shape: fps, and per canonical clip the loop flag, the
    events (name -> frame index, in 12 fps ticks), and per direction the frame files, the hold ticks and the
    sole edge. Two layouts are read: `kwpixel.frameset/1` (`frames/<clip>/<dir>/`, events as {name: index})
    and the wish pipeline's game set (`<dir>/<clip>/`, per-direction ticks, events as {index: [names]})."""
    mp = Path(manifest_path).resolve()
    if not mp.is_file():
        die(f"no frame-set manifest at {mp}")
    root, m = mp.parent, json.loads(mp.read_text())
    dirs = m.get("directions")
    if isinstance(dirs, dict):
        dirs = list(dirs)
    if not dirs or set(dirs) != set(WAY_NAMES) or len(dirs) != len(WAY_NAMES):
        die(f"{mp.name}: the frame set must carry all eight directions {', '.join(WAY_NAMES)}, it says {dirs}")
    cell = m.get("frame_size") or m.get("cell")
    out = {"fps": int(m.get("fps", 12)), "cell": tuple(cell) if cell else None, "clips": {}, "manifest": str(mp),
           "contract": m.get("contract") or m.get("format") or "unnamed"}
    for raw, c in (m.get("clips") or {}).items():
        name = WAY_ALIASES.get(raw, raw)
        if name not in WAY_CLIPS:
            warnings.append(f"clip `{raw}` is not one the Creator plays ({', '.join(WAY_CLIPS)}): left out")
            continue
        if name in out["clips"]:
            die(f"{mp.name}: two clips both mean `{name}`")
        per_dir = c.get("directions")   # the game-set layout: everything per direction
        files, anchors, ticks_by, events_by = {}, {}, {}, {}
        for d in WAY_NAMES:
            e = per_dir[d] if per_dir else c
            cands = [root / "frames" / raw / d, root / d / raw] + ([root / e["folder"]] if e.get("folder") else [])
            folder = next((p for p in cands if p.is_dir()), None)
            if folder is None:
                die(f"{mp.name}: no frames for {raw}/{d} (looked in {[str(p) for p in cands]})")
            fs = sorted(folder.glob("*.png"))
            if len(fs) != int(e["frames"]):
                die(f"{mp.name}: {raw}/{d} has {len(fs)} PNGs, the manifest says {e['frames']}")
            ticks = [int(t) for t in e.get("ticks") or [1] * len(fs)]
            if len(ticks) != len(fs):
                die(f"{mp.name}: {raw}/{d} has {len(ticks)} tick counts for {len(fs)} frames")
            start = [sum(ticks[:i]) for i in range(len(fs))]   # a source frame's first tick
            ev = e.get("events") or {}
            ev = {n: start[int(i)] for i, names in ev.items() for n in names} if per_dir else {n: start[int(i)] for n, i in ev.items()}
            ay = e.get("anchor_y") if per_dir else (c.get("anchor_y") or {}).get(d)
            ay = m.get("sole_edge") if ay is None else ay
            if ay is None:
                die(f"{mp.name}: {raw}/{d} has no sole edge (`anchor_y`)")
            files[d], ticks_by[d], events_by[d], anchors[d] = fs, ticks, ev, int(ay)
        if len({tuple(t) for t in ticks_by.values()}) != 1 or len({tuple(sorted(v.items())) for v in events_by.values()}) != 1:
            die(f"{mp.name}: clip `{raw}` differs in timing or events between directions: one timeline per clip")
        out["clips"][name] = {"source": raw, "loop": bool(c.get("loop", name in LOOPING)), "files": files,
                              "ticks": ticks_by["s"], "events": events_by["s"], "anchor_y": anchors}
    if "idle" not in out["clips"]:
        die(f"{mp.name}: an eight-way set needs an idle clip (its rest pose and the stage's turn)")
    return out


def build_ways(fs, errors, warnings):
    """The eight-way clips as clean 12 fps tick frames per direction: a held frame repeated, a one-shot's
    trailing hold trimmed to one frame (the player holds a finished clip on its last), a loop's repeated
    first frame at the seam dropped, one timeline per clip."""
    clips = {}
    for name in WAY_CLIPS:
        c = fs["clips"].get(name)
        if not c:
            continue
        ticks = list(c["ticks"])
        if not c["loop"] and len(ticks) > 1:
            ticks[-1] = 1
        per_dir = {}
        for d in WAY_NAMES:
            frames = [clean(read_png(f)) for f, t in zip(c["files"][d], ticks) for _ in range(t)]
            soft = sum(int(((read_png(f)[..., 3] > 0) & (read_png(f)[..., 3] < 255)).sum()) for f in c["files"][d])
            if soft:
                warnings.append(f"{name}/{d}: {soft} semi-transparent pixels thresholded to binary alpha")
            per_dir[d] = frames
        dropped = False
        if c["loop"] and all(len(per_dir[d]) > 2 and (per_dir[d][-1] == per_dir[d][0]).all() for d in WAY_NAMES):
            per_dir = {d: f[:-1] for d, f in per_dir.items()}
            dropped = True
        clips[name] = {"frames": per_dir, "n": len(per_dir["s"]), "loop": c["loop"], "events": c["events"],
                       "anchor_y": c["anchor_y"], "dropped_seam": dropped, "source": c["source"],
                       "files": [f for d in WAY_NAMES for f in c["files"][d]]}
        clips[name]["events"] = {k: min(v, clips[name]["n"] - 1) for k, v in c["events"].items()}
    return clips


def check_ways(clips, cell, errors, warnings):
    """The library's checks on every frame of every direction, reported once per clip and kind."""
    for name, c in clips.items():
        tally = {}
        for d in WAY_NAMES:
            for i, f in enumerate(c["frames"][d]):
                e, w = [], []
                check_frame(f, f"{name}/{d}[{i}]", cell, e, w)
                errors.extend(e)
                for line in w:
                    kind = re.sub(r"\(x \d+\.\.\d+\)", "", line.split(": ", 1)[1]).strip()
                    t = tally.setdefault(kind, [0, f"{d}[{i}]"])
                    t[0] += 1
        for kind, (count, first) in tally.items():
            warnings.append(f"{name}: {count} of {8 * c['n']} frames {kind} (first {first})")


def ways_atlas(clips, cell):
    """One PNG: a block of eight direction rows per clip (WAY_CLIPS order), frames along each row."""
    cw, ch = cell
    order = [n for n in WAY_CLIPS if n in clips]
    atlas = np.zeros((ch * len(WAY_NAMES) * len(order), cw * max(c["n"] for c in clips.values()), 4), np.uint8)
    rows = {}
    for k, name in enumerate(order):
        rows[name] = k * len(WAY_NAMES)
        for di, d in enumerate(WAY_NAMES):
            for i, f in enumerate(clips[name]["frames"][d]):
                y0 = (rows[name] + di) * ch
                atlas[y0:y0 + ch, i * cw:(i + 1) * cw] = f
    return atlas, rows


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--spec", help="the frame-set spec (JSON)")
    ap.add_argument("--library-dir", help="a folder already in the boss library's layout (needs --id)")
    ap.add_argument("--frameset", help="the pipeline's eight-direction frame-set manifest (needs --id)")
    ap.add_argument("--id", help="with --library-dir or --frameset: the set id")
    ap.add_argument("--name", help="with --library-dir or --frameset: what the stage calls it (default: the id, upper case)")
    ap.add_argument("--role", default="hero", help="with --library-dir or --frameset: the Creator role it is meant to show")
    ap.add_argument("--out", help="default: crates/delve/assets/creator/baked/<id> in this worktree")
    ap.add_argument("--strict", action="store_true", help="art-quality warnings refuse the set")
    a = ap.parse_args()
    if [bool(a.spec), bool(a.library_dir), bool(a.frameset)].count(True) != 1:
        die("give exactly one of --spec, --library-dir and --frameset")

    clips, files, errors, warnings = {}, {}, [], []
    frameset = ways = None
    if a.library_dir:
        if not a.id:
            die("--library-dir needs --id")
        spec, spec_name = spec_from_library_dir(a.library_dir, a), f"--library-dir {a.library_dir}"
        root = Path(spec["root"])
    elif a.frameset:
        if not a.id:
            die("--frameset needs --id")
        spec = {"id": a.id, "name": a.name or a.id.upper(), "role": a.role, "frameset": a.frameset}
        spec_name = f"--frameset {a.frameset}"
        root = Path(a.frameset).resolve().parent
    else:
        spec_path = Path(a.spec).resolve()
        spec, spec_name = json.loads(spec_path.read_text()), spec_path.name
        root = Path(spec.get("root") or spec_path.parent)
    if spec.get("frameset"):
        # The pipeline's eight-direction set: the legacy clips (south idle, attack and walk, east walk)
        # are taken from it and the whole set is written beside them as the ways atlas.
        frameset = load_frameset(Path(spec["frameset"]) if Path(spec["frameset"]).is_absolute() else root / spec["frameset"], warnings)
        ways = build_ways(frameset, errors, warnings)
        spec = {k: v for k, v in spec.items() if k != "frameset"}
        spec.setdefault("fps", frameset["fps"])
        spec.setdefault("cell", list(frameset["cell"] or DEFAULT_CELL))
        spec["clips"] = {}
        for out_name, (clip, d) in {"idle": ("idle", "s"), "walk-east": ("walk", "e"), "walk-south": ("walk", "s"), "attack": ("attack", "s")}.items():
            if clip in ways:
                spec["clips"][out_name] = {"anchor_y": ways[clip]["anchor_y"][d], "inline": ways[clip]["frames"][d], "inline_files": ways[clip]["files"]}
        if "attack" in ways:
            ev = {EVENT_NAMES[k]: v for k, v in ways["attack"]["events"].items() if k in EVENT_NAMES}
            if "impact" not in ev:
                die(f"the attack clip needs a `hit` (or `impact`) event: its events are {ways['attack']['events']}")
            spec["clips"]["attack"]["events"] = ev
        spec.setdefault("poses", {"rest": "idle[0]"})
        root = Path(frameset["manifest"]).parent
    sid = spec.get("id") or die("the spec needs an `id`")
    cell = tuple(spec.get("cell") or DEFAULT_CELL)
    cw, ch = cell
    fps = int(spec.get("fps", 12))
    repo = Path(__file__).resolve().parent.parent.parent
    out = Path(a.out) if a.out else repo / "crates/delve/assets/creator/baked" / sid
    unknown = set(spec.get("clips", {})) - set(CLIPS)
    if unknown:
        die(f"unknown clips {sorted(unknown)}; the clips are {', '.join(CLIPS)}")

    for name in CLIPS:
        if name not in spec.get("clips", {}):
            continue
        c = spec["clips"][name]
        if "inline" in c:
            frames, fs = c["inline"], c["inline_files"]
        else:
            frames, fs = frames_of(c["frames"], root, cell, name)
        # The library's alpha is binary. A source with soft edges (anti-aliasing) is thresholded, and said so.
        soft = sum(int(((f[..., 3] > 0) & (f[..., 3] < 255)).sum()) for f in frames)
        if soft:
            warnings.append(f"{name}: {soft} semi-transparent pixels thresholded to binary alpha (the look has no anti-aliasing at native)")
        clips[name] = [clean(f) for f in frames]
        files[name] = fs

    # A loop whose last frame repeats its first would hold that pose twice at the seam.
    dropped = []
    for name in ("idle", "walk-east", "walk-south"):
        f = clips.get(name, [])
        if len(f) > 2 and f[-1].shape == f[0].shape and (f[-1] == f[0]).all():
            clips[name] = f[:-1]
            dropped.append(name)

    # The attack's keys: the motion library's event names, only `impact` required.
    swing = clips.get("attack")
    swing_arc = wind_up = impact = hit_pause = None
    if swing:
        ev = {k: int(v) for k, v in (spec["clips"]["attack"].get("events") or {}).items()}
        impact = ev.get("impact", ev.get("strike"))
        if impact is None:
            die("the attack clip needs an `impact` event (the frame the blow lands)")
        swing_arc = ev.get("swing_arc", ev.get("swing", impact - 1))
        wind_up = ev.get("wind_up", ev.get("windup_key", max(swing_arc - 1, 0)))
        hit_pause = ev.get("hit_pause", impact + 1)
        if not (0 <= wind_up <= swing_arc <= impact < len(swing)):
            die(f"attack events wind_up {wind_up}, swing_arc {swing_arc}, impact {impact} must rise inside its {len(swing)} frames")

    poses = spec.get("poses", {})

    def pose_of(kind, default):
        ref = poses.get(kind, default)
        if ref is None:
            die(f"no {kind} pose: name it in `poses` (`clip[index]` or a PNG file), or give the clip it comes from")
        return pose(ref, clips, root, cell, kind)

    rest = pose_of("rest", "idle[0]" if "idle" in clips else None)
    windup = pose_of("windup", f"attack[{max(swing_arc - 1, 0)}]" if swing else None)
    strike = pose_of("strike", f"attack[{impact}]" if swing else None)

    if ways:
        check_ways(ways, cell, errors, warnings)
        idle = ways["idle"]
        for d in WAY_NAMES:
            rows = np.where(idle["frames"][d][0][..., 3] > 0)[0]
            if rows.size and int(rows.max()) + 1 != idle["anchor_y"][d]:
                warnings.append(f"idle/{d}: the manifest's sole edge is {idle['anchor_y'][d]}, the first frame's pixels end at {int(rows.max()) + 1}")
    else:
        for name, frames in clips.items():
            for i, f in enumerate(frames):
                check_frame(f, f"{name}[{i}]", cell, errors, warnings)
    for label, f in (("rest", rest), ("windup", windup), ("strike", strike)):
        check_frame(f, label, cell, errors, warnings)
    if "rest" not in poses and "idle" in clips and not (rest == clips["idle"][0]).all():
        errors.append("rest is not the idle clip's first frame")
    palette = set()
    for f in [rest, windup, strike] + [f for frames in clips.values() for f in frames] + [f for c in (ways or {}).values() for d in WAY_NAMES for f in c["frames"][d]]:
        palette |= colours(f)
    if len(palette) > 128:
        errors.append(f"{len(palette)} colours: not one master palette")
    elif len(palette) > 32:
        warnings.append(f"{len(palette)} colours (the look is at most 32 per character)")
    # Where the soles stand: the rest pose says, unless the spec does.
    ys = np.where(rest[..., 3] > 0)[0]
    anchor = list(spec.get("anchor") or (cw // 2, int(ys.max()) + 1)) if ys.size else [cw // 2, LIBRARY_EDGE]
    if anchor[1] != LIBRARY_EDGE:
        warnings.append(f"soles stand on edge {anchor[1]}, the library's is {LIBRARY_EDGE}: the drive will draw it {LIBRARY_EDGE - anchor[1]} rows lower")
    if errors:
        die("refusing to write the set:\n  " + "\n  ".join(errors))
    if warnings and a.strict:
        die("refusing to write the set (--strict):\n  " + "\n  ".join(warnings))

    # Where the motion starts, read from the pixels (a hold is identical frames); recovery begins
    # at the first change after the slam stops changing (or just after the impact).
    anticipation = recovery = settled = None
    if swing:
        diffs = [int((swing[i] != swing[i - 1]).any(axis=2).sum()) for i in range(1, len(swing))]
        anticipation = min(next((f for f in range(1, len(swing)) if diffs[f - 1]), 0), wind_up)
        after = range(impact + 1, len(swing))
        first_still = next((f for f in after if not diffs[f - 1]), None)
        recovery = next((f for f in after if first_still is not None and f > first_still and diffs[f - 1]),
                        min(max(hit_pause, impact + 1), len(swing) - 1))
        settled = len(swing) - 1

    out.mkdir(parents=True, exist_ok=True)
    blank = np.zeros((ch, cw, 4), np.uint8)   # a clip the set lacks is one blank cell, and no state in the record
    written = {
        "library-v1-rest.png": rest,
        "library-v1-windup.png": windup,
        "library-v1-strike.png": strike,
        "library-v1-idle-strip.png": strip(clips.get("idle") or [blank], cell),
        "library-v1-walk-east-strip.png": strip(clips.get("walk-east") or [blank], cell),
        "library-v1-walk-south-strip.png": strip(clips.get("walk-south") or [blank], cell),
        "library-v1-attack-strip.png": strip(swing or [blank], cell),
    }
    atlas_rows = {}
    if ways:
        written["library-v1-ways.png"], atlas_rows = ways_atlas(ways, cell)
    for name, arr in written.items():
        save_png(arr, out / name)

    def opaque_count(arr):
        return int((arr[..., 3] > 0).sum())

    props = spec.get("props") or [sid]
    grade = list(spec.get("grade") or (320, 320))
    role = spec.get("role", "hero")
    note = ("Creator art, not a boss: never registered in the boss library. It is read by the Creator's "
            "preview and Test drive only (render/creation/baked.rs).")
    outputs = {
        name: {
            "colours": len(colours(arr)),
            "edge_agreement": 1.0,
            "key_colour_agreement": 1.0,
            "opaque_pixels": opaque_count(arr),
            "sha256": sha(out / name),
            "silhouette_iou": 1.0,
            "size": [cw, ch],
        }
        for name, arr in (("library-v1-rest.png", rest), ("library-v1-strike.png", strike), ("library-v1-windup.png", windup))
    }
    import_record = {
        "boss": sid,
        "camera": "overhead",
        "definition_sha256": None,
        "display_policy": {
            "activation_note": note,
            "active": False,
            "anchor": "bottom-centre",
            "cell_moss_mix": 0,
            "cell_props": props[:2],
            "class": role,
            "grade": {"v1": grade},
            "props": props,
            "study": {"v1": [], "v2": []},
            "v1": [cw, ch],
            "v2": None,
        },
        "identity_props": props,
        "integration_status": "art-only: v1 only (no v2 art, no study sizes); no gameplay definition or registration is fabricated",
        "outputs": outputs,
        "reference_policy": "Frames are the source's native renders, written 1:1 (no resampling): the fidelity fields are 1.0 by construction, not a quality score.",
        "resample": "nearest",
        "thresholds": {"edge_agreement": 0.85, "key_colour_agreement": 0.9, "silhouette_iou": 0.98},
    }
    (out / "library-import.json").write_text(json.dumps(import_record, indent=2, sort_keys=True) + "\n")
    registry = {
        sid: {
            "active": False, "activation_note": note, "anchor": "bottom-centre", "class": role,
            "cell_moss_mix": 0, "props": props, "cell_props": props[:2], "v1": [cw, ch], "v2": None,
            "grade": {"v1": grade}, "study": {"v1": [], "v2": []},
        }
    }
    (out / "library-registry-entry.json").write_text(json.dumps(registry, indent=2) + "\n")

    dye = spec.get("dye") or derive_dye([rest] + clips.get("idle", [])[:1])

    def state(strip_name, frames, loop, anchor_y, stride=None):
        st = {
            "frames": [{"file": strip_name, "atlas_rect": [i * cw, 0, cw, ch], "duration_ticks": 1} for i in range(len(frames))],
            "loop": loop,
            "anchor": [anchor[0], anchor_y],
        }
        if stride is not None:
            st["root_stride_native_px"] = stride
        return st

    def clip_anchor(name):
        return int(spec["clips"][name].get("anchor_y", anchor[1]))

    states = {}
    if "idle" in clips:
        states["idle"] = state("library-v1-idle-strip.png", clips["idle"], True, clip_anchor("idle"))
    if "walk-south" in clips:
        st = spec["clips"]["walk-south"].get("stride_px") or [0.0, 0.0]
        states["walk-south"] = state("library-v1-walk-south-strip.png", clips["walk-south"], True, clip_anchor("walk-south"), st)
    if "walk-east" in clips:
        st = spec["clips"]["walk-east"].get("stride_px") or [0.0, 0.0]
        states["walk-east"] = state("library-v1-walk-east-strip.png", clips["walk-east"], True, clip_anchor("walk-east"), st)
    if swing:
        states["attack"] = state("library-v1-attack-strip.png", swing, False, clip_anchor("attack"))
    if "walk-east" in clips:
        st = spec["clips"]["walk-east"].get("stride_px") or [0.0, 0.0]
        states["walk-west"] = {"mirror_of": "walk-east", "flip_x": True, "loop": True,
                               "anchor": [anchor[0], clip_anchor("walk-east")], "root_stride_native_px": [-st[0], st[1]]}
    animation = {
        "format": "kwpixel.overhead-animation/1",
        "id": sid,
        "camera": "overhead",
        "fps": fps,
        "native_frame": [cw, ch],
        "anchor_convention": f"bottom-centre, pixel [{cw // 2},{LIBRARY_EDGE}]; soles on row {LIBRARY_EDGE - 1}. A state's anchor y is the edge its soles stand on in its own frames: the drive draws it `{LIBRARY_EDGE} - anchor_y` rows lower.",
        "palette": f"master palette ({len(palette)} colours), ungraded source RGBA",
        "sampling": "nearest",
        "alpha": "binary 0/255",
        "atlas": None,
        "states": states,
        "creator": {
            "name": spec.get("name", sid.upper()),
            "dye": dye,
            "clips": [c for c in CLIPS if c in clips],
            "loop_duplicates_dropped": dropped,
            **({"ways": True} if ways else {}),
        },
        "runtime_status": "read by the Creator preview and Test drive (render/creation/baked.rs); the boss library itself still consumes only rest and windup.",
        "gait_phase": "Phase by distance travelled, not by the clock: the Test drive sets one cycle length per run unit whatever the frame count, so its pace is the same for every creature.",
    }
    if ways:
        # The eight-way clips: every clip's block of direction rows in the atlas, its frames along a row, its
        # loop flag, its events (12 fps tick indices) and the sole edge of each direction's frames.
        animation["ways"] = {
            "directions": list(WAY_NAMES),
            "atlas": "library-v1-ways.png",
            "cell": [cw, ch],
            "fps": fps,
            "clips": {
                name: {
                    "row": atlas_rows[name],
                    "frames": ways[name]["n"],
                    "loop": ways[name]["loop"],
                    "events": ways[name]["events"],
                    "anchor_y": {d: ways[name]["anchor_y"][d] for d in WAY_NAMES},
                    "source": ways[name]["source"],
                    **({"loop_duplicates_dropped": True} if ways[name]["dropped_seam"] else {}),
                }
                for name in WAY_CLIPS if name in ways
            },
            "source": {"manifest": frameset["manifest"], "contract": frameset["contract"]},
        }
        animation["creator"]["ways_clips"] = [c for c in WAY_CLIPS if c in ways]
    if swing:
        animation["attack_events"] = {
            "anticipation_starts": anticipation,
            "windup_key": wind_up,
            "windup_hold_ticks": [wind_up, max(swing_arc - 1, wind_up)],
            "swing": swing_arc,
            "strike": impact,
            "impact_hold_ticks": [impact, hit_pause],
            "recovery_starts": recovery,
            "settled": settled,
            "total_ticks": len(swing),
        }
        animation["creator"]["windup_frame"] = next((i for i, f in enumerate(swing) if (f == windup).all()), None)
        animation["creator"]["strike_frame"] = next((i for i, f in enumerate(swing) if (f == strike).all()), None)
    (out / "animation.json").write_text(json.dumps(animation, indent=2) + "\n")

    def tree_hash(fs):
        return sha(b"".join(Path(f).read_bytes() for f in fs))

    provenance = {
        "spec": spec_name,
        "source": str(root),
        "script": "scripts/creator/export-baked-set.py",
        "clips": {n: [len(files[n]), tree_hash(files[n])] for n in CLIPS if n in files},
        "picks": {k: poses.get(k, d) for k, d in (("rest", "idle[0]"), ("windup", f"attack[{max((swing_arc or 1) - 1, 0)}]"), ("strike", f"attack[{impact}]"))},
        "written": {name: sha(out / name) for name in sorted(written)},
        "unregistered": True,
    }
    if ways:
        provenance["ways"] = {n: [len(c["files"]), tree_hash(c["files"])] for n, c in ways.items()}
        provenance["ways_source"] = {"manifest": frameset["manifest"], "sha256": sha(frameset["manifest"])}
    (out / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")

    print(f"{sid}: {out}")
    print(f"  {len(palette)} colours; rest {opaque_count(rest)} px; soles on edge {anchor[1]}; dye {dye}")
    counts = ", ".join(f"{n} {len(clips[n])}" for n in CLIPS if n in clips) or "no clips (three poses only)"
    if ways:
        counts = "eight-way: " + ", ".join(f"{n} {ways[n]['n']}" for n in WAY_CLIPS if n in ways) + "; legacy: " + counts
    print(f"  {counts}" + (f"; loop seam duplicates dropped: {', '.join(dropped)}" if dropped else ""))
    if swing:
        print(f"  attack keys: anticipation {anticipation}, wind-up {wind_up}, swing {swing_arc}, impact {impact}, recovery {recovery}, settled {settled}")
    for w in warnings:
        print(f"  warning: {w}")


if __name__ == "__main__":
    main()
