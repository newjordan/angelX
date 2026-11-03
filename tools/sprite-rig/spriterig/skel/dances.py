"""Creative non-parity dances: python -m spriterig.skel.dances --clips DIR --out showcase/skel/dances"""
import argparse, os, time
import numpy as np
from . import billboard as BB, clips as C
from .__main__ import billboards, _gif, _row
from ..delve import source

DANCES = ["hip_hop_dancing", "salsa_dancing", "breakdance_uprock", "gangnam_style", "thriller_part_2",
          "silly_dancing", "swing_dancing", "robot_hip_hop_dance"]


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--clips", default=os.environ.get("MIXAMO_DIR", "mixamo"))
    ap.add_argument("--out", default="showcase/skel/dances")
    ap.add_argument("--fps", type=int, default=15)
    ap.add_argument("--secs", type=float, default=6)
    a = ap.parse_args(argv)
    os.makedirs(a.out, exist_ok=True)
    pal = source.palette()
    t = time.time()
    clips, skipped = C.load_folder(a.clips, in_place=True)
    print(f"import {len(clips)} clips {time.time() - t:.2f}s skipped={skipped}")
    dances = [d for d in DANCES if d in clips]
    bbs = billboards(source.DEFAULT_ROOT)
    if not dances:
        ap.error("no supported dance clips found; supply --clips DIR")
    if a.fps < 1 or a.secs <= 0:
        ap.error("--fps and --secs must be positive")
    t2, n2 = 0, 0
    for d in dances:
        c = clips[d]; N = int(min(len(c) / c.fps, a.secs) * a.fps)
        f2 = []
        for i in range(N):
            R, h = c.sample(i / a.fps)
            cam = BB.Camera(yaw=i * 360 / N, pitch=15)
            s = time.time()
            f2.append(_row([b.rgba(b.frame(R, h, facing=0, cam=cam)[0], pal) for b in bbs])); t2 += time.time() - s; n2 += len(bbs)
        _gif(f2, os.path.join(a.out, f"{d}_2d.gif"), a.fps, 4)
    # dance party: each 2D character a different dance, orbiting camera
    N = int(a.secs * 1.5 * a.fps)
    party = []
    for i in range(N):
        cam = BB.Camera(yaw=i * 360 / N, pitch=20)
        row = []
        for k, b in enumerate(bbs):
            c = clips[dances[k % len(dances)]]
            row.append(b.rgba(b.frame(*c.sample(i / a.fps), facing=0, cam=cam)[0], pal))
        party.append(_row(row))
    _gif(party, os.path.join(a.out, "dance_party.gif"), a.fps, 5)
    print(f"2D {t2 / n2 * 1000:.2f} ms/char-frame; total {time.time() - t:.0f}s")
    for f in sorted(os.listdir(a.out)):
        print(f, os.path.getsize(os.path.join(a.out, f)) // 1024, "KB")


if __name__ == "__main__":
    main()
