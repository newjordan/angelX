"""python -m spriterig.delve [--angelx ROOT] [--out DIR] [--only a,b] [--anims ...]
Creative cut-out animation, NOT native game-pixel parity. For game assets use spriterig.native."""
import argparse, json, os, time
from .cast import cast as full_cast
from . import source
from .out import export, render_all
from ..anim import DEFAULT_SET


def main(argv=None):
    ap = argparse.ArgumentParser(prog="spriterig.delve", description=__doc__)
    ap.add_argument("--angelx", default=source.DEFAULT_ROOT)
    ap.add_argument("--out", default="out/delve")
    ap.add_argument("--only", default="")
    ap.add_argument("--anims", default=DEFAULT_SET)
    ap.add_argument("--mode", default="pixel", choices=["pixel", "rotate"])
    ap.add_argument("--bench", action="store_true", help="time only, write nothing")
    ap.add_argument("--no-demo", action="store_true")
    a = ap.parse_args(argv)
    pal = source.palette(a.angelx)
    anims = a.anims.split(",")
    rows = []
    for ch in full_cast(a.angelx):
        if a.only and ch.name not in a.only.split(","):
            continue
        t0 = time.perf_counter()
        from .cutout import Renderer
        Renderer(ch)
        t1 = time.perf_counter()
        r, out, _ = render_all(ch, anims, mode=a.mode)
        t2 = time.perf_counter()
        n = sum(len(v[0]) for v in out.values())
        if not a.bench:
            export(ch, pal, os.path.join(a.out, ch.name), anims, mode=a.mode)
        t3 = time.perf_counter()
        rows.append((ch.name, f"{ch.w}x{ch.h}", (t1 - t0) * 1e3, (t2 - t1) * 1e3 / n, n, t2 - t0, t3 - t0))
        print(f"{ch.name:12s} {ch.w}x{ch.h}  rig {rows[-1][2]:.1f} ms  {rows[-1][3]:.2f} ms/frame  "
              f"{n} frames  render {rows[-1][5]:.2f} s  with export {rows[-1][6]:.2f} s")
    if not a.bench and not a.no_demo and not a.only:
        from .out import scroll_demo
        cs = full_cast(a.angelx)
        order = [c for c in cs if c.name == "hero_red"] + [c for c in cs if c.name in ("brannoc", "goblin", "guard", "miner")]
        scroll_demo(order, pal, os.path.join(a.out, "side_scroll_demo.gif"))
    return rows


if __name__ == "__main__":
    main()
