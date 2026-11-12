"""Aster Vale on his tired horse (34 by 32, bottom-centre anchor, facing left;
the right-facing frames are the mirror). A rider in lantern-leathers with a
saddle bag of oil and a lantern pole; a dun horse with a dropped head, whose
near hind hoof is bare until he is shod. States: idle (a breath, a tail
swish), ride (a walking gait), lame (a limp, head low, the hind hoof held up)."""
from engine import *

W, H = 34, 32
HORSE = ["B", "P", "o", "O"]
HORSE_D = ["I", "B", "P", "o"]
LEATHER = ["s", "x", "S", "u"]
FOOT = 30


def leg(sp, x, top, bottom, ramp, hoof="k", lift=0, bend=0):
    y1 = bottom - lift
    sp.paint(rect(x, top, 2, y1 - top), ramp, rim=False, fall=False)
    if bend:
        sp.fill(rect(x + bend, y1 - 3, 2, 3), ramp[1])
    sp.fill(rect(x - (1 if bend else 0) + bend, y1, 3, 1), hoof)


def horse_and_rider(ph=0, gait="stand", lame=False, bare=True, head_drop=0, swish=0, breathe=0):
    sp = Sprite(W, H)
    # the far legs first, darker
    cyc = [(0, 1), (0, 0), (1, 0), (0, 0)][ph % 4] if gait != "stand" else (0, 0)
    far_f, far_h = (cyc[1], cyc[0]) if gait != "stand" else (0, 0)
    near_f, near_h = (cyc[0], cyc[1]) if gait != "stand" else (0, 0)
    lift_hind_near = 0
    bend_hind_near = 0
    if lame:
        # the bare hoof is held up and barely touches the road
        lift_hind_near = 3
        bend_hind_near = 0
    top = 19
    # far side
    leg(sp, 13, top, FOOT, HORSE_D, lift=far_f * 2)
    leg(sp, 25, top, FOOT, HORSE_D, lift=far_h * 2)
    # the tail, streaming back
    tail = [(28, 14), (30, 15), (31, 17 + swish), (31, 20 + swish), (30, 22 + swish)]
    for i in range(len(tail) - 1):
        sp.fill(cap(tail[i][0], tail[i][1], tail[i + 1][0], tail[i + 1][1], 1.3), "I" if i % 2 else "n")
    # the body
    sp.paint(ell(19, 17, 10.5, 4.6 + breathe * 0.4), HORSE, rim=True, fall=False)
    sp.paint(ell(19, 19.6, 9.0, 1.6), HORSE_D, rim=False, fall=False)
    # the neck and head, low and tired
    neck = poly([(11, 12), (14.5, 12.5), (10.5, 19), (6.5, 20)])
    sp.paint(neck, HORSE, rim=True, fall=False)
    hy = 19 + head_drop
    sp.paint(ell(5.2, hy, 3.4, 2.4), HORSE, rim=True, fall=False)
    sp.paint(poly([(2.5, hy - 1.5), (0.5, hy + 0.6), (1.8, hy + 2.6), (4.2, hy + 1.8)]), HORSE_D, rim=True, fall=False)
    sp.put(2, hy + 1, "k"); sp.put(4, hy - 1, "k")
    sp.put(6, hy - 3, "I"); sp.put(7, hy - 3, "I")
    # the mane along the neck
    for i in range(6):
        sp.put(13 - i * 1 + (i // 3), 12 + i, "I" if i % 2 else "n")
    # the near legs
    leg(sp, 11, top, FOOT, HORSE, lift=near_f * 2)
    leg(sp, 23, top, FOOT, HORSE, lift=near_h * 2 + lift_hind_near, hoof="k" if bare else "j")
    if lame:
        # the hoof hangs bare, a pale sole showing
        sp.put(24, FOOT - lift_hind_near + 1, "$")
    elif not bare:
        sp.put(23, FOOT, "j"); sp.put(24, FOOT, "j"); sp.put(25, FOOT, "j")
    # the saddle bag of oil, and the saddle
    sp.paint(rect(21, 13, 9, 3), ["b", "I", "B"], rim=True, fall=False)
    sp.paint(ell(25.5, 19.5, 2.6, 3.0), ["I", "R", "O"], rim=True, fall=False)
    sp.put(25, 19, "6")
    # the rider: leathers, a cap, a scarf; leaning a little, tired
    ry = 8
    sp.paint(ell(17, ry + 3, 3.4, 4.6), LEATHER, rim=True, fall=False)
    sp.paint(poly([(15.0, ry + 6), (19.5, ry + 6), (21.5, ry + 9), (14.5, ry + 9)]), LEATHER, rim=True, fall=False)
    sp.paint(ell(16, ry - 2, 2.9, 3.0), ["$", "c", "9"], rim=True, fall=False)
    sp.paint(ell(16.3, ry - 3.6, 3.3, 1.9), ["I", "B", "O"], rim=True, fall=False)
    sp.put(14, ry - 2, "k")
    sp.fill(rect(15, ry + 1, 4, 1), "2")
    # the lantern pole in his near hand, its lantern lit
    for y in range(1, ry + 5):
        sp.put(11, y, "P" if y % 3 else "o")
    sp.fill(rect(9, 3, 4, 5), "k")
    sp.fill(rect(10, 4, 2, 3), "6")
    sp.put(10, 4, "w")
    sp.fill(rect(11, ry + 3, 2, 2), "c")
    sp.paint(cap(16, ry + 3, 12, ry + 3.6, 1.0), LEATHER, rim=False, fall=False)
    return sp


def frames():
    out = {}
    # idle: a breath and a swish of the tail
    out["idle"] = [
        horse_and_rider(0, "stand", False, True, 1, 0, 0),
        horse_and_rider(0, "stand", False, True, 2, 1, 1),
    ]
    out["ride"] = [horse_and_rider(i, "walk", False, False, 0, [0, 1, 0, -1][i], 0) for i in range(4)]
    out["lame"] = [
        horse_and_rider(0, "walk", True, True, 3, 0, 0),
        horse_and_rider(2, "walk", True, True, 4, 1, 1),
    ]
    return out


if __name__ == "__main__":
    fr = frames()
    allf = []
    for k, v in fr.items():
        allf += [s.finish().rows() for s in v]
    sheet(allf, scale=5, cols=4, bg=(60, 58, 52)).save("/work/tmp/mf/aster.png")
