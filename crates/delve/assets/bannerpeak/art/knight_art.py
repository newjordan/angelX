"""Bannerpeak's knight, inked by hand as parts and composed into poses.

Template inks: steel j J h i (dark to spec), seat tones 1 (shade) 2 (base) 3 (light) 4 (spec),
gold trim o t T, leather I B, eye c. Facing right; the game flips for left.
The silhouette gets one black rim at the end; inner edges are inked by tone.
"""
from engine import parse, shift, over, flip, rot90, rotate, recolor, rows, render, outline

W, H = 24, 28
CX = 12          # anchor column; the feet are on the last row

# ---- the helm, 8 x 7, face to the right: the dome tops the 20 px box at row 9
HELM = parse([
    "..hhhh..",
    ".hiihhJ.",
    "hhihhhJJ",
    "hhhhhhJJ",
    "JhhJkkcJ",
    "JJhhJhJj",
    ".jJJJJj.",
], 8, 9)

# ---- the body: gorget, breastplate under a seat tabard, a gold belt
TORSO = parse([
    ".jJhhJj..",
    "jJh222Jj.",
    "J1h2222J1",
    "J1h22321j",
    "j1222221.",
    ".otTtoto.",
    ".1222J12.",
], 8, 16)

# ---- the shield: a heater on the forward arm, gold rim round a seat field, cream crescent
SHIELD = parse([
    "ottto.",
    "o2332o",
    "o23TTo",
    "o2T22o",
    "o2332o",
    ".o22o.",
    "..oo..",
], 13, 17)

# the same shield raised to guard: taller, held high and forward
SHIELD_UP = parse([
    ".ottto.",
    "o23332o",
    "o23TT2o",
    "o2T222o",
    "o2T222o",
    "o23TT2o",
    "o23332o",
    ".o222o.",
    "..o2o..",
    "...o...",
], 13, 12)

# ---- plumes: a swept feather raked from the helm's top back (11, 8), tones 3 light, 2 base, 1 shade
def plume(root, ctrl, tip, w=3):
    pts = {}
    n = 40
    for i in range(n + 1):
        t = i / n
        x = (1 - t) ** 2 * root[0] + 2 * (1 - t) * t * ctrl[0] + t * t * tip[0]
        y = (1 - t) ** 2 * root[1] + 2 * (1 - t) * t * ctrl[1] + t * t * tip[1]
        width = max(1, round(w * (1 - t * 0.75)))
        cx, cy = round(x), round(y)
        top = cy - width // 2
        for k in range(width):
            tone = "3" if k == 0 and width > 1 else ("1" if k == width - 1 and width > 1 else "2")
            pts.setdefault((cx, top + k), tone)
    return pts

PLUME = {
    "hang": plume((11, 8), (6, 3), (4, 13)),
    "back1": plume((11, 8), (6, 4), (0, 8)),
    "back2": plume((11, 8), (6, 3), (0, 5)),
    "up": plume((11, 8), (8, 0), (4, 5)),
    "down": plume((11, 8), (6, 7), (5, 16)),
    "snap": plume((11, 8), (16, 1), (18, 8)),
}



def _line(p, q):
    (x0, y0), (x1, y1) = p, q
    n = max(abs(x1 - x0), abs(y1 - y0), 1)
    return [(round(x0 + (x1 - x0) * i / n), round(y0 + (y1 - y0) * i / n)) for i in range(n + 1)]

def leg(hip, knee, foot, back=False, boot=2):
    steel = ("j", "j") if back else ("J", "j")
    out = {}
    path = _line(hip, knee)[:-1] + _line(knee, foot)
    for (x, y) in path:
        for dx in (-1, 0, 1):
            out[(x + dx, y)] = steel[0] if dx < 1 else steel[1]
    fx, fy = foot
    for dy in range(boot):
        for dx in range(-1, 3 if dy == boot - 1 else 2):
            out[(fx + dx, fy - dy)] = "B"
    for dx in range(-1, 3):
        out[(fx + dx, fy)] = "I" if back else "B"
    out[(fx, fy - boot)] = "j" if back else "J"
    return out

# hip back, foot back, knee back, hip front, foot front, knee front (small knight's rows)
STANCE = {
    "stand": ((10, 23), (10, 27), (10, 25), (14, 23), (14, 27), (14, 25)),
    "rise": ((10, 23), (7, 26), (8, 25), (14, 23), (16, 24), (16, 23)),
    "fall": ((10, 23), (8, 27), (9, 25), (14, 23), (17, 26), (16, 24)),
    "crouch": ((10, 25), (9, 27), (10, 26), (14, 25), (15, 27), (15, 26)),
    "skid": ((9, 23), (6, 27), (7, 25), (14, 23), (17, 27), (16, 25)),
    "wall": ((10, 23), (9, 27), (10, 25), (14, 23), (15, 25), (15, 24)),
    "brace": ((10, 26), (9, 27), (10, 27), (14, 26), (15, 27), (15, 27)),
}

def stance(pts, big=False):
    def m(p):
        if not big:
            return p
        # the big knight's hips sit 6 rows lower and its feet 10: legs two thirds longer
        return (p[0], 29 + (p[1] - 23) * 2) if p[1] >= 23 else p
    hb, fb, kb, hf, ff, kf = [m(p) for p in pts]
    return over(leg(hb, kb, fb, back=True), leg(hf, kf, ff))

def legs(name, big=False):
    return stance(STANCE[name], big)

def run_pts(i):
    import math
    hy = 23
    def foot(u, hx):
        u %= 1.0
        S = 4
        if u < 0.5:
            return hx + round(S - 2 * S * (u / 0.5)), 27, 0
        k = (u - 0.5) / 0.5
        lift = round(3.2 * math.sin(math.pi * k))
        return hx - S + round(2 * S * k), 27 - lift, lift
    (bx, by, bl), (fx, fy, fl) = foot(i / 6, 10), foot(i / 6 + 0.5, 14)
    kb = ((10 + bx) // 2 + 1 + bl // 2, (hy + by) // 2 - bl // 2)
    kf = ((14 + fx) // 2 + 1 + fl // 2, (hy + fy) // 2 - fl // 2)
    return ((10, hy), (bx, by), kb, (14, hy), (fx, fy), kf)

def run_legs(i, big=False):
    return stance(run_pts(i), big)

def finish(*layers):
    return outline(over(*layers), "k")

def stretch(spr, after, extra, dup_rows=1):
    """Insert `extra` copies of row `after` (a body row to repeat) into a part."""
    out = {}
    for (x, y), c in spr.items():
        if y <= after:
            out[(x, y)] = c
        else:
            out[(x, y + extra)] = c
    for (x, y), c in list(spr.items()):
        if y == after:
            for k in range(1, extra + 1):
                out[(x, y + k)] = c
    return out

# the big knight: a taller tabard under a gold gorget, steel pauldrons, a longer shield
TORSO_BIG = stretch(stretch(TORSO, 16 + 3, 3), 16 + 4 + 3, 3)
# a steel cuirass over the chest, the seat's tabard hanging below the belt, a gold collar
_cuirass = {}
for (x, y), c in TORSO_BIG.items():
    if 17 <= y <= 23 and c in "23":
        c = "h" if (c == "3" or (x + y) % 5 == 0) else "J"
    if y == 17 and c in "Jh":
        c = "t" if x % 2 else "o"
    _cuirass[(x, y)] = c
TORSO_BIG = _cuirass
SHIELD_BIG = stretch(stretch(SHIELD, 17 + 2, 2), 17 + 4 + 2, 2)
PAULDRON_F = parse([
    ".otto.",
    "oThhJ.",
    "ohiJjo",
    ".jJjj.",
], 14, 16)
PAULDRON_B = parse([
    ".ott.",
    "ohhJo",
    "jJjj.",
], 7, 16)
TORSO_SHORT = {p: c for p, c in TORSO.items() if p[1] <= 20}
TORSO_BIG_SHORT = {p: c for p, c in TORSO_BIG.items() if p[1] <= 26}

def eyes_shut(s):
    s = dict(s)
    for p, c in list(s.items()):
        if c == "c":
            s[p] = "k"
    return s

def raw(plume_name="hang", leg=None, dy=0, dx=0, shield=None, helm_dy=0, torso_dy=0, torso=None, big=False, plume_dy=None):
    if shield is None:
        shield = SHIELD_BIG if big else SHIELD
    if torso is None:
        torso = TORSO_BIG if big else TORSO
    parts = [
        shift(PLUME[plume_name], dx, dy + helm_dy if plume_dy is None else plume_dy),
        leg if leg is not None else legs("stand", big),
    ]
    parts += [shift(torso, dx, dy + torso_dy), shift(HELM, dx, dy + helm_dy), shift(shield, dx, dy + torso_dy)]
    return over(*parts)

def body(*a, **k):
    return outline(raw(*a, **k), "k")

def tuck_raw():
    """The knight curled in a ball for the somersault and the slam, on a 16 x 16 square."""
    helm = shift(HELM, -4, -4)
    torso = shift({p: c for p, c in TORSO.items() if p[1] <= 21}, -4, -10)
    boots = over(leg((7, 9), (9, 12), (10, 12)), leg((10, 9), (12, 12), (13, 12)))
    shield = shift(SHIELD, -4, -10)
    tail = shift(PLUME["hang"], -4 + 1, -4 + 1)
    tail = {p: c for p, c in tail.items() if 0 <= p[0] < 16 and 0 <= p[1] < 16}
    return over(tail, boots, torso, helm, shield)

def knight_frames(big=False):
    f = {}
    lo = 10 if big else 0           # how much taller the box is
    L = lambda n: legs(n, big)
    f["idle"] = [body("hang", big=big), body("hang", helm_dy=1, big=big)]
    f["run"] = []
    for i in range(6):
        bob = -1 if i % 3 == 1 else 0
        f["run"].append(body("back1" if i % 2 == 0 else "back2", run_legs(i, big), dy=bob, dx=1, big=big))
    f["skid"] = [body("snap", L("skid"), dx=-1, big=big)]
    f["rise"] = [body("up", L("rise"), big=big)]
    f["apex"] = [body("hang", L("rise"), big=big)]
    f["fall"] = [body("down", L("fall"), big=big)]
    f["second"] = [body("up", L("rise"), dy=-1, big=big)]
    f["wall"] = [body("down", L("wall"), big=big)]
    cs = 6 if not big else 14
    f["crouch"] = [outline(over(
        shift(PLUME["hang"], 0, cs),
        L("crouch"),
        shift(TORSO_BIG_SHORT if big else TORSO_SHORT, 0, cs),
        shift(HELM, 0, cs),
        shift(SHIELD, 0, cs - 2),
    ), "k")]
    f["slide"] = [outline(over(
        shift(PLUME["back1"], 1, cs + 1),
        L("skid"),
        shift(TORSO_BIG_SHORT if big else TORSO_SHORT, -1, cs),
        shift(HELM, -1, cs + 1),
        shift(SHIELD, 0, cs - 2),
    ), "k")]
    ball = tuck_raw()
    f["spin"], f["back"] = [], []
    for k in range(4):
        r, _, _ = rot90(ball, 16, 16, k)
        f["spin"].append(outline(shift(r, 4, 10 + lo), "k"))
        r, _, _ = rot90(ball, 16, 16, -k)
        f["back"].append(outline(shift(r, 4, 10 + lo), "k"))
    lj = raw("back2", L("rise"), dx=1, big=big)
    f["long"] = [outline(rotate(lj, 28, 12, 18 + lo // 2), "k")]
    dv = raw("back2", L("rise"), big=big)
    f["dive"] = [outline(rotate(dv, 80, 12, 18 + lo // 2), "k")]
    last = 27 + lo
    belly = rotate(raw("back1", L("rise"), big=big), 90, 12, 18 + lo // 2)
    ys = [y for (_, y) in belly]
    f["belly"] = [outline(shift(belly, 0, last - max(ys)), "k")]
    hurt = raw("down", L("fall"), dx=-1, big=big)
    f["hurt"] = [outline(eyes_shut(rotate(hurt, -16, 12, 18 + lo // 2)), "k")]
    f["brace"] = [outline(over(
        shift(PLUME["hang"], 0, 3 + lo // 2),
        legs("brace", big),
        shift(TORSO_BIG_SHORT if big else TORSO_SHORT, 0, 3 + lo // 2),
        shift(HELM, 0, 3 + lo // 2),
        shift(SHIELD_UP, 1, 4 + lo // 2),
    ), "k")]
    f["wind"] = [outline(shift(rot90(ball, 16, 16, 0)[0], 4, 10 + lo), "k")]
    f["slam"] = [outline(shift(rot90(ball, 16, 16, 1)[0], 4, 10 + lo), "k")]
    return f

ORDER = ("idle", "run", "skid", "rise", "apex", "fall", "second", "wall", "crouch", "slide",
         "spin", "back", "long", "dive", "belly", "hurt", "brace", "wind", "slam")

PALX = {"1": "q", "2": "Q", "3": "z", "4": "W"}

def preview(sprites, path="/work/tmp/bp/art/sheet.png", cols=4, zoom=14, h=H):
    sprites = [recolor(s, PALX) for s in sprites]
    render(sprites, W, h, zoom=zoom, cols=cols, box=(6, 8, 12, h - 8)).save(path)

if __name__ == "__main__":
    import sys
    big = len(sys.argv) > 1 and sys.argv[1] == "big"
    fr = knight_frames(big)
    allf = [s for k in ORDER for s in fr[k]]
    preview(allf, cols=10, zoom=5, h=38 if big else 28)
