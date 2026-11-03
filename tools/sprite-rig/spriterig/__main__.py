import argparse, json, os, time
import numpy as np
from PIL import Image
from .segment import load_sprite, segment, load_override, PARTS
from .rig import build_rig
from .anim import ANIMS, DEFAULT_SET
from .render import render_frames
from . import export


def debug_parts(labels, path):
    rng = np.random.default_rng(3)
    pal = rng.integers(60, 255, (len(PARTS), 3))
    img = np.zeros(labels.shape + (4,), np.uint8)
    k = labels >= 0
    img[k, :3] = pal[labels[k]]; img[k, 3] = 255
    Image.fromarray(img).save(path)


def pick_mode(rgba, mode):
    if mode != "auto":
        return mode
    a = rgba[..., 3]
    hard = np.isin(a, (0, 255)).mean() > 0.995
    return "nearest" if hard and rgba.shape[0] <= 96 else "aa"


def run(inp, out, anims=DEFAULT_SET, frames=0, override=None, cell=None, scale=1, mode="auto",
        ss=None, write_frames=True, gif=True, montage=True, name=None):
    os.makedirs(out, exist_ok=True)
    T0 = time.perf_counter()
    o = load_override(override) if override else {}
    rgba = load_sprite(inp)
    lab, J = segment(rgba, o)
    rig = build_rig(rgba, lab, J, cell=cell or o.get("cell", 6), side_f=o.get("side_f", "r"),
                    facing=o.get("facing", "right"), amp=o.get("amp"),
                    backfill=o.get("backfill", True))
    t_rig = time.perf_counter() - T0
    mode = pick_mode(rgba, o.get("mode", mode) if mode == "auto" else mode)
    h, w = rig["shape"]
    m = int(0.75 * h)  # generous canvas for jump / death; cropped to the union bbox below
    canvas, offset = (w + 2 * m, h + 2 * m), (m, m)
    timing = {"input": inp, "mode": mode, "scale": scale, "rig_ms": round(1000 * t_rig, 2),
              "triangles": int(sum(len(p.tris) for p in rig["parts"])), "anims": {}}
    clips = {}
    n_total, t_render = 0, 0.0
    for a in anims.split(","):
        fn, n, fps = ANIMS[a]
        n = frames or n
        t0 = time.perf_counter()
        clips[a] = (render_frames(rig, fn, n, scale=scale, mode=mode, ss=ss, canvas=canvas,
                                  offset=offset), fps, getattr(fn, "loop", True))
        dt = time.perf_counter() - t0
        t_render += dt; n_total += n
        timing["anims"][a] = {"frames": n, "ms_per_frame": round(1000 * dt / n, 2)}
    box = export.union_bbox([c[0] for c in clips.values()], pad=4 * scale)
    ground_y = int((offset[1] + h) * scale - box[1])
    for a, (fr, fps, loop) in clips.items():
        fr = export.crop_all(fr, box)
        clips[a] = (fr, fps, loop)
        export.save_sheet(fr, os.path.join(out, f"{a}_sheet.png"), fps, loop)
        if write_frames:
            d = os.path.join(out, a); os.makedirs(d, exist_ok=True)
            for i, f in enumerate(fr):
                Image.fromarray(f).save(os.path.join(d, f"{i:03d}.png"))
    t_core = time.perf_counter() - T0
    timing.update({"render_ms_per_frame": round(1000 * t_render / max(n_total, 1), 2),
                   "frames_total": n_total, "core_s": round(t_core, 3)})
    if gif or montage:
        t1 = time.perf_counter()
        sx = (offset[0] + J["root"][0]) * scale - box[0]
        allimgs, durs = [], []
        for a, (fr, fps, loop) in clips.items():
            imgs = [export.compose(f, ground_y, shadow_x=sx) for f in fr]
            if gif:
                export.save_gif(imgs, os.path.join(out, f"{a}.gif"), fps)
            if montage:
                lab_imgs = [export.label(im, a) for im in imgs] * (2 if loop else 1)
                if not loop:
                    lab_imgs += [lab_imgs[-1]] * max(3, fps // 2)
                allimgs += lab_imgs
        if montage:
            export.save_gif(allimgs, os.path.join(out, f"{name or 'montage'}_montage.gif"), 12)
        timing["gif_s"] = round(time.perf_counter() - t1, 3)
    debug_parts(lab, os.path.join(out, "parts.png"))
    with open(os.path.join(out, "rig.json"), "w") as f:
        json.dump({"joints": J, "parts": [p.name for p in rig["parts"]],
                   "triangles": timing["triangles"]}, f, indent=1)
    timing["total_s"] = round(time.perf_counter() - T0, 3)
    with open(os.path.join(out, "timing.json"), "w") as f:
        json.dump(timing, f, indent=1)
    return timing


def creative_main(argv=None):
    ap = argparse.ArgumentParser(prog="spriterig creative", description="Creative procedural animation; NOT game-pixel parity.")
    ap.add_argument("input")
    ap.add_argument("--anims", default=DEFAULT_SET, help="comma list or 'all'")
    ap.add_argument("--out", default="out")
    ap.add_argument("--frames", type=int, default=0, help="override frame count")
    ap.add_argument("--override", help="JSON: joints/parts/polys/fractions/side_f/facing/amp/cell/mode")
    ap.add_argument("--cell", type=int, default=None, help="mesh cell size px (default 6)")
    ap.add_argument("--scale", type=int, default=1, help="output scale (rendered, not resized)")
    ap.add_argument("--mode", default="auto", choices=["auto", "aa", "nearest"],
                    help="aa = supersampled bilinear; nearest = crisp pixel art")
    ap.add_argument("--ss", type=int, default=None, help="supersample factor for aa (default 2 = 4 bilinear taps per output px)")
    ap.add_argument("--no-frames", action="store_true", help="skip per-frame PNGs")
    ap.add_argument("--no-gif", action="store_true")
    ap.add_argument("--no-montage", action="store_true")
    ap.add_argument("--name", default=None)
    a = ap.parse_args(argv)
    anims = DEFAULT_SET if a.anims == "all" else a.anims
    t = run(a.input, a.out, anims, a.frames, a.override, a.cell, a.scale, a.mode, a.ss,
            not a.no_frames, not a.no_gif, not a.no_montage, a.name)
    print(json.dumps({k: v for k, v in t.items() if k != "anims"}))


def main(argv=None):
    import sys
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv and argv[0] == "quick":
        from .quick import main as quick_main
        return quick_main(argv[1:])
    if argv and argv[0] == "creative":
        return creative_main(argv[1:])
    # Keep the original positional PNG invocation compatible, but explicit.
    if argv and not argv[0].startswith("-"):
        print("Creative animation (non-game-parity); prefer 'spriterig creative'.")
        return creative_main(argv)
    from .native import main as native_main
    return native_main(argv)


if __name__ == "__main__":
    main()
