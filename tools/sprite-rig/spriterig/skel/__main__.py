"""Creative billboards, NOT native game parity.

python -m spriterig.skel [--out showcase/skel] [--clips ~/Downloads/mixamo] [--bench]"""
import argparse, importlib, os, time
import numpy as np
from PIL import Image
from . import billboard as BB, clips as C
from ..delve import source

K = importlib.import_module("spriterig.delve.cast")


def _gif(frames, path, fps=12, k=6, bg=(34, 30, 38)):
    ims = []
    for f in frames:
        if f.shape[-1] == 4:
            b = np.empty(f.shape[:2] + (3,), np.uint8); b[:] = bg
            f = np.where(f[..., 3:] > 0, f[..., :3], b)
        ims.append(Image.fromarray(f).resize((f.shape[1] * k, f.shape[0] * k), Image.NEAREST))
    ims[0].save(path, save_all=True, append_images=ims[1:], duration=int(1000 / fps), loop=0, disposal=2)


def _row(imgs):
    h = max(i.shape[0] for i in imgs)
    return np.concatenate([np.pad(i, ((h - i.shape[0], 0), (0, 0), (0, 0))) for i in imgs], 1)


def billboards(root):
    return [BB.hero("1", root), BB.hero("2", root), BB.from_character(K.folk("KING", root)),
            BB.from_character(K.folk("GUARD", root)), BB.from_character(K.goblin(root))]


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--angelx", default=source.DEFAULT_ROOT)
    ap.add_argument("--out", default="showcase/skel")
    ap.add_argument("--clips", default=None, help="folder of Mixamo FBX/BVH/GLB clips")
    ap.add_argument("--in-place", action="store_true", help="strip root X/Z travel from imported clips")
    ap.add_argument("--bench", action="store_true")
    a = ap.parse_args(argv)
    os.makedirs(a.out, exist_ok=True)
    pal = source.palette()
    t0 = time.time()
    clips = C.procedural_set()
    print(f"procedural clips: {len(clips)} in {(time.time() - t0) * 1000:.0f} ms")
    if a.clips:
        t = time.time()
        more, skipped = C.load_folder(os.path.expanduser(a.clips), a.in_place)
        print(f"imported {len(more)} clips in {time.time() - t:.2f} s; skipped {skipped}")
        for n, c in more.items():
            clips["mx_" + n] = c
    bbs = billboards(a.angelx)
    # ---- 2D bench: random camera / facing, every clip
    for b in bbs:
        n, t = 0, time.time()
        for ci, c in enumerate(clips.values()):
            for f in range(len(c)):
                b.frame(*c.sample(f / c.fps), facing=(f * 37 + ci * 11) % 360, cam=BB.Camera(f * 13 % 360, (f * 7) % 60))
                n += 1
        dt = time.time() - t
        print(f"2D {b.name:11s} {b.views[0].w}x{b.views[0].h} views={len(b.views)}  {dt / n * 1000:.2f} ms/frame  ({n} frames)")
    if a.bench:
        return
    hero = bbs[0]
    # ---- orbit: the camera circles a walking hero (any camera angle)
    for clip in ["walk", "attack_right", "cast"] + [k for k in clips if k.startswith("mx_")]:
        c = clips[clip]
        fr = []
        N = 48
        for i in range(N):
            cam = BB.Camera(yaw=i * 360 / N, pitch=20)
            row = [b.rgba(b.frame(*c.sample(i / c.fps), facing=0, cam=cam)[0], pal) for b in bbs]
            fr.append(_row(row))
        _gif(fr, os.path.join(a.out, f"orbit_{clip}.gif"), 12, 5)
    # ---- 8 directions sheet
    rows = []
    for clip in ("idle", "walk", "run", "attack_right", "jump"):
        c = clips[clip]
        rows.append(_row([hero.rgba(hero.frame(*c.sample(3 / c.fps), facing=d, cam=BB.Camera(0, 25))[0], pal)
                          for d in range(0, 360, 45)]))
    sh = np.concatenate(rows, 0)
    _gif([sh], os.path.join(a.out, "hero_8dir_sheet.gif"), 1, 6)
    Image.fromarray(sh).resize((sh.shape[1] * 6, sh.shape[0] * 6), Image.NEAREST).save(os.path.join(a.out, "hero_8dir_sheet.png"))
    print(f"total {time.time() - t0:.1f} s -> {a.out}")


if __name__ == "__main__":
    main()
