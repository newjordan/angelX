"""Lanternward's props, painted in the realm's inks: pots and grass, the
sign and the kingsman, the lever, plate, block and doors, the sigil, the
resting brazier and the ember bowl, the fissure, the pickups, the bolts, the
wisp and the lock-on spark."""
import math
from engine import *

WOOD = ["b", "I", "P", "o"]
CLAY = ["p", "B", "R", "o"]
STONE = ["K", "g", "j", "G"]
STONE_L = ["g", "j", "G", "J"]
IRON = ["K", "X", "g", "j"]
STEEL = ["g", "j", "G", "H"]
GRASS = ["f", "E", "m", "y"]


def flame(sp, cx, base, h, w, ph=0, cool=False):
    """A pixel flame: a teardrop that sways and licks, red-gold at the foot, a white core."""
    for dy in range(h):
        t = dy / max(1, h - 1)
        half = max(0, round((w / 2) * (1 - t ** 2.0) * (0.6 + 0.4 * min(1.0, t * 4))))
        sway = round(math.sin(ph + t * 3.2) * (0.4 + t * 1.8))
        lick = ((dy * 5 + int(ph * 2)) % 3) - 1 if 1 < dy < h - 1 and half > 0 else 0
        y = base - dy
        left, right = cx - half + sway - (lick > 0), cx + half + sway + (lick < 0)
        if dy == h - 1:
            left, right = cx + sway + 1, cx + sway + 1
        for x in range(left, right + 1):
            core = abs(x - cx - sway) <= max(0, half - 1)
            if cool:
                ink = "3" if t > 0.5 else ("2" if t > 0.15 else "1")
                if t < 0.4 and core:
                    ink = "w"
            else:
                ink = "4" if t < 0.2 else ("@" if t < 0.45 else ("6" if t < 0.75 else "5"))
                if t < 0.4 and core:
                    ink = "6" if t > 0.2 else "@"
                if t < 0.28 and abs(x - cx - sway) <= max(0, half - 2):
                    ink = "w"
            sp.put(x, y, ink)


def pot():
    out = {}
    s = Sprite(10, 11)
    s.paint(ell(5, 7.4, 4.4, 3.5), CLAY)
    s.paint(rect(3, 2, 4, 3) | ell(5, 3, 3, 1.5), CLAY)
    s.fill(rect(2, 1, 6, 1), "o"); s.fill(rect(2, 2, 6, 1), "R")
    s.fill(rect(3, 5, 4, 1), "B")
    s.put(3, 8, "t"); s.put(2, 7, "o")
    s.fill(rect(2, 10, 6, 1), "B")
    out["whole"] = [s.finish()]
    b = Sprite(10, 11)
    for (x, y, c) in [(1, 8, "R"), (2, 9, "B"), (3, 8, "o"), (2, 7, "R"), (6, 8, "R"), (7, 9, "B"), (8, 8, "R"), (5, 9, "p"), (4, 7, "B"),
                      (3, 9, "R"), (6, 7, "o"), (1, 9, "p"), (8, 9, "p"), (4, 10, "B"), (5, 10, "p"), (2, 10, "p"), (7, 10, "B")]:
        b.put(x, y, c)
    for (x, y) in [(3, 6), (7, 6), (4, 5)]:
        b.put(x, y, "B")
    out["broken"] = [b.finish()]
    return out


def grass():
    out = {}
    def tuft(shift):
        s = Sprite(16, 14)
        for i, bx in enumerate([2, 4, 6, 8, 10, 12, 13]):
            top = 2 + (i * 3) % 5
            lean = shift if i % 2 else -shift
            for y in range(top, 13):
                t = (y - top) / max(1, 12 - top)
                x = bx + round(lean * (1 - t))
                s.put(x, y, "y" if y == top else ("m" if i % 3 == 0 else "E" if i % 2 else "l"))
        s.fill(rect(1, 11, 14, 2), "E")
        for x in range(1, 15, 2):
            s.put(x, 11, "l")
        for x in range(2, 15, 3):
            s.put(x, 12, "D")
        return s
    out["tuft"] = [tuft(1), tuft(-1)]
    c = Sprite(16, 14)
    c.fill(rect(2, 11, 12, 2), "E")
    for x in range(2, 14, 2):
        c.put(x, 10 + (x // 2) % 2, "m")
        c.put(x, 11, "l")
    for x in range(3, 14, 3):
        c.put(x, 12, "D")
    out["cut"] = [c]
    return out


def sign():
    s = Sprite(10, 15)
    s.paint(rect(4, 7, 2, 8), WOOD)
    s.paint(rect(0, 1, 10, 7), WOOD)
    s.fill(rect(1, 2, 8, 1), "o")
    for y in (3, 5):
        for x in range(2, 8):
            if (x + y) % 3:
                s.put(x, y, "b")
    s.put(0, 1, "k"); s.put(9, 1, "k")
    s.put(1, 7, "I"); s.put(8, 7, "I")
    return {"idle": [s.finish()]}


def folk():
    """The kingsman: a steel helm, a red tabard with a gold crown, a staff."""
    out = {}
    def man(talk, arm):
        s = Sprite(12, 18)
        # boots and tabard
        s.paint(rect(3, 14, 2, 3) | rect(7, 14, 2, 3), IRON, rim=False)
        s.paint(poly([(2, 8), (10, 8), (10, 15), (2, 15)]), ["p", "7", "R", "O"])
        s.fill(rect(5, 9, 2, 1), "6"); s.fill(rect(4, 10, 4, 1), "6"); s.put(4, 9, "6"); s.put(7, 9, "6")
        s.fill(rect(2, 14, 8, 1), "B")
        # arms
        s.paint(cap(2, 9, 1, 13, 1.2), STEEL)
        if arm:
            s.paint(cap(10, 9, 11, 5, 1.2), STEEL)
            s.put(11, 4, "T")
        else:
            s.paint(cap(10, 9, 10, 13, 1.2), STEEL)
        # head: helm with a nasal guard and a plume
        s.paint(ell(6, 5, 3.6, 3.8), STEEL)
        s.fill(rect(5, 5, 2, 4), "T")
        s.fill(rect(4, 4, 4, 1), "k")
        s.put(5, 5, "k"); s.put(6, 5, "k")
        if talk:
            s.put(5, 7, "k"); s.put(6, 7, "k")
        s.fill(rect(4, 7, 4, 1) if not talk else rect(4, 8, 4, 1), "j")
        s.fill(rect(5, 0, 2, 2), "7"); s.put(6, 0, "R")
        return s.finish()
    out["idle"] = [man(False, False), man(False, False)]
    out["talk"] = [man(True, True), man(False, False)]
    # the second idle frame breathes
    b = Sprite(12, 18)
    for (x, y), c in out["idle"][0].px.items():
        b.px[(x, y + (1 if y < 14 else 0))] = c
    for x in range(3, 5):
        for y in (14, 15, 16):
            b.px.setdefault((x, y), "K")
    out["idle"][1] = b
    return out


def lever():
    out = {}
    def make(pulled):
        s = Sprite(12, 14)
        s.paint(rect(1, 9, 10, 5), STONE)
        s.paint(rect(3, 7, 6, 3), STONE_L)
        s.fill(rect(4, 10, 4, 1), "k")  # the slot
        if pulled:
            s.paint(cap(6, 9, 10, 5, 1.0), IRON)
            s.paint(ell(10.5, 4.5, 1.8, 1.8), ["a", "R", "@", "6"])
        else:
            s.paint(cap(6, 9, 3, 2, 1.0), IRON)
            s.paint(ell(2.8, 2, 1.8, 1.8), ["7", "R", "@", "6"])
        return s.finish()
    out["up"] = [make(False)]
    out["pulled"] = [make(True)]
    return out


def plate():
    out = {}
    def make(down):
        s = Sprite(16, 16)
        s.fill(rect(1, 1, 14, 14), "K")
        if down:
            s.paint(rect(2, 3, 12, 11), ["K", "X", "g", "j"], rim=False, fall=False)
            s.fill(rect(2, 3, 12, 1), "K")
            s.fill(ell(8, 8.5, 3, 3), "a")
            s.fill(ell(8, 8.5, 1.5, 1.5), "R")
        else:
            s.paint(rect(2, 1, 12, 12), ["g", "j", "G", "J"])
            s.fill(rect(2, 13, 12, 2), "K")
            s.fill(rect(2, 13, 12, 1), "X")
            s.fill(ell(8, 7, 3, 3), "R")
            s.fill(ell(8, 7, 2, 2), "@")
            s.put(8, 7, "w")
        return s
    out["up"] = [make(False)]
    out["down"] = [make(True)]
    return out


def block():
    s = Sprite(16, 16)
    s.paint(rect(1, 4, 14, 11), STONE_L)
    s.paint(poly([(1, 4), (3, 1), (13, 1), (15, 4)]), ["j", "G", "J", "H"])
    s.fill(rect(2, 5, 12, 1), "J")
    s.fill(rect(1, 14, 14, 1), "K")
    for (x, y) in [(3, 8), (4, 8), (10, 10), (11, 10), (7, 12)]:
        s.put(x, y, "g")
    s.fill(rect(5, 7, 6, 1), "g")
    s.fill(rect(7, 6, 1, 6), "g")  # a carved cross seam
    s.fill(rect(1, 4, 1, 10), "G")
    return {"idle": [s.finish()]}


def cracked():
    out = {}
    s = Sprite(16, 16)
    s.paint(rect(0, 0, 16, 16), ["g", "j", "G", "J"], rim=False, fall=False)
    for row in (5, 10):
        s.fill(rect(0, row, 16, 1), "g")
    for (x, y) in [(4, 0), (10, 5), (3, 10)]:
        for k in range(5):
            s.put(x, y + k, "g")
    # the crack, with a faint light in it
    for (x, y) in [(7, 1), (7, 2), (8, 3), (8, 4), (7, 5), (6, 6), (7, 7), (8, 8), (9, 9), (9, 10), (8, 11), (8, 12), (9, 13)]:
        s.put(x, y, "k")
    for (x, y) in [(8, 2), (9, 5), (7, 9)]:
        s.put(x, y, "a")
    s.fill(rect(0, 15, 16, 1), "K")
    out["intact"] = [s]
    b = Sprite(16, 16)
    b.fill(rect(0, 3, 16, 12), "k")
    b.fill(rect(2, 4, 12, 10), "K")
    for (x, y, c) in [(1, 12, "G"), (2, 11, "J"), (3, 13, "j"), (5, 14, "G"), (7, 13, "J"), (9, 14, "j"), (11, 13, "G"), (13, 12, "J"), (14, 14, "j"),
                      (4, 12, "g"), (8, 12, "G"), (12, 14, "g"), (1, 14, "g"), (6, 12, "j"), (10, 12, "g")]:
        b.fill(rect(x, y, 2, 1), c)
    for x in (0, 1, 14, 15):
        b.fill(rect(x, 0, 1, 6), "g")
    b.fill(rect(0, 0, 16, 3), "j")
    b.fill(rect(0, 3, 16, 1), "g")
    out["blown"] = [b]
    return out


def doorframe(s, ring, chains=False, open_=False):
    # a stone arch round an oak door, one tile
    s.paint(rect(0, 0, 16, 16), STONE, rim=False, fall=False)
    s.fill(rect(0, 0, 16, 2), "j")
    s.fill(rect(0, 0, 1, 16), "G"); s.fill(rect(15, 0, 1, 16), "K")
    if open_:
        s.fill(rect(3, 3, 10, 13), "k")
        for y in range(4, 16, 3):
            s.fill(rect(4, y, 8, 1), "K")
        s.fill(rect(0, 3, 3, 13), "j"); s.fill(rect(13, 3, 3, 13), "g")
        return s
    s.fill(poly([(3, 15), (3, 5), (5, 3), (11, 3), (13, 5), (13, 15)]), "k")
    s.paint(poly([(4, 15), (4, 6), (6, 4), (10, 4), (12, 6), (12, 15)]), WOOD)
    for x in (6, 9):
        s.fill(rect(x, 5, 1, 10), "b")
    s.fill(rect(4, 8, 8, 1), "X"); s.fill(rect(4, 12, 8, 1), "X")
    s.put(5, 8, "j"); s.put(10, 8, "j"); s.put(5, 12, "j"); s.put(10, 12, "j")
    if ring:
        s.paint(ell(8, 9, 2.4, 2.4), ring)
        s.fill(ell(8, 9, 1.2, 1.2), "k")
    if chains:
        for k in range(0, 16):
            s.put(2 + k // 1, 3 + k * 12 // 16, "h" if k % 2 else "J")
            s.put(14 - k, 3 + k * 12 // 16 + 1, "h" if k % 2 else "J")
        s.fill(rect(6, 7, 4, 4), "R")
        s.fill(rect(7, 8, 2, 2), "k")
    return s


def doors():
    out = {}
    s = Sprite(16, 16); doorframe(s, ["$", "c", "c", "9"]); out["lock"] = {"locked": [s], "open": [doorframe(Sprite(16, 16), None, open_=True)]}
    b = Sprite(16, 16); doorframe(b, ["p", "7", "R", "O"], chains=True)
    out["boss-door"] = {"chained": [b], "open": [doorframe(Sprite(16, 16), None, open_=True)]}
    return out


def sigil():
    out = {}
    out["hidden"] = [Sprite(16, 16)]
    def rune(k, bright):
        s = Sprite(16, 16)
        r = 2 + k * 2
        for a in range(0, 360, 12):
            if (a // 12) % 3 == 0 and not bright:
                continue
            x = 8 + math.cos(math.radians(a)) * r
            y = 8 + math.sin(math.radians(a)) * r * 0.8
            s.put(int(round(x)), int(round(y)), "2" if bright else "0")
        if k >= 1:
            s.fill(line(8 - k, 8, 8 + k, 8), "3" if bright else "1")
            s.fill(line(8, 8 - k, 8, 8 + k), "3" if bright else "1")
        if k >= 2:
            for (dx, dy) in ((-3, -3), (3, -3), (-3, 3), (3, 3)):
                s.put(8 + dx, 8 + dy, "3")
        return s
    out["revealing"] = [rune(1, False), rune(2, False), rune(3, True)]
    f1 = rune(3, True)
    f1.fill(ell(8, 8, 1.3, 1.3), "w")
    f2 = rune(3, True)
    for (x, y) in [(2, 8), (14, 8), (8, 2), (8, 14)]:
        f2.put(x, y, "w")
    out["found"] = [f1, f2]
    return out


def brazier_rest():
    out = {}
    frames = []
    for i in range(4):
        s = Sprite(14, 20)
        s.paint(poly([(3, 19), (4, 13), (10, 13), (11, 19)]), STONE)
        s.paint(ell(7, 12, 6.0, 2.4), STONE_L)
        s.fill(ell(7, 11.5, 4.8, 1.5), "k")
        s.fill(rect(2, 19, 10, 1), "K")
        flame(s, 7, 12, 9 + (i % 2), 5, ph=i * 1.6, cool=True)
        s.put(7 + (i - 1), 3 - i % 2, "3")
        s.finish()
        frames.append(s)
    out["lit"] = frames
    return out


def ember():
    out = {}
    frames = []
    for i in range(4):
        s = Sprite(14, 12)
        s.paint(ell(7, 8, 6.2, 3.8), IRON)
        s.fill(ell(7, 6, 5.0, 2.4), "k")
        s.fill(ell(7, 6, 4.2, 1.8), ["a", "R", "O", "R"][i])
        for (x, y) in [(5, 6), (8, 6), (7, 5), (9, 7)]:
            s.put(x + (i % 2), y, ["@", "6", "4", "6"][(x + i) % 4])
        flame(s, 7, 5, 4 + i % 2, 3, ph=i * 1.7)
        s.fill(rect(4, 11, 6, 1), "K")
        out.setdefault("lit", []).append(s.finish())
    return out


def fissure():
    out = {}
    s = Sprite(14, 10)
    for (x, y) in [(1, 5), (2, 5), (3, 4), (4, 4), (5, 5), (6, 5), (7, 4), (8, 5), (9, 5), (10, 4), (11, 5), (12, 5), (5, 6), (8, 6)]:
        s.put(x, y, "k")
    for (x, y) in [(3, 5), (7, 5), (10, 5)]:
        s.put(x, y, "K")
    out["shut"] = [s]
    def opened(i):
        o = Sprite(14, 10)
        o.fill(ell(7, 5, 6, 3.4), "k")
        o.fill(ell(7, 5, 5, 2.6), "(")
        o.fill(ell(7, 5, 3.6, 1.8), ")")
        o.fill(ell(7, 5, 2, 1), "[")
        for k in range(3):
            o.put(5 + k * 2, 3 - ((i + k) % 2), "S" if k % 2 else "u")
        o.put(7, 1 + i, "U")
        return o
    out["open"] = [opened(0), opened(1)]
    return out


def pickups():
    out = {}
    def heart(i):
        s = Sprite(7, 7)
        rows = [".XX.XX.", "XXXXXXX", "XXXXXXX", ".XXXXX.", "..XXX..", "...X..."]
        for y, r in enumerate(rows):
            for x, c in enumerate(r):
                if c == "X":
                    s.put(x, y, "7")
        s.put(1, 1, "w" if i == 0 else "c"); s.put(1, 2, "c")
        for (x, y) in [(5, 3), (4, 4), (3, 5), (5, 2), (6, 2)]:
            s.put(x, y, "R" if (x + y) % 2 else "p")
        s.put(3, 6, "p")
        return s.finish()
    out["heart"] = {"idle": [heart(0), heart(1)]}
    def gem(i):
        s = Sprite(6, 7)
        s.fill(rect(2, 0, 2, 1), "6"); s.fill(rect(1, 1, 4, 1), "6"); s.fill(rect(0, 2, 6, 2), "4")
        s.fill(rect(1, 4, 4, 1), "@"); s.fill(rect(2, 5, 2, 1), "a")
        s.put(1, 2, "w"); s.put(2, 1, "5")
        s.put(4, 3, "a"); s.put(3, 4, "a")
        if i == 1:
            s.put(4, 1, "w"); s.put(0, 2, "6")
        if i == 2:
            s.put(1, 3, "w"); s.put(5, 2, "6")
        return s
    out["gem"] = {"idle": [gem(0), gem(1), gem(2)]}
    def bomb(fuse):
        s = Sprite(8, 9)
        s.paint(ell(4, 5.6, 3.6, 3.4), ["k", "K", "X", "g"])
        s.put(2, 4, "j"); s.put(3, 3, "G")
        s.fill(rect(3, 1, 3, 1), "j"); s.fill(rect(3, 2, 1, 1), "j")
        s.put(5, 0, "o"); s.put(4, 0, "P")
        if fuse:
            s.put(6, 0, "w" if fuse == 1 else "6"); s.put(6, 1, "@"); s.put(5, 0, "6" if fuse == 1 else "4")
        return s
    out["bomb"] = {"idle": [bomb(0).finish()], "fuse": [bomb(1).finish(), bomb(2).finish()]}
    def cont(i):
        s = Sprite(12, 12)
        s.fill(ring(6, 6, 5.6, 4.6), "6" if i == 0 else "@")
        rows = [".XX..XX.", "XXXXXXXX", "XXXXXXXX", ".XXXXXX.", "..XXXX..", "...XX..."]
        for y, r in enumerate(rows):
            for x, c in enumerate(r):
                if c == "X":
                    s.put(2 + x, 3 + y, "7")
        s.put(3, 4, "w"); s.put(3, 5, "c")
        for (x, y) in [(7, 6), (6, 7), (5, 8)]:
            s.put(x, y, "R")
        s.put(0 if i else 11, 6, "w"); s.put(6, 0 if i else 11, "6")
        return s
    out["container"] = {"idle": [cont(0).finish() if False else cont(0), cont(1)]}
    return out


def ring(cx, cy, ro, ri):
    return ell(cx, cy, ro, ro) - ell(cx, cy, ri, ri)


def bolts():
    out = {}
    def mk(draw):
        s = Sprite(6, 6)
        draw(s)
        return s
    def iron(s):
        s.fill(ell(3, 3, 2.6, 2.6), "k"); s.fill(ell(3, 3, 1.8, 1.8), "O"); s.put(3, 3, "w"); s.put(2, 2, "t")
    def light(i):
        def f(s):
            s.fill(ell(3, 3, 2.6, 2.6), "5"); s.fill(ell(3, 3, 1.6, 1.6), "w")
            if i:
                s.put(0, 0, "4"); s.put(5, 5, "4")
            else:
                s.put(5, 0, "4"); s.put(0, 5, "4")
        return f
    def bright(i):
        def f(s):
            s.fill(ell(3, 3, 3.0, 3.0), "w"); s.fill(ell(3, 3, 1.6, 1.6), "6")
            if i:
                s.put(0, 3, "5"); s.put(5, 3, "5")
            else:
                s.put(3, 0, "5"); s.put(3, 5, "5")
        return f
    def dim(s):
        s.fill(ell(3, 3, 2.2, 2.2), "h"); s.fill(ell(3, 3, 1.2, 1.2), "H")
    out["iron"] = [mk(iron)]
    out["light"] = [mk(light(0)), mk(light(1))]
    out["bright"] = [mk(bright(0)), mk(bright(1))]
    out["dim"] = [mk(dim)]
    return out


def wisp():
    out = []
    for i in range(3):
        s = Sprite(8, 10)
        s.fill(ell(4, 6, 2.6, 3), "5")
        s.fill(ell(4, 6.4, 1.6, 2), "w")
        # the pointed head and the tail
        s.fill(poly([(3, 4), (4, 0 + (i % 2)), (5, 4)]), "6")
        s.put(4, 2 + (i % 2), "w")
        for k in range(3):
            s.put(4 + (1 if (i + k) % 2 else -1) * (k > 0), 8 + (k > 1), "4" if k < 2 else "a")
        out.append(s)
    return {"idle": out}


def mark():
    out = []
    for i in range(2):
        s = Sprite(10, 10)
        # a small gold diamond-spark, a point of wick-light
        for (x, y, c) in [(5, 1, "6"), (4, 2, "6"), (5, 2, "w"), (6, 2, "6"), (3, 3, "5"), (4, 3, "w"), (5, 3, "w"), (6, 3, "w"), (7, 3, "5"),
                          (4, 4, "6"), (5, 4, "w"), (6, 4, "6"), (5, 5, "6")]:
            s.put(x, y + i, c)
        s.put(5, 0 + i, "4") if i == 0 else s.put(5, 6, "4")
        s.put(2, 3 + i, "4"); s.put(8, 3 + i, "4")
        out.append(s)
    return {"idle": out}


def all_props():
    """{slot id: {state: [Sprite]}}"""
    out = {"pot": pot(), "grass": grass(), "sign": sign(), "folk": folk(), "lever": lever(), "plate": plate(),
           "block": block(), "cracked": cracked(), "sigil": sigil(), "rest": brazier_rest(), "ember": ember(),
           "fissure": fissure(), "bolt": bolts(), "wisp": wisp(), "mark": mark()}
    out.update(doors())
    out.update(pickups())
    return out


if __name__ == "__main__":
    allf = []
    names = []
    for k, st in all_props().items():
        for s, fr in st.items():
            for f in fr:
                allf.append(f.rows())
                names.append(f"{k}.{s}")
    # pad frames to a common cell
    cw = max(len(r[0]) for r in allf)
    ch = max(len(r) for r in allf)
    padded = [[row.ljust(cw, ".") for row in f] + ["." * cw] * (ch - len(f)) for f in allf]
    sheet(padded, scale=7, cols=14).save("/work/tmp/lw/props.png")
    print(len(allf), cw, ch)
