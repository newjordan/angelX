"""Moonfall's five villagers on one rig (`rig.py`). Each is a `Cfg` plus the
hooks that draw their one signature thing (ART.md's identity table): Mira's
plumed sallet, scarf, book and taper; Tovin's iron cone, reed plume, lantern
pole and key ring; Nessa's round sallet, floured apron, tray of loaves and
board; Orin's stoop, inked fingers and two quills; Pella's star, horn and
scroll. Frames are 18 by 28, bottom-centre anchor."""
from rig import *

STEEL = ["g", "J", "i", "H"]
STEEL_D = ["K", "g", "J", "i"]
IRON = ["K", "g", "j", "G"]


def blob(sp, cx, cy, rx, ry, ramp, **kw):
    sp.paint(ell(cx, cy, rx, ry), ramp, **kw)


# ---- Mira -------------------------------------------------------------------

def mira_hat(sp, c, d, kind, ph, bob, x):
    hy = c.head_y + bob
    if d == "left":
        hx = CX - 0.5
        sp.paint(ell(hx + 0.6, hy - 1.5, 3.7, 3.0), STEEL, rim=True)
        # the sallet's tail flares back
        sp.paint(poly([(hx + 2.5, hy - 1.5), (hx + 6.0, hy + 2.2), (hx + 3.0, hy + 3.2), (hx + 1.5, hy + 0.5)]), STEEL_D, rim=True, fall=False)
        sp.fill(rect(int(hx - 3), int(hy - 0.6), 3, 1), "g")      # brim shadow
        # plume, streaming back
        for i, (dx, dy) in enumerate([(1, -5), (2, -6), (3, -6), (4, -5), (5, -4)]):
            px(sp, hx + dx, hy + dy, ["W", "H", "W", "V", "V"][i])
        px(sp, hx + 2, hy - 5, "H")
        # the quill behind the plume
        for i in range(4):
            px(sp, hx + 1 + i, hy - 7 + i // 2, "W")
        px(sp, hx + 5, hy - 5, "k")
    elif d == "up":
        sp.paint(ell(CX, hy - 0.8, 3.9, 3.8), STEEL, rim=True)
        sp.paint(poly([(CX - 3.2, hy + 1.5), (CX + 3.2, hy + 1.5), (CX + 4.0, hy + 4.2), (CX - 4.0, hy + 4.2)]), STEEL_D, rim=True, fall=False)
        for dx, dy, ch in [(0, -5, "H"), (1, -6, "W"), (2, -6, "W"), (2, -5, "V"), (-1, -5, "V")]:
            px(sp, CX + dx, hy + dy, ch)
        sp.put(int(CX) + 2, int(hy) - 8, "W")
    else:
        sp.paint(ell(CX, hy - 2.2, 4.0, 2.7), STEEL, rim=True)
        sp.fill(rect(int(CX - 4), int(hy - 0.5), 8, 1), "g")      # brim
        sp.fill(rect(int(CX - 3), int(hy - 0.5), 1, 1), "J")
        # the plume stands up and bows right
        for dx, dy, ch in [(0, -5, "H"), (1, -6, "W"), (2, -6, "W"), (3, -5, "V"), (0, -6, "V")]:
            px(sp, CX + dx, hy + dy, ch)
        px(sp, CX + 3, hy - 4, "V")
        # the quill behind it, a long white line with a dark nib
        for i in range(4):
            px(sp, CX + 1 + i, hy - 7 + (1 if i > 1 else 0), "W")
        px(sp, CX + 5, hy - 6, "k")


def mira_back(sp, c, d, kind, ph, bob, x):
    # the long scarf tail behind
    if d == "up":
        top = c.top + bob
        sp.paint(rect(int(CX) - 1, top + 1, 3, 9), ["0", "2", "3"], rim=True, fall=False)
    elif d == "left":
        top = c.top + bob
        sp.paint(poly([(CX + 2, top), (CX + 4.5, top + 1), (CX + 5, top + 8), (CX + 3, top + 7)]), ["0", "2", "3"], rim=False, fall=False)


def mira_front(sp, c, d, kind, ph, bob, x, hands):
    top = c.top + bob
    flame = ["6", "@", "6", "4"][(x.get("t", 0)) % 4]
    if d == "down":
        # scarf round the neck, tail on her left
        sp.paint(rect(int(CX) - 3, top, 7, 2), ["0", "2", "3"], rim=True, fall=False)
        sp.paint(poly([(CX - 3, top + 1), (CX - 1, top + 1), (CX - 1.5, top + 8), (CX - 3.5, top + 7)]), ["0", "2", "3"], rim=False, fall=False)
        # the moon crest on the tabard: a small crescent
        for dx, dy in [(0, 5), (1, 4), (2, 4), (0, 6), (1, 7), (2, 7)]:
            px(sp, CX + 1 + dx - 1, top + dy, "6")
        # the book under her right arm (left of the picture)
        sp.paint(rect(int(CX) - 7, top + 5, 3, 5), ["I", "B", "R"], rim=True, fall=False)
        px(sp, CX - 6, top + 7, "6")
        # the taper in her free hand
        hx, hy = hands[1]
        sp.fill(rect(int(hx), int(hy) - 3, 1, 3), "T")
        px(sp, hx, hy - 4, flame)
        px(sp, hx, hy - 5, "4" if flame != "6" else "w")
    elif d == "up":
        sp.paint(rect(int(CX) - 3, top, 7, 2), ["0", "2", "3"], rim=True, fall=False)
        sp.paint(rect(int(CX) - 7, top + 5, 3, 5), ["I", "B", "R"], rim=True, fall=False)
        hx, hy = hands[1]
        sp.fill(rect(int(hx), int(hy) - 3, 1, 3), "T")
        px(sp, hx, hy - 4, flame)
    else:
        hx, hy = hands["near"]
        sp.paint(rect(int(CX) - 2, top, 5, 2), ["0", "2", "3"], rim=True, fall=False)
        sp.paint(rect(int(CX) - 1, top + 4, 3, 5), ["I", "B", "R"], rim=True, fall=False)
        sp.fill(rect(int(hx) - 1, int(hy) - 3, 1, 3), "T")
        px(sp, hx - 1, hy - 4, flame)
        px(sp, hx - 1, hy - 5, "4" if flame != "6" else "w")


MIRA = Cfg(
    skin=["$", "c", "9"], hair=["b", "I", "p"], head_y=9.6, eye_dy=1.3,
    tunic=["!", "?", ";"], sleeve=["x", "S", "u"], belt=["I", "O"],
    legs=["Z", "n", "I"], boot=["k", "b", "I"],
    sh=3.6, hm=3.8, side_w=2.8, hat=mira_hat, back=mira_back, front=mira_front,
)


# ---- Tovin ------------------------------------------------------------------

CLOAK = ["F", "E", "l", "m"]


def tovin_hat(sp, c, d, kind, ph, bob, x):
    hy = c.head_y + bob
    t = x.get("t", 0)
    if d == "left":
        hx = CX - 0.5
        sp.paint(poly([(hx - 1.0, hy - 9.5), (hx - 6.0, hy - 1.2), (hx + 6.0, hy - 1.2), (hx + 2.0, hy - 7.5)]), IRON, rim=True, fall=False)
        sp.fill(rect(int(hx - 6), int(hy - 1.5), 12, 1), "g")
        # the reed plume leaning out of the point
        for i, ch in enumerate(["M", "y", "y"]):
            px(sp, hx + 1 + i, hy - 9 - (i == 1), ch)
        px(sp, hx + 3, hy - 10, "T")
    else:
        sp.paint(poly([(CX, hy - 9.8), (CX - 6.6, hy - 1.2), (CX + 6.6, hy - 1.2)]), IRON, rim=True, fall=False)
        sp.fill(rect(int(CX - 6), int(hy - 1.5), 13, 1), "g")
        for dx in (-5, -2, 2, 5):
            px(sp, CX + dx, hy - 2.6, "j") if abs(dx) < 3 else None
        # a reed plume tucked in the band, bending over
        px(sp, CX + 1, hy - 10, "M"); px(sp, CX + 2, hy - 11, "y"); px(sp, CX + 3, hy - 11, "y"); px(sp, CX + 4, hy - 10, "T")


def tovin_back(sp, c, d, kind, ph, bob, x):
    # the pole stands behind the near hand: a long dark staff
    px_ = CX + 6.5 if d != "left" else CX - 4.5
    dy = x.get("pole_dy", 0)
    for y in range(2 + dy, FOOT + 1 + min(dy, 0)):
        sp.put(int(px_), y, "P" if y % 3 else "o")


def tovin_front(sp, c, d, kind, ph, bob, x, hands):
    top = c.top + bob
    t = x.get("t", 0)
    flame = ["6", "@", "6", "4"][t % 4]
    # lantern hung at the top of the pole
    lx = int(CX + 6.5) if d != "left" else int(CX - 4.5)
    dy = x.get("pole_dy", 0)
    sw = x.get("sway", [0, 1, 0, -1][t % 4] if kind == "idle" else 0)
    sp.fill(rect(lx - 1 + sw, 4 + dy, 3, 4), "k")
    sp.fill(rect(lx - 1 + sw, 5 + dy, 3, 2), flame)
    px(sp, lx, 3 + dy, "o"); px(sp, lx, 2 + dy, "o")
    if sw:
        px(sp, lx + (1 if sw > 0 else -1) * 0, 3 + dy, "o")
    # the pole across the near hand
    if d == "down" or d == "up":
        hx, hy = hands[1]
        sp.fill(rect(int(CX + 6), int(hy) - 1, 2, 2), c.skin[1])
    else:
        hx, hy = hands["near"]
    # rope coil and the key ring on the belt
    if d == "down":
        for dx, dy in [(0, 0), (1, 0), (2, 0), (-1, 1), (3, 1), (-1, 2), (3, 2), (0, 3), (1, 3), (2, 3)]:
            px(sp, CX - 5 + dx, top + 8 + dy, "O" if (dx + dy) % 2 else "t")
        for dx, ch in [(2, "h"), (3, "i"), (4, "h")]:
            px(sp, CX + dx, top + 9, ch)
        px(sp, CX + 3, top + 10, "J")
    elif d == "left":
        for dx, dy in [(0, 0), (1, 0), (-1, 1), (2, 1), (-1, 2), (2, 2), (0, 3), (1, 3)]:
            px(sp, CX - 2 + dx, top + 8 + dy, "O" if (dx + dy) % 2 else "t")
    else:
        for dx, dy in [(0, 0), (1, 0), (2, 0), (-1, 1), (3, 1), (0, 2), (1, 2), (2, 2)]:
            px(sp, CX - 5 + dx, top + 8 + dy, "O" if (dx + dy) % 2 else "t")


TOVIN = Cfg(
    skin=["o", "t", "T"], hair=["I", "P", "r"], head_y=10.0, eye_dy=1.2,
    tunic=CLOAK, sleeve=["E", "l", "m"], belt=["I", "O"],
    legs=["D", "F", "E"], boot=["k", "I", "P"],
    sh=4.0, hm=5.2, side_w=3.4, hem=22, hat=tovin_hat, back=tovin_back, front=tovin_front,
)


# ---- Nessa ------------------------------------------------------------------

def nessa_hat(sp, c, d, kind, ph, bob, x):
    hy = c.head_y + bob
    if d == "left":
        hx = CX - 0.5
        sp.paint(ell(hx + 0.6, hy - 1.8, 4.2, 3.0), STEEL, rim=True)
        sp.paint(poly([(hx + 2.6, hy - 1.2), (hx + 5.4, hy + 2.0), (hx + 3.0, hy + 3.0), (hx + 1.6, hy + 0.5)]), STEEL_D, rim=True, fall=False)
        sp.fill(rect(int(hx - 3), int(hy - 0.4), 3, 1), "g")
    elif d == "up":
        sp.paint(ell(CX, hy - 0.8, 4.4, 3.9), STEEL, rim=True)
        sp.paint(poly([(CX - 3.6, hy + 1.6), (CX + 3.6, hy + 1.6), (CX + 4.4, hy + 4.0), (CX - 4.4, hy + 4.0)]), STEEL_D, rim=True, fall=False)
    else:
        sp.paint(ell(CX, hy - 2.0, 4.4, 3.0), STEEL, rim=True)
        sp.fill(rect(int(CX - 4), int(hy - 0.4), 9, 1), "g")
        sp.fill(rect(int(CX - 3), int(hy - 0.4), 1, 1), "J")
        # a crest bead on top
        px(sp, CX, hy - 5.4, "i")


def nessa_back(sp, c, d, kind, ph, bob, x):
    pass


def loaf(sp, cx, cy, seed=0):
    sp.paint(ell(cx, cy, 2.1, 1.5), ["R", "O", "t"], rim=True, fall=False, seed=seed)
    sp.put(int(cx), int(cy) - 1, "T")


def nessa_front(sp, c, d, kind, ph, bob, x, hands):
    top = c.top + bob
    if d == "down":
        # the floured apron over her red tabard, a leather skirt below it
        sp.paint(poly([(CX - 2.6, top + 1.5), (CX + 2.6, top + 1.5), (CX + 3.6, c.hem - 4), (CX - 3.6, c.hem - 4)]), ["i", "H", "W"], rim=True, fall=False)
        sp.paint(poly([(CX - 4.6, c.hem - 4), (CX + 4.6, c.hem - 4), (CX + 5.4, c.hem), (CX - 5.4, c.hem)]), ["I", "B", "o"], rim=True, fall=False)
        px(sp, CX - 1, top + 4, "V"); px(sp, CX + 1, top + 6, "V")
        # a tray of loaves held at her belly
        sp.paint(rect(int(CX - 5), top + 9, 11, 2), ["I", "P", "o"], rim=True, fall=False)
        for i, lx in enumerate((-3.0, 0.5, 4.0)):
            loaf(sp, CX + lx, top + 7.6 - (1 if (x.get("t", 0) + i) % 4 == 1 and kind == "idle" else 0), i)
        sp.fill(rect(int(CX - 6), top + 9, 1, 2), c.skin[1])
        sp.fill(rect(int(CX + 6), top + 9, 1, 2), c.skin[1])
    elif d == "up":
        # the shield-board slung on her back
        sp.paint(ell(CX, top + 6, 4.4, 4.6), ["I", "B", "O"], rim=True, fall=False)
        sp.fill(rect(int(CX), top + 2, 1, 9), "I")
        sp.fill(rect(int(CX - 3), top + 6, 7, 1), "I")
        px(sp, CX, top + 6, "t")
    else:
        hx, hy = hands["near"]
        sp.paint(poly([(CX - 3.0, top + 1.5), (CX + 0.4, top + 1.5), (CX + 0.4, c.hem - 4), (CX - 3.2, c.hem - 4)]), ["i", "H", "W"], rim=True, fall=False)
        sp.paint(rect(int(CX - 7), top + 9, 6, 2), ["I", "P", "o"], rim=True, fall=False)
        for i, lx in enumerate((-6.0, -2.6)):
            loaf(sp, CX + lx, top + 7.6, i)
        sp.paint(ell(CX + 3.4, top + 6, 1.6, 4.0), ["I", "B", "O"], rim=True, fall=False)


NESSA = Cfg(
    skin=["O", "t", "T"], hair=["b", "I", "p"], head_y=10.2, eye_dy=1.2, head_r=(4.3, 3.7),
    tunic=["!", "7", "R"], sleeve=["8", "p", "R"], belt=None,
    legs=["Z", "n", "I"], boot=["k", "b", "I"],
    sh=5.2, hm=5.6, side_w=4.2, hat=nessa_hat, back=nessa_back, front=nessa_front, leg_w=3, hem=21, top=12,
)


# ---- Orin -------------------------------------------------------------------

COAT = ["s", "x", "S", "u"]


def orin_hat(sp, c, d, kind, ph, bob, x):
    hy = c.head_y + bob + c.stoop
    if d == "left":
        hx = CX - 0.5 - c.head_dx
        sp.paint(ell(hx + 0.8, hy - 2.4, 3.6, 2.2), COAT, rim=True, fall=False)
        for dx, dy, ch in [(2, -4, "j"), (3, -3, "G"), (4, -2, "j")]:
            px(sp, hx + dx, hy + dy, ch)
    elif d == "up":
        sp.paint(ell(CX, hy - 1.8, 3.9, 2.8), COAT, rim=True, fall=False)
        for dx in (-3, 3):
            px(sp, CX + dx, hy + 1, "j")
        px(sp, CX - 4, hy + 2, "G"); px(sp, CX + 4, hy + 2, "G")
    else:
        sp.paint(ell(CX, hy - 2.6, 3.9, 2.0), COAT, rim=True, fall=False)
        sp.fill(rect(int(CX - 3), int(hy - 1), 7, 1), "s")
        # grey hair at the temples, spectacles over the eyes
        px(sp, CX - 4, hy, "j"); px(sp, CX + 4, hy, "j")
        px(sp, CX - 3, hy + c.eye_dy, "V"); px(sp, CX + 2, hy + c.eye_dy, "V")


def orin_back(sp, c, d, kind, ph, bob, x):
    pass


def orin_front(sp, c, d, kind, ph, bob, x, hands):
    top = c.top + bob + c.stoop
    t = x.get("t", 0)
    ink = "k"
    if d == "down":
        for side in (-1, 1):
            hx, hy = hands[side]
            sp.fill(rect(int(hx - 0.9), int(hy - 1), 2, 2), ink)
            # a quill in each hand, feather up and out
            twitch = 1 if kind == "idle" and t % 2 == 1 else 0
            for i in range(5):
                px(sp, hx + side * (i // 2 + twitch * (i > 1)), hy - 2 - i, "W" if i > 1 else "V")
            px(sp, hx, hy - 1, ink)
        # ink stains on the coat
        for dx, dy in [(-2, 8), (-1, 8), (2, 5), (3, 9), (0, 9)]:
            px(sp, CX + dx, top + dy, "q" if (dx + dy) % 2 else "d")
    elif d == "up":
        for dx, dy in [(-2, 6), (2, 5), (1, 8)]:
            px(sp, CX + dx, top + dy, "q")
        for side in (-1, 1):
            hx, hy = hands[side]
            sp.fill(rect(int(hx - 0.9), int(hy - 1), 2, 2), ink)
    else:
        hx, hy = hands["near"]
        sp.fill(rect(int(hx - 0.9), int(hy), 2, 2), ink)
        for i in range(5):
            px(sp, hx - (i // 2), hy - 1 - i, "W" if i > 1 else "V")
        for dx, dy in [(-1, 8), (1, 6), (2, 9)]:
            px(sp, CX + dx, top + dy, "q" if (dx + dy) % 2 else "d")


ORIN = Cfg(
    skin=["$", "c", "9"], hair=["j", "G", "h"], head_y=9.8, eye_dy=1.2, head_r=(3.5, 3.5),
    tunic=COAT, sleeve=["s", "x", "S"], belt=None, stoop=1, head_dx=0.8,
    legs=["d", "s", "x"], boot=["k", "d", "x"],
    sh=3.2, hm=4.4, side_w=2.6, leg_w=2, hat=orin_hat, back=orin_back, front=orin_front, hem=22, top=12, gait=2,
)


# ---- Pella ------------------------------------------------------------------

GOLD = ["a", "4", "6", "5"]
TABARD = ["i", "H", "W", "W"]


def pella_hat(sp, c, d, kind, ph, bob, x):
    hy = c.head_y + bob
    if d == "left":
        hx = CX - 0.5
        sp.paint(ell(hx + 0.8, hy - 2.6, 3.5, 2.0), ["I", "B", "O"], rim=True, fall=False)
        sp.fill(rect(int(hx - 3), int(hy - 1.5), 7, 1), "6")
        for dx, dy, ch in [(2, -6, "H"), (3, -7, "W"), (4, -7, "V"), (5, -6, "V")]:
            px(sp, hx + dx, hy + dy, ch)
    elif d == "up":
        sp.paint(ell(CX, hy - 1.6, 4.0, 3.0), ["I", "B", "O"], rim=True, fall=False)
        sp.fill(rect(int(CX - 4), int(hy - 1.0), 9, 1), "6")
        for dx, dy, ch in [(2, -6, "H"), (3, -7, "W"), (4, -7, "V")]:
            px(sp, CX + dx, hy + dy, ch)
    else:
        sp.paint(ell(CX, hy - 2.6, 4.2, 2.2), ["I", "B", "O"], rim=True, fall=False)
        sp.fill(rect(int(CX - 4), int(hy - 1.5), 9, 1), "6")
        px(sp, CX, hy - 1.5, "w")
        # a tall white feather swept to the right
        for dx, dy, ch in [(3, -5, "H"), (4, -6, "W"), (5, -7, "W"), (5, -6, "V"), (6, -6, "V"), (4, -5, "V")]:
            px(sp, CX + dx, hy + dy, ch)


def pella_back(sp, c, d, kind, ph, bob, x):
    pass


def star(sp, cx, cy, ch="6"):
    for dx, dy in [(0, -2), (0, -1), (0, 0), (0, 1), (-2, 0), (-1, 0), (1, 0), (2, 0), (-1, 1), (1, 1), (-1, 2), (1, 2)]:
        px(sp, cx + dx, cy + dy, ch)
    px(sp, cx, cy, "w")


def pella_front(sp, c, d, kind, ph, bob, x, hands):
    top = c.top + bob
    if d == "down":
        # a gold collar and hem on the white tabard, a star on the chest
        sp.fill(rect(int(CX - 3), top, 7, 1), "6")
        sp.fill(rect(int(CX - 4), c.hem - 2, 9, 1), "4")
        sp.fill(rect(int(CX - 4), c.hem - 1, 9, 1), "a")
        star(sp, CX - 0.0, top + 4)
        # the horn on its lanyard, hung at her right hip
        for i in range(5):
            px(sp, CX + 3 - (i > 2), top + 1 + i, "a")
        sw = 1 if kind == "idle" and x.get("t", 0) % 2 == 1 else 0
        sp.paint(poly([(CX + 2.5 + sw, top + 7), (CX + 6.5 + sw, top + 6), (CX + 7 + sw, top + 10.5), (CX + 2.5 + sw, top + 9)]), ["a", "4", "6", "5"], rim=True, fall=False)
        px(sp, CX + 6 + sw, top + 8, "k")
        # the rolled scroll in her left hand
        hx, hy = hands[-1]
        sp.paint(rect(int(hx - 1.5), int(hy) - 2, 3, 5), ["$", "T", "9"], rim=True, fall=False)
        px(sp, hx - 0.5, hy + 0.5, "7")
    elif d == "up":
        sp.fill(rect(int(CX - 3), top, 7, 1), "6")
        sp.fill(rect(int(CX - 4), c.hem - 2, 9, 1), "4")
        sp.fill(rect(int(CX - 4), c.hem - 1, 9, 1), "a")
        sp.paint(ell(CX, top + 6, 3.0, 2.8), ["a", "4", "6"], rim=True, fall=False)
    else:
        hx, hy = hands["near"]
        sp.fill(rect(int(CX - 3), top, 6, 1), "6")
        sp.fill(rect(int(CX - 3), c.hem - 2, 7, 1), "4")
        sp.fill(rect(int(CX - 3), c.hem - 1, 7, 1), "a")
        star(sp, CX - 1.0, top + 4)
        sp.paint(poly([(CX + 0.5, top + 7), (CX + 4.5, top + 6), (CX + 5, top + 10.5), (CX + 0.5, top + 9)]), ["a", "4", "6", "5"], rim=True, fall=False)
        sp.paint(rect(int(hx - 2), int(hy) - 2, 3, 5), ["$", "T", "9"], rim=True, fall=False)
        px(sp, hx - 1, hy + 0.5, "7")


PELLA = Cfg(
    skin=["r", "o", "O"], hair=["n", "b", "I"], head_y=9.4, eye_dy=1.3,
    tunic=TABARD, sleeve=["g", "j", "i"], belt=["a", "4"],
    legs=["K", "g", "j"], boot=["k", "I", "P"],
    sh=3.8, hm=4.2, side_w=3.0, top=11, hem=21, hat=pella_hat, back=pella_back, front=pella_front,
)

PEOPLE = {"mira": MIRA, "tovin": TOVIN, "nessa": NESSA, "orin": ORIN, "pella": PELLA}


# ---- what each of them does for a living (front-facing, four frames) ---------

def mira_read(ph):
    c = MIRA
    sp = person(c, "down", "idle", ph, x={"bob": 1 if ph % 2 else 0, "skip": (-1, 1), "bare": True, "t": ph})
    top = c.top + (1 if ph % 2 else 0)
    # an open book held at the chest, a page lifting on two of the frames
    sp.paint(rect(int(CX - 4), top + 5, 9, 5), ["I", "B", "R"], rim=True, fall=False)
    sp.fill(rect(int(CX - 3), top + 6, 4, 3), "H")
    sp.fill(rect(int(CX + 1), top + 6, 3, 3), "W")
    sp.fill(rect(int(CX), top + 6, 1, 3), "i")
    for y in (7, 8):
        sp.fill(rect(int(CX - 2), top + y, 2, 1), "j") if ph != 2 else None
        sp.fill(rect(int(CX + 2), top + y, 2, 1), "j")
    if ph in (1, 2):
        sp.fill(rect(int(CX + 1), top + 5, 4, 1), "W")
        px(sp, CX + 4, top + 4, "H")
    arm_to(sp, c, -1, 1 if ph % 2 else 0, CX - 4, top + 9)
    arm_to(sp, c, 1, 1 if ph % 2 else 0, CX + 4, top + 9)
    return sp


def mira_light(ph):
    c = MIRA
    sp = person(c, "down", "idle", 0, x={"skip": (1,), "bare": True, "t": ph})
    top = c.top
    reach = [(CX + 4.5, top - 1), (CX + 4.8, top - 4), (CX + 4.8, top - 5)][ph]
    sp.paint(rect(int(CX) - 7, top + 5, 3, 5), ["I", "B", "R"], rim=True, fall=False)
    arm_to(sp, c, 1, 0, reach[0], reach[1])
    tx, ty = int(reach[0]), int(reach[1])
    sp.fill(rect(tx, ty - 4, 1, 3), "T")
    px(sp, tx, ty - 5, "6"); px(sp, tx, ty - 6, "@" if ph else "4")
    if ph == 2:
        px(sp, tx - 1, ty - 6, "4"); px(sp, tx + 1, ty - 6, "4"); px(sp, tx, ty - 7, "w")
    return sp


def tovin_work(ph):
    c = TOVIN
    return person(c, "down", "idle", ph, x={"pole_dy": [0, -1, -2, -1][ph], "sway": [0, 1, 0, -1][ph], "t": ph, "bob": [0, 0, -1, 0][ph]})


def nessa_hammer(ph):
    c = NESSA
    sp = person(c, "down", "idle", 0, x={"skip": (1,), "bare": True, "bob": [0, -1, 0, 0][ph]})
    top = c.top + [0, -1, 0, 0][ph]
    # the apron and the skirt, no tray
    sp.paint(poly([(CX - 2.6, top + 1.5), (CX + 2.6, top + 1.5), (CX + 3.6, c.hem - 4), (CX - 3.6, c.hem - 4)]), ["i", "H", "W"], rim=True, fall=False)
    sp.paint(poly([(CX - 4.6, c.hem - 4), (CX + 4.6, c.hem - 4), (CX + 5.4, c.hem), (CX - 5.4, c.hem)]), ["I", "B", "o"], rim=True, fall=False)
    hand = [(CX + 6.0, top - 3), (CX + 6.0, top - 5), (CX + 3.5, top + 9), (CX + 5.0, top + 4)][ph]
    arm_to(sp, c, 1, [0, -1, 0, 0][ph], hand[0], hand[1])
    hx, hy = int(hand[0]), int(hand[1])
    # the hammer: a haft and an iron head
    if ph in (0, 1):
        sp.fill(rect(hx, hy - 4, 1, 4), "P"); sp.fill(rect(hx - 1, hy - 6, 3, 2), "j"); px(sp, hx - 1, hy - 6, "i")
    elif ph == 2:
        sp.fill(rect(hx - 4, hy, 4, 1), "P"); sp.fill(rect(hx - 6, hy - 1, 2, 3), "j"); px(sp, hx - 6, hy - 1, "i")
        for dx, dy, ch in [(-8, -1, "6"), (-7, 2, "4"), (-9, 1, "w"), (-5, -2, "@")]:
            px(sp, hx + dx, hy + dy, ch)
    else:
        sp.fill(rect(hx, hy - 3, 1, 3), "P"); sp.fill(rect(hx - 1, hy - 5, 3, 2), "j")
    return sp


def orin_write(ph):
    c = ORIN
    sp = person(c, "down", "idle", ph, x={"skip": (-1, 1), "bare": True, "bob": 1})
    top = c.top + c.stoop + 1
    # a sheet held at the chest, the quill scratching along a line
    sp.paint(rect(int(CX - 3), top + 6, 7, 4), ["$", "T", "9"], rim=True, fall=False)
    for i in range(3):
        sp.fill(rect(int(CX - 2), top + 7 + i // 2 * 0, 1, 1), "j") if False else None
    for y in (7, 8, 9):
        sp.fill(rect(int(CX - 2), top + y, 1 + (ph + y) % 3 + 1, 1), "q")
    arm_to(sp, c, -1, 1, CX - 3, top + 9, hand="k")
    qx = [CX + 1, CX + 2, CX + 3, CX + 2][ph]
    arm_to(sp, c, 1, 1, qx, top + 8, hand="k")
    for i in range(4):
        px(sp, qx + i // 2 + 1, top + 7 - i, "W" if i > 0 else "k")
    return sp


def orin_search(ph):
    c = ORIN
    sp = person(c, "down", "idle", 0, x={"skip": (-1, 1), "bare": True, "bob": 3})
    top = c.top + c.stoop + 3
    sx = [CX - 4, CX - 1, CX + 2, CX - 1][ph]
    arm_to(sp, c, -1, 3, sx, FOOT - 4, hand="k")
    arm_to(sp, c, 1, 3, CX + 5.5 - (sx - CX) * 0.5, FOOT - 5, hand="k")
    for i, ch in enumerate(["M", "y", "A"]):
        px(sp, sx + 1 + i % 2, FOOT - 6 - i, ch)
    return sp


def pella_cry(ph):
    c = PELLA
    sp = person(c, "down", "idle", 0, x={"skip": (1,), "bare": True, "bob": [0, 0, -1, -1][ph]})
    top = c.top + [0, 0, -1, -1][ph]
    sp.fill(rect(int(CX - 3), top, 7, 1), "6")
    sp.fill(rect(int(CX - 4), c.hem - 2, 9, 1), "4")
    sp.fill(rect(int(CX - 4), c.hem - 1, 9, 1), "a")
    star(sp, CX - 0.0, top + 4)
    hy = c.head_y + [0, 0, -1, -1][ph]
    lift = [(CX + 4.0, top + 7), (CX + 4.5, top + 3), (CX + 3.0, hy + 3.2), (CX + 3.0, hy + 3.2)][ph]
    arm_to(sp, c, 1, [0, 0, -1, -1][ph], lift[0], lift[1])
    hx, hy2 = int(lift[0]), int(lift[1])
    # the brass horn: a thin tube out to a flared bell
    if ph >= 2:
        sp.paint(poly([(hx + 1, hy2 - 1), (hx + 6, hy2 - 3), (hx + 7, hy2 + 2), (hx + 1, hy2 + 1)]), ["a", "4", "6", "5"], rim=True, fall=False)
        for dx, dy, ch in ([(9, -3, "T"), (10, 0, "T"), (9, 3, "T")] if ph == 2 else [(10, -4, "t"), (12, 0, "t"), (10, 4, "t"), (11, -2, "O"), (11, 2, "O")]):
            px(sp, hx + dx, hy2 + dy, ch)
    else:
        sp.paint(poly([(hx + 1, hy2 - 3), (hx + 4, hy2 - 5), (hx + 6, hy2 - 3), (hx + 3, hy2 - 1)]), ["a", "4", "6", "5"], rim=True, fall=False)
    hxl, hyl = hands_left = (CX - 4.2, top + 7)
    sp.paint(rect(int(hxl - 1.5), int(hyl) - 2, 3, 5), ["$", "T", "9"], rim=True, fall=False)
    return sp


WORK = {
    "mira": {"work": mira_read, "light": mira_light},
    "tovin": {"work": tovin_work},
    "nessa": {"work": nessa_hammer},
    "orin": {"work": orin_write, "search": orin_search},
    "pella": {"cry": pella_cry},
}
WORK_FRAMES = {"mira": {"work": 4, "light": 3}, "tovin": {"work": 4}, "nessa": {"work": 4}, "orin": {"work": 4, "search": 4}, "pella": {"cry": 4}}



def preview(name="mira"):
    c = PEOPLE[name]
    rows = []
    for d in ("down", "up", "left"):
        for kind, n in (("idle", 2), ("walk", 4)):
            for ph in range(n):
                rows.append(person(c, d, kind, ph, x={"t": ph}).finish().rows())
    for state, f in WORK.get(name, {}).items():
        for ph in range(WORK_FRAMES[name][state]):
            rows.append(f(ph).finish().rows())
    return rows


def lineup():
    rows = []
    for n, c in PEOPLE.items():
        rows.append(person(c, "down", "idle", 0).finish().rows())
    for n, c in PEOPLE.items():
        rows.append(person(c, "left", "idle", 0).finish().rows())
    return rows


if __name__ == "__main__":
    import sys
    from engine import sheet
    names = sys.argv[1:] or list(PEOPLE)
    for n in names:
        sheet(preview(n), scale=7, cols=6, bg=(60, 58, 52)).save(f"/work/tmp/mf/{n}.png")
    sheet(lineup(), scale=8, cols=5, bg=(60, 58, 52)).save("/work/tmp/mf/lineup.png")
