"""The turret: a squat riveted iron drum on a stone plinth, one eye in its
lid that warms to amber through the tell (22 by 24, bottom-centre anchor).
The stage lays the barrel over the lid toward the footprint."""
from engine import *

W, H = 22, 24
IRON = ["K", "X", "g", "j"]
IRON_T = ["X", "g", "j", "G"]
STONE = ["K", "g", "j", "G"]


def turret(eye=("a", "R"), core=None, rings=0, shake=0, stun=False, flash=False):
    sp = Sprite(W, H)
    cx = 11 + shake
    # the plinth: a low stone step
    sp.paint(poly([(1, 23), (3, 18), (19, 18), (21, 23)]), STONE)
    for x in range(4, 19, 4):
        sp.put(x, 21, "K")
    sp.fill(rect(2, 22, 18, 1), "K")
    # the drum's side, banded
    side = rect(3 + shake, 10, 16, 9) | ell(cx, 18, 8, 3.0)
    sp.paint(side, IRON, fall=True)
    for y in (13, 16):
        sp.fill(rect(3 + shake, y, 16, 1), "K")
    for x in (6, 11, 16):
        sp.put(x + shake, 14, "J")
        sp.put(x + shake, 17, "J")
    # the lid: an iron ellipse
    sp.paint(ell(cx, 10, 9.0, 6.2), IRON_T)
    sp.paint(ell(cx, 10, 7.0, 4.6), ["X", "g", "j", "G"], rim=True)
    # rivets round the lid
    for k in range(8):
        a = k * 3.14159 / 4
        import math
        sp.put(int(round(cx + 8.2 * math.cos(a))), int(round(10 + 5.6 * math.sin(a))), "J")
    # the eye
    if stun:
        sp.fill(ell(cx, 10, 3.3, 2.7), "k")
        sp.fill(ell(cx, 10, 2.3, 1.9), "u")
        sp.put(cx, 10, "3")
        sp.put(cx - 4, 7, "3"); sp.put(cx + 4, 8, "w"); sp.put(cx + 2, 6, "3")
    else:
        sp.fill(ell(cx, 10, 3.6, 3.0), "k")
        sp.fill(ell(cx, 10, 2.6, 2.1), eye[0])
        sp.fill(ell(cx, 10, 1.5, 1.1), eye[1])
        if core:
            sp.put(cx, 10, core)
        if flash:
            sp.fill(ell(cx, 10, 4.2, 3.4), "6")
            sp.fill(ell(cx, 10, 2.8, 2.2), "w")
    return sp.finish()


def frames():
    T = turret
    out = {}
    out["idle"] = [T(eye=("a", "R")), T(eye=("a", "a"))]
    warm = [("a", "a"), ("a", "R"), ("R", "R"), ("R", "O"), ("O", "@"), ("@", "6"), ("6", "w")]
    out["tell"] = [T(eye=w, core=("w" if i == 6 else None), shake=(i % 2 if i >= 5 else 0)) for i, w in enumerate(warm)]
    out["fire"] = [T(eye=("6", "w"), flash=True), T(eye=("@", "6"), flash=True, shake=1)]
    out["recover"] = [T(eye=("R", "R")), T(eye=("a", "R")), T(eye=("a", "a")), T(eye=("a", "a"))]
    out["stun"] = [T(stun=True), T(stun=True, shake=1)]
    return out


if __name__ == "__main__":
    allf = []
    for v in frames().values():
        allf += [s.rows() for s in v]
    sheet(allf, scale=8, cols=9).save("/work/tmp/lw/turret.png")
