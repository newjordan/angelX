"""The snuffer: a hooded wisp of cold smoke over frayed cloth, two pale eyes,
a wick-stub on the end of its tail that burns when it is awake (18 by 20,
bottom-centre anchor)."""
import math
from engine import *

W, H = 18, 20
SMOKE = ["x", "S", "u", "U"]
SMOKE_L = ["x", "S", "u", "U"]
CLOAK = ["d", "(", ")", "["]


def snuffer(sq=0, lean=0, hem=0, tail=0.0, eyes="3", wick="6", lids=0, hooded=False, stretch=0, wide=0, eyeshape=0, drop=0):
    """sq squashes the body down, stretch pulls it up, lean slides the head."""
    sp = Sprite(W, H)
    foot = 18 + drop
    top = 1 + sq - stretch
    hx = 8 + lean
    # the tail first, curling out behind: a thin smoke rope to a wick
    pts = []
    for i in range(9):
        t = i / 8
        px = 12 + 5 * t + lean * (1 - t) * 0.0
        py = foot - 1 - 8 * t + 2.2 * math.sin(t * 5.5 + tail)
        pts.append((px, py))
    for i in range(len(pts) - 1):
        sp.fill(cap(pts[i][0], pts[i][1], pts[i + 1][0], pts[i + 1][1], 1.15 - i * 0.05), "x" if i % 2 else "S")
    # the body: a teardrop, point up
    body = set()
    for y in range(top, foot + 1):
        t = (y - top) / max(1, foot - top)
        rx = 1.0 + (5.6 + wide) * (t ** 0.62)
        cx = hx + lean * (1 - t) * 1.0
        for x in range(int(cx - rx - 1), int(cx + rx + 2)):
            if abs(x + 0.5 - cx) <= rx:
                body.add((x, y))
    # a frayed hem
    for x in range(0, W):
        if (x, foot) in body and ((x * 7 + hem * 3) % 5 < 2):
            body.discard((x, foot))
            if (x * 3 + hem) % 3 == 0:
                body.discard((x, foot - 1))
    cloth = CLOAK if hooded else SMOKE
    sp.paint(body, cloth, rim=True)
    # a fold line down the hood
    for y in range(top + 3, foot - 2):
        if (hx - 3, y) in sp.px and y % 2 == 0:
            sp.px[(hx - 3, y)] = cloth[0]
    # the dark of the hood and the eyes in it
    ey = top + 8 - sq // 2
    face = ell(hx + lean * 0.3, ey + 1.5, 3.9, 3.4)
    sp.fill(face, "k")
    if hooded:
        # a cloak over the whole head, stitched, the eyes shut away
        sp.fill(ell(hx, ey + 1.5, 4.6, 4.4), ")")
        sp.fill(ell(hx, ey + 1.5, 3.4, 3.2), "(")
        sp.put(hx - 2, ey + 1, "d"); sp.put(hx + 2, ey + 1, "d")
        for dx in (-3, 0, 3):
            sp.put(hx + dx, ey + 4, "[")
        sp.put(hx, ey - 2, "]")
    else:
        lx, rx_ = hx - 2, hx + 2
        if eyeshape == 0:
            sp.put(lx, ey + 1, eyes); sp.put(rx_, ey + 1, eyes)
            sp.put(lx, ey + 2, eyes); sp.put(rx_, ey + 2, eyes)
        elif eyeshape == 1:   # narrowed, lit
            sp.fill(rect(lx - 1, ey + 2, 2, 1), eyes); sp.fill(rect(rx_, ey + 2, 2, 1), eyes)
        elif eyeshape == 2:   # wide, blazing
            sp.fill(rect(lx - 1, ey + 1, 2, 3), eyes); sp.fill(rect(rx_, ey + 1, 2, 3), eyes)
            sp.put(lx, ey + 2, "w"); sp.put(rx_, ey + 2, "w")
        elif eyeshape == 3:   # dazed: crosses
            for (cx_, cy_) in ((lx, ey + 2), (rx_, ey + 2)):
                sp.put(cx_ - 1, cy_ - 1, "7"); sp.put(cx_ + 1, cy_ - 1, "7")
                sp.put(cx_, cy_, "7")
                sp.put(cx_ - 1, cy_ + 1, "7"); sp.put(cx_ + 1, cy_ + 1, "7")
        elif eyeshape == 4:   # shut: flat lines
            sp.fill(rect(lx - 1, ey + 2, 2, 1), "u"); sp.fill(rect(rx_, ey + 2, 2, 1), "u")
    # the wick on the end of the tail
    wx, wy = pts[-1]
    wx, wy = int(round(wx)), int(round(wy))
    if wick:
        sp.put(wx, wy, wick)
        sp.put(wx, wy - 1, "w" if wick in "6@" else wick)
        if wick in "6@":
            sp.put(wx, wy - 2, "@")
            sp.put(wx + 1, wy - 1, "4")
    else:
        sp.put(wx, wy, "a")
    return sp.finish()


def frames():
    S = snuffer
    out = {}
    out["idle"] = [S(tail=0.0, hem=0), S(tail=1.4, hem=2, sq=0, drop=0)]
    out["drift"] = [S(lean=-1, tail=0.0, hem=0), S(lean=0, tail=1.5, hem=1), S(lean=1, tail=3.0, hem=2), S(lean=0, tail=4.5, hem=3)]
    out["tell"] = [
        S(sq=1, wide=0.6, eyeshape=1, eyes="5", tail=0.5),
        S(sq=2, wide=1.0, eyeshape=1, eyes="6", tail=1.5, wick="@"),
        S(sq=3, wide=1.4, eyeshape=2, eyes="6", tail=2.5, wick="6"),
        S(sq=3, wide=1.4, eyeshape=2, eyes="w", tail=3.5, wick="w", lean=1),
    ]
    out["lunge"] = [S(stretch=1, wide=-1.2, eyeshape=2, eyes="w", lean=-1, tail=0.8, wick="w"), S(stretch=1, wide=-1.5, eyeshape=2, eyes="6", lean=-2, tail=2.0, wick="6")]
    out["recover"] = [S(sq=2, wide=1.2, eyeshape=4, tail=0.2, wick="4"), S(sq=1, wide=0.6, eyeshape=0, tail=0.9, wick="5")]
    out["hit"] = [S(sq=2, lean=1, eyeshape=4, tail=2.5, wick="w"), S(sq=1, lean=-1, eyeshape=1, eyes="w", tail=3.0, wick="5")]
    out["stagger"] = [S(sq=2, lean=-1, eyeshape=3, tail=0.4, wick="4"), S(sq=2, lean=1, eyeshape=3, tail=1.8, wick="@")]
    out["hooded"] = [S(hooded=True, tail=0.0, wick=None), S(hooded=True, tail=1.4, hem=2, wick=None)]
    # die: it unravels upward into smoke and a last spark of light
    die = []
    base = S(sq=1, eyeshape=4, wick="w")
    for i in range(7):
        k = i / 6
        sp = Sprite(W, H)
        for (x, y), c in base.px.items():
            ny = y + int(k * 5)
            h = (x * 29 + y * 71 + i * 13) % 100
            if h < k * 95:
                # rises as smoke
                if h % 3 == 0 and ny - int(k * 12) >= 0:
                    sp.px[(x, ny - int(k * 12))] = "S" if h % 2 else "u"
                continue
            sp.px[(x, ny)] = c
        if i >= 3:
            sp.put(8, 17 - (i - 3), "6"); sp.put(9, 16 - (i - 3), "w")
        if i == 6:
            sp.put(8, 14, "4"); sp.put(7, 15, "@")
        die.append(sp)
    out["die"] = die
    return out


if __name__ == "__main__":
    fr = frames()
    allf = []
    for k, v in fr.items():
        allf += [s.rows() for s in v]
    sheet(allf, scale=8, cols=9).save("/work/tmp/lw/snuffer.png")
